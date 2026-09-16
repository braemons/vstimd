//! Finding the external tools and running them as subprocesses.
//!
//! Subprocesses rather than libraries: a training run frees all of its GPU
//! memory when it exits, a crash in a tool takes nothing else down, and there is
//! no wgpu or CUDA stack to keep in step with vstimd's.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::Serialize;

use crate::job::Job;

#[derive(Clone, Debug, Default)]
pub struct Tools {
    pub colmap: Option<PathBuf>,
    pub brush: Option<PathBuf>,
    /// Only needed when the capture is a video; a folder of stills needs none.
    pub ffmpeg: Option<PathBuf>,
}

impl Tools {
    /// Each tool from its flag, else its environment variable (`VSTIMD_COLMAP`,
    /// `VSTIMD_BRUSH`, `VSTIMD_FFMPEG`), else `PATH`.
    pub fn find(colmap: Option<PathBuf>, brush: Option<PathBuf>, ffmpeg: Option<PathBuf>) -> Tools {
        let pick = |flag: Option<PathBuf>, env: &str, names: &[&str]| {
            flag.or_else(|| std::env::var_os(env).map(PathBuf::from))
                .or_else(|| names.iter().find_map(|n| which(n)))
        };
        Tools {
            colmap: pick(colmap, "VSTIMD_COLMAP", &["colmap"]),
            brush: pick(brush, "VSTIMD_BRUSH", &["brush_app", "brush"]),
            ffmpeg: pick(ffmpeg, "VSTIMD_FFMPEG", &["ffmpeg"]),
        }
    }

    pub fn colmap(&self) -> anyhow::Result<&Path> {
        self.colmap.as_deref().context(
            "COLMAP was not found: install it (https://colmap.github.io/install.html), \
             or point --colmap or VSTIMD_COLMAP at the binary",
        )
    }

    /// Only asked for when the capture is a video, so the error says that
    /// rather than implying the tool is always required.
    pub fn ffmpeg(&self) -> anyhow::Result<&Path> {
        self.ffmpeg.as_deref().context(
            "ffmpeg was not found, and this capture is a video: install it \
             (Debian/Ubuntu: apt install ffmpeg), point --ffmpeg or VSTIMD_FFMPEG at it \
             (reconstruct/scripts/ffmpeg-docker runs it from a container), or pass a folder \
             of extracted frames instead. ffmpeg 5.1 or newer is needed, for -fps_mode",
        )
    }

    pub fn brush(&self) -> anyhow::Result<&Path> {
        self.brush.as_deref().context(
            "Brush was not found: download brush_app from \
             https://github.com/ArthurBrussee/brush/releases, or point --brush or VSTIMD_BRUSH at it",
        )
    }
}

pub fn which(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

#[derive(Debug, Serialize)]
pub struct ToolReport {
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CheckReport {
    pub ready: bool,
    pub colmap: ToolReport,
    /// Whether this COLMAP has the global mapper; without it `--mapper global`
    /// falls back to the incremental one.
    pub colmap_global_mapper: bool,
    pub brush: ToolReport,
}

pub fn check(tools: &Tools) -> CheckReport {
    let probe = |path: &Option<PathBuf>, args: &[&str], needle: &str| -> ToolReport {
        let Some(path) = path else {
            return ToolReport {
                path: None,
                version: None,
                error: Some("not found".into()),
            };
        };
        match output_text(Command::new(path).args(args)) {
            Ok((_, text)) => ToolReport {
                path: Some(path.clone()),
                version: text
                    .lines()
                    .find(|l| l.contains(needle))
                    .map(|l| l.trim().to_string()),
                error: None,
            },
            Err(e) => ToolReport {
                path: Some(path.clone()),
                version: None,
                error: Some(format!("{e:#}")),
            },
        }
    };
    let colmap = probe(&tools.colmap, &["help"], "COLMAP");
    let brush = probe(&tools.brush, &["--version"], "brush");
    CheckReport {
        ready: colmap.error.is_none() && brush.error.is_none(),
        colmap_global_mapper: tools
            .colmap
            .as_deref()
            .is_some_and(colmap_has_global_mapper),
        colmap,
        brush,
    }
}

pub fn colmap_has_global_mapper(colmap: &Path) -> bool {
    output_text(Command::new(colmap).args(["global_mapper", "-h"])).is_ok_and(|(ok, _)| ok)
}

fn output_text(cmd: &mut Command) -> anyhow::Result<(bool, String)> {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("running {}", cmd.get_program().to_string_lossy()))?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text += &String::from_utf8_lossy(&out.stderr);
    Ok((out.status.success(), text))
}

/// `[12/340]` or `[3/10, 1/5]`: the first counter in the last bracket, as COLMAP
/// prints while extracting and matching.
pub fn parse_bracket_counter(line: &str) -> Option<f32> {
    let start = line.rfind('[')?;
    let inner = &line[start + 1..];
    let (done, rest) = inner.split_once('/')?;
    let total: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let (done, total) = (done.trim().parse::<f32>().ok()?, total.parse::<f32>().ok()?);
    (total > 0.0).then(|| done / total)
}

/// Run `cmd` to completion for `job`: its stdout and stderr go line by line
/// into the log, `parse` turns lines into progress, and `poll` is asked for
/// progress every half second (for tools whose output says nothing).
pub fn run_tool(
    job: &mut Job,
    mut cmd: Command,
    parse: &dyn Fn(&str) -> Option<f32>,
    poll: &mut dyn FnMut() -> Option<f32>,
) -> anyhow::Result<()> {
    let program = Path::new(cmd.get_program())
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let first_arg = cmd
        .get_args()
        .next()
        .map(|a| a.to_string_lossy().into_owned())
        .unwrap_or_default();
    let label = format!("{program} {first_arg}");
    let line = std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    job.log_line(&format!("$ {line}"));

    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("starting {label}"))?;

    let (tx, rx) = mpsc::channel::<String>();
    let readers = [
        forward_lines(child.stdout.take().expect("piped"), tx.clone()),
        forward_lines(child.stderr.take().expect("piped"), tx),
    ];
    let mut tail = VecDeque::with_capacity(12);
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(text) => {
                if let Some(p) = parse(&text) {
                    job.progress(Some(p));
                }
                job.log_line(&text);
                if tail.len() == 12 {
                    tail.pop_front();
                }
                tail.push_back(text);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(p) = poll() {
                    job.progress(Some(p));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    for r in readers {
        let _ = r.join();
    }
    let status = child
        .wait()
        .with_context(|| format!("waiting for {label}"))?;
    if !status.success() {
        let tail: Vec<String> = tail.into_iter().collect();
        bail!(
            "{label} failed ({status}); its last output:\n  {}\nfull output: {}",
            tail.join("\n  "),
            job.dir.join("log.txt").display()
        );
    }
    Ok(())
}

/// Lines split on `\n` and `\r` alike, so progress bars that redraw arrive as lines.
fn forward_lines(
    stream: impl Read + Send + 'static,
    tx: mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();
        while reader.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
            for part in buf.split(|b| *b == b'\r' || *b == b'\n') {
                let text = String::from_utf8_lossy(part);
                let text = text.trim_end();
                if !text.is_empty() && tx.send(text.to_string()).is_err() {
                    return;
                }
            }
            buf.clear();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_colmap_counters() {
        assert_eq!(parse_bracket_counter("Processed file [12/48]"), Some(0.25));
        assert_eq!(
            parse_bracket_counter("Matching block [3/4, 1/5] in 0.1s"),
            Some(0.75)
        );
        assert_eq!(parse_bracket_counter("Elapsed time: 0.1 [minutes]"), None);
        assert_eq!(parse_bracket_counter("no counter"), None);
    }
}
