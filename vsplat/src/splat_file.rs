//! Reading Gaussian splat scene files.
//!
//! Two formats:
//! - **`.ply`** in the layout the 3DGS reference implementation writes: binary
//!   little-endian, one `vertex` element with `x y z`, `f_dc_0..2`, `opacity`
//!   (logit), `scale_0..2` (log) and `rot_0..3` (`w x y z`). Other properties —
//!   normals, the higher spherical-harmonic bands — are skipped: only the
//!   view-independent colour is used.
//! - **`.splat`** (antimatter15's web viewer): 32 bytes per splat, no header —
//!   position `3×f32`, scale `3×f32` (linear), RGBA `4×u8`, rotation `4×u8`
//!   (`w x y z`, mapped `(b − 128) / 128`).
//!
//! [`probe`] reads only enough to validate a file and count its splats, for the
//! create command; [`load`] reads everything, on a loader thread.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use glam::{Quat, Vec3};

use super::GpuSplat;

/// Zeroth spherical-harmonic basis constant: `f_dc` → colour is `0.5 + C0·f_dc`.
pub(crate) const SH_C0: f32 = 0.282_094_8;
const SPLAT_RECORD_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplatFormat {
    Ply,
    Splat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplatFileInfo {
    pub format: SplatFormat,
    pub count: u32,
}

#[derive(Debug)]
pub enum SplatFileError {
    NotFound(String),
    /// Readable, but not a splat file this reader understands.
    Invalid(String),
    Io(String),
}

impl std::fmt::Display for SplatFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(m) | Self::Invalid(m) | Self::Io(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for SplatFileError {}

pub(crate) fn invalid(path: &Path, why: impl std::fmt::Display) -> SplatFileError {
    SplatFileError::Invalid(format!("{}: {why}", path.display()))
}

pub(crate) fn open(path: &Path) -> Result<File, SplatFileError> {
    File::open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            SplatFileError::NotFound(format!("no such splat file: {}", path.display()))
        }
        _ => SplatFileError::Io(format!("{}: {e}", path.display())),
    })
}

fn format_of(path: &Path) -> Result<SplatFormat, SplatFileError> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("ply") => Ok(SplatFormat::Ply),
        Some("splat") => Ok(SplatFormat::Splat),
        _ => Err(invalid(path, "expected a .ply or .splat file")),
    }
}

/// Validate `path` and count its splats without reading the splats.
pub fn probe(path: &Path) -> Result<SplatFileInfo, SplatFileError> {
    let format = format_of(path)?;
    let file = open(path)?;
    let count = match format {
        SplatFormat::Ply => read_ply_header(path, &mut BufReader::new(file))?.count,
        SplatFormat::Splat => splat_count(path, &file)?,
    };
    if count == 0 {
        return Err(invalid(path, "the file holds no splats"));
    }
    Ok(SplatFileInfo { format, count })
}

/// Read every splat in `path`.
pub fn load(path: &Path) -> Result<Vec<GpuSplat>, SplatFileError> {
    let format = format_of(path)?;
    let file = open(path)?;
    let io = |e: std::io::Error| SplatFileError::Io(format!("{}: {e}", path.display()));
    match format {
        SplatFormat::Ply => {
            let mut reader = BufReader::with_capacity(1 << 20, file);
            let header = read_ply_header(path, &mut reader)?;
            let mut record = vec![0u8; header.record_bytes];
            let mut out = Vec::with_capacity(header.count as usize);
            for _ in 0..header.count {
                reader.read_exact(&mut record).map_err(io)?;
                out.push(header.decode(&record));
            }
            Ok(out)
        }
        SplatFormat::Splat => {
            let count = splat_count(path, &file)?;
            let mut reader = BufReader::with_capacity(1 << 20, file);
            let mut record = [0u8; SPLAT_RECORD_BYTES];
            let mut out = Vec::with_capacity(count as usize);
            for _ in 0..count {
                reader.read_exact(&mut record).map_err(io)?;
                out.push(decode_splat_record(&record));
            }
            Ok(out)
        }
    }
}

fn splat_count(path: &Path, file: &File) -> Result<u32, SplatFileError> {
    let len = file
        .metadata()
        .map_err(|e| SplatFileError::Io(format!("{}: {e}", path.display())))?
        .len() as usize;
    if len == 0 || !len.is_multiple_of(SPLAT_RECORD_BYTES) {
        return Err(invalid(
            path,
            format!("a .splat file is a multiple of {SPLAT_RECORD_BYTES} bytes, this one is {len}"),
        ));
    }
    u32::try_from(len / SPLAT_RECORD_BYTES).map_err(|_| invalid(path, "too many splats"))
}

fn decode_splat_record(r: &[u8; SPLAT_RECORD_BYTES]) -> GpuSplat {
    let f = |i: usize| f32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]]);
    let unit = |b: u8| (f32::from(b) - 128.0) / 128.0;
    let byte = |b: u8| f32::from(b) / 255.0;
    GpuSplat::new(
        Vec3::new(f(0), f(4), f(8)),
        Vec3::new(f(12), f(16), f(20)),
        // Stored w, x, y, z.
        Quat::from_xyzw(unit(r[29]), unit(r[30]), unit(r[31]), unit(r[28])),
        [byte(r[24]), byte(r[25]), byte(r[26]), byte(r[27])],
    )
}

// ── PLY ───────────────────────────────────────────────────────────────────────

/// The properties a splat needs, as offsets of `f32`s into one vertex record.
pub(crate) struct PlyHeader {
    pub(crate) count: u32,
    pub(crate) record_bytes: usize,
    /// Every vertex property: name, byte offset, and whether it is an `f32`.
    pub(crate) props: Vec<(String, usize, bool)>,
    pub(crate) position: [usize; 3],
    pub(crate) f_dc: [usize; 3],
    pub(crate) opacity: usize,
    pub(crate) scale: [usize; 3],
    pub(crate) rot: [usize; 4],
}

impl PlyHeader {
    fn decode(&self, r: &[u8]) -> GpuSplat {
        let f = |i: usize| f32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]]);
        let dc = self.f_dc.map(|i| 0.5 + SH_C0 * f(i));
        let alpha = 1.0 / (1.0 + (-f(self.opacity)).exp());
        GpuSplat::new(
            Vec3::from_array(self.position.map(f)),
            Vec3::from_array(self.scale.map(|i| f(i).exp())),
            Quat::from_xyzw(
                f(self.rot[1]),
                f(self.rot[2]),
                f(self.rot[3]),
                f(self.rot[0]),
            ),
            [dc[0], dc[1], dc[2], alpha],
        )
    }
}

fn ply_type_bytes(ty: &str) -> Option<usize> {
    Some(match ty {
        "char" | "uchar" | "int8" | "uint8" => 1,
        "short" | "ushort" | "int16" | "uint16" => 2,
        "int" | "uint" | "int32" | "uint32" | "float" | "float32" => 4,
        "double" | "float64" => 8,
        _ => return None,
    })
}

pub(crate) fn read_ply_header(
    path: &Path,
    reader: &mut impl BufRead,
) -> Result<PlyHeader, SplatFileError> {
    let mut line = String::new();
    let mut next_line = |line: &mut String| -> Result<(), SplatFileError> {
        line.clear();
        // Header lines are short ASCII; bounding each read keeps a binary file
        // with no newline from being read whole as one "line".
        let n = reader
            .by_ref()
            .take(4096)
            .read_line(line)
            .map_err(|_| invalid(path, "not a PLY file (unreadable header)"))?;
        if n == 0 {
            return Err(invalid(path, "PLY header ends before end_header"));
        }
        Ok(())
    };

    next_line(&mut line)?;
    if line.trim_end() != "ply" {
        return Err(invalid(path, "not a PLY file"));
    }

    let mut count = None;
    let mut in_vertex = false;
    let mut offset = 0usize;
    let mut props: Vec<(String, usize, bool)> = Vec::new();
    loop {
        next_line(&mut line)?;
        let mut words = line.split_whitespace();
        match words.next() {
            Some("format") => {
                if words.next() != Some("binary_little_endian") {
                    return Err(invalid(path, "only binary_little_endian PLY is supported"));
                }
            }
            Some("element") => {
                let name = words.next().unwrap_or_default();
                if count.is_some() {
                    // Elements after the vertices are never read.
                    in_vertex = false;
                    continue;
                }
                if name != "vertex" {
                    return Err(invalid(path, "the vertex element must come first"));
                }
                let n = words.next().and_then(|n| n.parse::<u32>().ok());
                count = Some(n.ok_or_else(|| invalid(path, "bad vertex count"))?);
                in_vertex = true;
            }
            Some("property") if in_vertex => {
                let ty = words.next().unwrap_or_default();
                if ty == "list" {
                    return Err(invalid(
                        path,
                        "list properties on vertices are not supported",
                    ));
                }
                let bytes = ply_type_bytes(ty)
                    .ok_or_else(|| invalid(path, format!("unknown PLY type {ty}")))?;
                let name = words.next().unwrap_or_default().to_string();
                props.push((name, offset, bytes == 4 && ty.starts_with("float")));
                offset += bytes;
            }
            Some("end_header") => break,
            _ => {}
        }
    }

    let count = count.ok_or_else(|| invalid(path, "no vertex element"))?;
    let find = |name: &str| -> Result<usize, SplatFileError> {
        match props.iter().find(|(n, _, _)| n == name) {
            Some((_, off, true)) => Ok(*off),
            Some(_) => Err(invalid(path, format!("property {name} must be float"))),
            None => Err(invalid(
                path,
                format!("not a Gaussian splat PLY: no `{name}` property"),
            )),
        }
    };
    let position = [find("x")?, find("y")?, find("z")?];
    let f_dc = [find("f_dc_0")?, find("f_dc_1")?, find("f_dc_2")?];
    let opacity = find("opacity")?;
    let scale = [find("scale_0")?, find("scale_1")?, find("scale_2")?];
    let rot = [
        find("rot_0")?,
        find("rot_1")?,
        find("rot_2")?,
        find("rot_3")?,
    ];
    Ok(PlyHeader {
        count,
        record_bytes: offset,
        props,
        position,
        f_dc,
        opacity,
        scale,
        rot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vstimd-splat-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::File::create(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        path
    }

    fn ply(extra_leading_prop: bool, splats: &[[f32; 14]]) -> Vec<u8> {
        let mut h = String::from("ply\nformat binary_little_endian 1.0\n");
        h += &format!("element vertex {}\n", splats.len());
        if extra_leading_prop {
            h += "property uchar flag\n";
        }
        for p in [
            "x", "y", "z", "f_dc_0", "f_dc_1", "f_dc_2", "opacity", "scale_0", "scale_1",
            "scale_2", "rot_0", "rot_1", "rot_2", "rot_3",
        ] {
            h += &format!("property float {p}\n");
        }
        h += "end_header\n";
        let mut out = h.into_bytes();
        for s in splats {
            if extra_leading_prop {
                out.push(7);
            }
            for v in s {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out
    }

    #[test]
    fn reads_a_reference_ply() {
        // Position (1,2,3); f_dc 0 → grey 0.5; opacity logit 0 → 0.5;
        // log-scale 0 → 1; identity rotation (w = 1).
        let s = [
            1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
        ];
        let path = temp("a.ply", &ply(true, &[s, s]));
        assert_eq!(
            probe(&path).unwrap(),
            SplatFileInfo {
                format: SplatFormat::Ply,
                count: 2
            }
        );
        let splats = load(&path).unwrap();
        assert_eq!(splats.len(), 2);
        assert_eq!(splats[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(splats[0].cov_a, [1.0, 0.0, 0.0]);
        assert_eq!(splats[0].color, 0x8080_8080);
    }

    #[test]
    fn a_point_cloud_ply_is_not_a_splat_ply() {
        let bytes = b"ply\nformat binary_little_endian 1.0\nelement vertex 1\nproperty float x\nproperty float y\nproperty float z\nend_header\n\0\0\0\0\0\0\0\0\0\0\0\0";
        let path = temp("points.ply", bytes);
        let e = probe(&path).unwrap_err();
        assert!(matches!(e, SplatFileError::Invalid(_)), "{e}");
    }

    #[test]
    fn reads_a_splat_file() {
        let mut r = [0u8; 32];
        r[0..4].copy_from_slice(&5.0f32.to_le_bytes());
        for i in [12, 16, 20] {
            r[i..i + 4].copy_from_slice(&2.0f32.to_le_bytes());
        }
        r[24..28].copy_from_slice(&[255, 0, 0, 255]);
        r[28..32].copy_from_slice(&[255, 128, 128, 128]); // ≈ identity
        let path = temp("a.splat", &r);
        assert_eq!(probe(&path).unwrap().count, 1);
        let s = load(&path).unwrap()[0];
        assert_eq!(s.position, [5.0, 0.0, 0.0]);
        assert_eq!(s.color, 0xff00_00ff);
        assert!((s.cov_b[2] - 4.0).abs() < 1e-3);
    }

    #[test]
    fn missing_and_misnamed_files_are_told_apart() {
        assert!(matches!(
            probe(Path::new("/nonexistent/room.splat")),
            Err(SplatFileError::NotFound(_))
        ));
        assert!(matches!(
            probe(Path::new("room.obj")),
            Err(SplatFileError::Invalid(_))
        ));
        let path = temp("short.splat", &[0u8; 31]);
        assert!(matches!(probe(&path), Err(SplatFileError::Invalid(_))));
    }
}
