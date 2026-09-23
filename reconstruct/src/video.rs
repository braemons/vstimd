//! Video in, sharp frames out.
//!
//! A corridor is easier to capture as a walk-through video than as a few hundred
//! stills, and the capture guide has always said so — it just told the user to
//! run `ffmpeg -vf fps=3` themselves. That fixed rate is blind to the thing that
//! ruins a reconstruction: a handheld walk blurs unevenly, so a frame every
//! third of a second is sometimes the sharpest of its neighbourhood and
//! sometimes the worst. Rule 5 of the capture guide is "no blur".
//!
//! So frames are extracted at a multiple of the wanted rate and only the
//! sharpest of each group is kept. The choosing is done here, on grayscale
//! thumbnails, which is why [`sharpness`] and [`pick_sharpest`] are ordinary
//! functions over bytes and numbers: they are the part worth testing, and they
//! need neither ffmpeg nor a video to test.
//!
//! ffmpeg is run twice over the video. The first pass writes small grayscale
//! PGMs — no image decoder needed on our side, which is why PGM — and the second
//! writes only the frames the first pass chose, at full quality. Extracting the
//! rejects at full size and deleting them would cost gigabytes in `work/` for a
//! 4K walk.

use std::path::Path;

use anyhow::{Context, bail};

/// Extensions taken as video rather than as a folder of stills.
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v", "avi", "mkv", "webm", "mts", "m2ts"];

/// Whether this path names a video this tool will try to read.
pub fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.as_str()))
}

/// Width the scoring thumbnails are scaled to. Big enough that focus blur still
/// shows in the Laplacian, small enough that a thousand of them cost nothing.
pub const SCORE_WIDTH_PX: u32 = 320;

/// A grayscale PGM as ffmpeg writes it: `P5`, dimensions, maxval, then one byte
/// per pixel.
struct Pgm {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

fn parse_pgm(bytes: &[u8]) -> anyhow::Result<Pgm> {
    let mut pos = 0usize;
    // Fields are whitespace-separated ASCII; `#` starts a comment to end of line.
    let mut field = || -> anyhow::Result<String> {
        loop {
            while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos < bytes.len() && bytes[pos] == b'#' {
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    pos += 1;
                }
                continue;
            }
            break;
        }
        let start = pos;
        while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if start == pos {
            bail!("truncated PGM header");
        }
        Ok(String::from_utf8_lossy(&bytes[start..pos]).into_owned())
    };

    if field()? != "P5" {
        bail!("not a binary PGM (expected the magic P5)");
    }
    let width: usize = field()?.parse().context("PGM width")?;
    let height: usize = field()?.parse().context("PGM height")?;
    let maxval: u32 = field()?.parse().context("PGM maxval")?;
    if maxval > 255 {
        bail!("16-bit PGM is not supported (maxval {maxval})");
    }
    // Exactly one whitespace byte separates the header from the data.
    pos += 1;
    let want = width * height;
    if width == 0 || height == 0 || bytes.len() < pos + want {
        bail!(
            "PGM is {}×{} but holds {} pixel bytes",
            width,
            height,
            bytes.len().saturating_sub(pos)
        );
    }
    Ok(Pgm {
        width,
        height,
        pixels: bytes[pos..pos + want].to_vec(),
    })
}

/// How sharp a grayscale image is: the variance of its Laplacian.
///
/// The standard focus measure, and the right shape for this job — a blurred
/// image has small second derivatives everywhere, so the variance collapses,
/// while what the value *is* does not matter. Only the comparison between
/// neighbouring frames of one video is ever used, so exposure and content, which
/// shift the absolute number, cancel out.
pub fn sharpness(pgm_bytes: &[u8]) -> anyhow::Result<f64> {
    let img = parse_pgm(pgm_bytes)?;
    if img.width < 3 || img.height < 3 {
        return Ok(0.0);
    }
    let at = |x: usize, y: usize| f64::from(img.pixels[y * img.width + x]);
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut n = 0.0;
    for y in 1..img.height - 1 {
        for x in 1..img.width - 1 {
            let lap = 4.0 * at(x, y) - at(x - 1, y) - at(x + 1, y) - at(x, y - 1) - at(x, y + 1);
            sum += lap;
            sum_sq += lap * lap;
            n += 1.0;
        }
    }
    let mean = sum / n;
    Ok((sum_sq / n - mean * mean).max(0.0))
}

/// Indices of the frames to keep: the sharpest of each consecutive group of
/// `oversample`.
///
/// Grouping rather than a global threshold on purpose. A walk-through needs
/// frames spread evenly along the corridor — SfM matches neighbours, and a gap
/// breaks the chain — so the job is to pick the best frame *near each moment*,
/// not the best frames overall. A global threshold would happily drop a whole
/// blurry stretch and leave a hole no amount of sharpness elsewhere repairs.
///
/// A trailing partial group still yields its best frame, so the end of the walk
/// is never dropped.
pub fn pick_sharpest(scores: &[f64], oversample: u32) -> Vec<usize> {
    let step = oversample.max(1) as usize;
    scores
        .chunks(step)
        .enumerate()
        .filter_map(|(chunk, group)| {
            let (offset, _) = group
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?;
            Some(chunk * step + offset)
        })
        .collect()
}

/// The `-vf` argument that extracts exactly `keep` (indices into the stream
/// after `fps`) at the wanted rate.
///
/// Commas inside `select` are escaped because ffmpeg's own filter parser, not a
/// shell, splits on them: the argument is passed to the process directly.
///
/// The terms are summed as a balanced tree, not a flat `a+b+c+…` chain. ffmpeg
/// refuses a flat chain somewhere past a hundred terms ("Cannot allocate
/// memory" from the filter graph) — a 54 s walk kept 162 frames and failed —
/// while the same terms nested pairwise go only log₂ deep.
pub fn select_filter(fps: f32, keep: &[usize]) -> String {
    let terms: Vec<String> = keep.iter().map(|i| format!(r"eq(n\,{i})")).collect();
    format!("fps={fps},select='{}'", balanced_sum(&terms))
}

fn balanced_sum(terms: &[String]) -> String {
    match terms {
        [] => "0".to_string(),
        [one] => one.clone(),
        _ => {
            let (left, right) = terms.split_at(terms.len() / 2);
            format!("({}+{})", balanced_sum(left), balanced_sum(right))
        }
    }
}

/// The `-vf` argument for the scoring pass: the same frames, grayscale and small.
pub fn score_filter(fps: f32) -> String {
    format!("fps={fps},scale={SCORE_WIDTH_PX}:-1,format=gray")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PGM whose pixels come from `f`.
    fn pgm(width: usize, height: usize, f: impl Fn(usize, usize) -> u8) -> Vec<u8> {
        let mut out = format!("P5\n{width} {height}\n255\n").into_bytes();
        for y in 0..height {
            for x in 0..width {
                out.push(f(x, y));
            }
        }
        out
    }

    #[test]
    fn a_sharp_image_scores_above_a_blurred_one() {
        // A hard checkerboard, and the same thing smoothed into a gradient.
        let sharp = pgm(64, 64, |x, y| if (x / 4 + y / 4) % 2 == 0 { 0 } else { 255 });
        let blurred = pgm(64, 64, |x, y| ((x + y) * 2 % 256) as u8);
        let s = sharpness(&sharp).unwrap();
        let b = sharpness(&blurred).unwrap();
        assert!(s > b * 10.0, "sharp {s} should far exceed blurred {b}");
    }

    #[test]
    fn a_flat_image_has_no_sharpness() {
        assert_eq!(sharpness(&pgm(16, 16, |_, _| 128)).unwrap(), 0.0);
    }

    #[test]
    fn pgm_comments_and_odd_whitespace_parse() {
        let mut bytes = b"P5\n# made by ffmpeg\n4  4\n255\n".to_vec();
        bytes.extend(std::iter::repeat_n(7u8, 16));
        assert_eq!(sharpness(&bytes).unwrap(), 0.0);
    }

    #[test]
    fn a_truncated_pgm_is_an_error_not_a_panic() {
        assert!(sharpness(b"P5\n64 64\n255\nshort").is_err());
        assert!(sharpness(b"not a pgm at all").is_err());
    }

    #[test]
    fn the_sharpest_of_each_group_is_kept() {
        let scores = [1.0, 9.0, 2.0, /**/ 5.0, 4.0, 3.0, /**/ 0.0, 8.0, 7.0];
        assert_eq!(pick_sharpest(&scores, 3), vec![1, 3, 7]);
    }

    #[test]
    fn a_trailing_partial_group_still_yields_a_frame() {
        // The end of the walk must not be dropped: SfM needs the chain whole.
        let scores = [1.0, 2.0, 3.0, /**/ 4.0, 9.0];
        assert_eq!(pick_sharpest(&scores, 3), vec![2, 4]);
    }

    #[test]
    fn oversample_of_one_keeps_everything_in_order() {
        let scores = [3.0, 1.0, 2.0];
        assert_eq!(pick_sharpest(&scores, 1), vec![0, 1, 2]);
        assert_eq!(pick_sharpest(&scores, 0), vec![0, 1, 2]);
    }

    #[test]
    fn video_is_recognised_by_extension_whatever_the_case() {
        assert!(is_video(Path::new("walk.mp4")));
        assert!(is_video(Path::new("WALK.MOV")));
        assert!(!is_video(Path::new("photos")));
        assert!(!is_video(Path::new("frame.jpg")));
    }

    #[test]
    fn the_select_filter_escapes_commas_for_ffmpegs_parser() {
        assert_eq!(select_filter(3.0, &[0, 4]), r"fps=3,select='(eq(n\,0)+eq(n\,4))'");
        assert_eq!(select_filter(3.0, &[7]), r"fps=3,select='eq(n\,7)'");
    }

    #[test]
    fn a_long_walks_select_nests_log_deep_rather_than_chaining() {
        let keep: Vec<usize> = (0..162).map(|i| i * 3).collect();
        let filter = select_filter(3.0, &keep);
        let mut depth = 0i32;
        let mut deepest = 0;
        for c in filter.chars() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            deepest = deepest.max(depth);
        }
        // eq(…) adds one level; the sum of 162 terms adds ⌈log₂ 162⌉ = 8.
        assert!(deepest <= 9, "nested {deepest} deep");
        assert_eq!(filter.matches("eq(n").count(), 162);
    }
}
