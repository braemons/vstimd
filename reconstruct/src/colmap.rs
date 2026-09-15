//! COLMAP's sparse model in its text format: the registered images and the 3-D
//! points, which is all alignment needs. Camera intrinsics are not read.
//!
//! COLMAP writes binary models; the `sfm` stage converts the chosen one to text
//! with `model_converter`, which keeps this reader trivial and the test fixtures
//! readable.
//!
//! Conventions (COLMAP's): an image stores the world-to-camera rotation as a
//! quaternion `QW QX QY QZ` and translation `t`, so the camera centre is
//! `−Rᵀt`. The camera looks down its +Z with +Y pointing *down* the image.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, bail};
use glam::{DMat3, DQuat, DVec3};

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub id: u32,
    pub name: String,
    /// World → camera.
    pub rotation: DQuat,
    pub translation: DVec3,
}

impl Image {
    fn cam_to_world(&self) -> DMat3 {
        DMat3::from_quat(self.rotation).transpose()
    }

    pub fn center(&self) -> DVec3 {
        -(self.cam_to_world() * self.translation)
    }

    /// The image's "up" (−Y in camera space) in world coordinates.
    pub fn up(&self) -> DVec3 {
        self.cam_to_world() * DVec3::NEG_Y
    }

    /// The viewing direction (+Z in camera space) in world coordinates.
    pub fn view(&self) -> DVec3 {
        self.cam_to_world() * DVec3::Z
    }

    /// The image a camera at `center`, looking along `view` with `up` up, would store.
    pub fn looking(id: u32, name: &str, center: DVec3, view: DVec3, up: DVec3) -> Self {
        let z = view.normalize();
        let x = (-up).cross(z).normalize();
        let y = z.cross(x);
        // Columns are the camera axes in world space: camera → world.
        let cam_to_world = DMat3::from_cols(x, y, z);
        let rotation = DQuat::from_mat3(&cam_to_world.transpose());
        Self {
            id,
            name: name.into(),
            rotation,
            translation: -(cam_to_world.transpose() * center),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub position: DVec3,
    /// Mean reprojection error, pixels.
    pub error: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    /// Sorted by name, which is capture order after `ingest`.
    pub images: Vec<Image>,
    pub points: Vec<Point>,
}

impl Model {
    pub fn read_text(dir: &Path) -> anyhow::Result<Self> {
        let images = read(dir, "images.txt")?;
        let points = read(dir, "points3D.txt")?;
        let mut model = Model {
            images: parse_images(&images).context("images.txt")?,
            points: parse_points(&points).context("points3D.txt")?,
        };
        model.images.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(model)
    }

    /// Only for fixtures: enough of a text model for [`Model::read_text`].
    pub fn write_text(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(
            dir.join("cameras.txt"),
            "# fixture\n1 PINHOLE 640 480 500 500 320 240\n",
        )?;
        let mut s = String::from("# Image list with two lines of data per image:\n");
        for i in &self.images {
            let (q, t) = (i.rotation, i.translation);
            let _ = writeln!(
                s,
                "{} {} {} {} {} {} {} {} 1 {}\n",
                i.id, q.w, q.x, q.y, q.z, t.x, t.y, t.z, i.name
            );
        }
        std::fs::write(dir.join("images.txt"), s)?;
        let mut s = String::from("# 3D point list\n");
        for (k, p) in self.points.iter().enumerate() {
            let v = p.position;
            let _ = writeln!(
                s,
                "{} {} {} {} 128 128 128 {} 1 0 2 0",
                k + 1,
                v.x,
                v.y,
                v.z,
                p.error
            );
        }
        std::fs::write(dir.join("points3D.txt"), s)
    }

    pub fn mean_reprojection_error_px(&self) -> Option<f64> {
        (!self.points.is_empty())
            .then(|| self.points.iter().map(|p| p.error).sum::<f64>() / self.points.len() as f64)
    }
}

fn read(dir: &Path, name: &str) -> anyhow::Result<String> {
    let path = dir.join(name);
    std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
}

fn numbers<const N: usize>(fields: &[&str], line_no: usize) -> anyhow::Result<[f64; N]> {
    let mut out = [0.0; N];
    for (o, f) in out.iter_mut().zip(fields) {
        *o = f
            .parse()
            .with_context(|| format!("line {line_no}: bad number {f:?}"))?;
    }
    Ok(out)
}

fn parse_images(text: &str) -> anyhow::Result<Vec<Image>> {
    // Two lines per image, the second (its 2-D points) possibly empty — so
    // blank lines count, and only comments are skipped.
    let mut lines = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.starts_with('#'));
    let mut images = Vec::new();
    while let Some((n, line)) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            bail!(
                "line {}: expected IMAGE_ID QW QX QY QZ TX TY TZ CAMERA_ID NAME",
                n + 1
            );
        }
        let [id, qw, qx, qy, qz, tx, ty, tz] = numbers::<8>(&fields, n + 1)?;
        images.push(Image {
            id: id as u32,
            // A name may contain spaces.
            name: fields[9..].join(" "),
            rotation: DQuat::from_xyzw(qx, qy, qz, qw).normalize(),
            translation: DVec3::new(tx, ty, tz),
        });
        lines.next(); // POINTS2D
    }
    Ok(images)
}

fn parse_points(text: &str) -> anyhow::Result<Vec<Point>> {
    let mut points = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 8 {
            bail!("line {}: expected POINT3D_ID X Y Z R G B ERROR", n + 1);
        }
        let [_, x, y, z, _, _, _, error] = numbers::<8>(&fields, n + 1)?;
        points.push(Point {
            position: DVec3::new(x, y, z),
            error,
        });
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_camera_round_trips_its_pose() {
        let c = DVec3::new(1.0, 2.0, 3.0);
        let view = DVec3::new(0.0, 0.0, -1.0);
        let i = Image::looking(1, "a.jpg", c, view, DVec3::Y);
        assert!((i.center() - c).length() < 1e-12);
        assert!((i.view() - view).length() < 1e-12);
        assert!((i.up() - DVec3::Y).length() < 1e-12);
    }

    #[test]
    fn reads_what_colmap_writes() {
        let text = "# Image list with two lines of data per image:\n\
                    #   IMAGE_ID, QW, QX, QY, QZ, TX, TY, TZ, CAMERA_ID, NAME\n\
                    # Number of images: 2\n\
                    2 1 0 0 0 0 0 -5 1 00002.jpg\n\
                    \n\
                    1 1 0 0 0 0 0 0 1 00001.jpg\n\
                    10.5 20.5 7 11.0 21.0 -1\n";
        let images = parse_images(text).unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[1].name, "00001.jpg");
        assert_eq!(images[0].center(), DVec3::new(0.0, 0.0, 5.0));

        let points = parse_points("# 3D point list\n7 1.5 2 3 255 0 0 0.25 1 0 2 3\n").unwrap();
        assert_eq!(
            points,
            vec![Point {
                position: DVec3::new(1.5, 2.0, 3.0),
                error: 0.25
            }]
        );
    }

    #[test]
    fn written_fixtures_read_back() {
        let dir = std::env::temp_dir().join(format!("reconstruct-colmap-{}", std::process::id()));
        let model = Model {
            images: vec![
                Image::looking(1, "00001.jpg", DVec3::ZERO, DVec3::NEG_Z, DVec3::Y),
                Image::looking(
                    2,
                    "00002.jpg",
                    DVec3::new(0.0, 0.0, -1.0),
                    DVec3::NEG_Z,
                    DVec3::Y,
                ),
            ],
            points: vec![Point {
                position: DVec3::ONE,
                error: 0.5,
            }],
        };
        model.write_text(&dir).unwrap();
        let back = Model::read_text(&dir).unwrap();
        assert_eq!(back.images.len(), 2);
        assert!((back.images[1].center() - model.images[1].center()).length() < 1e-9);
        assert_eq!(back.mean_reprojection_error_px(), Some(0.5));
    }
}
