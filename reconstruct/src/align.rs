//! Moving a reconstruction into vstimd's world: centimetres, the floor at
//! `y = 0`, the corridor running down −Z from the origin, +X to the right.
//!
//! Structure-from-motion has no idea which way is up, how large anything is or
//! where the corridor starts. All of that is estimated from the capture itself,
//! under one protocol: the corridor was walked once, start to end, with the
//! camera upright and looking forward, and the distance walked was measured.
//! The steps and thresholds are `dev/design/SPLAT_RECONSTRUCTION_PLAN.md` §6.
//!
//! Everything here is a deterministic, pure function of the camera poses and sparse points,
//! so it is tested on synthetic captures rather than real ones.

use anyhow::bail;
use glam::{DMat3, DQuat, DVec3, Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::colmap::Model;
use crate::params::{Alignment as AlignmentParams, Crop};

/// Cameras whose up vectors spread by more than this (median angle to their
/// mean) were not held upright; the estimate is refused.
const MAX_UP_SPREAD_DEG: f64 = 20.0;
/// A floor still tilting further than this from up after levelling is reported.
const MAX_FLOOR_TILT_DEG: f64 = 5.0;
/// Sideways spread of the path relative to its length above which the capture
/// is reported as not a corridor walk.
const MAX_SIDEWAYS_RATIO: f64 = 0.3;
const MIN_FLOOR_INLIERS: usize = 20;
/// Share of floor points that must lie in the middle half of the corridor's width.
const MIN_FLOOR_MIDDLE_SHARE: f64 = 0.2;
/// Levelling by the floor stops once up moves less than this.
const UP_CONVERGED_DEG: f64 = 0.05;
const MAX_UP_ITERATIONS: usize = 10;
/// A floor further than this from the cameras' up is taken for something else.
const MAX_UP_CORRECTION_DEG: f64 = 40.0;

/// The estimated similarity transform and what it was estimated from.
/// Written to `alignment.json`; the report copies it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alignment {
    /// `aligned_cm = scale · rotation · model + translation_cm`.
    pub scale: f32,
    /// Quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    pub translation_cm: [f32; 3],

    pub images_registered: usize,
    pub points: usize,
    pub mean_reprojection_error_px: Option<f64>,
    /// Median angle between each camera's up and the cameras' mean up.
    pub up_spread_deg: f64,
    /// How far levelling by the floor moved up away from the cameras' mean up —
    /// roughly how far the camera was pitched while capturing.
    pub up_correction_deg: f64,
    /// Sideways spread of the camera path over its spread along the corridor.
    pub sideways_ratio: f64,
    pub path_length_cm: f32,
    /// Height of the cameras above the fitted floor (median), or the given one.
    pub capture_height_cm: f32,
    pub floor: Option<FloorFit>,
    pub crop: CropBox,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FloorFit {
    pub inliers: usize,
    pub candidates: usize,
    /// Angle between the fitted plane and the final up: what levelling left.
    pub tilt_deg: f64,
    /// From the fit, before any given `capture_height_cm` is considered.
    pub capture_height_cm: f32,
}

/// Axis-aligned, in the aligned frame. Splats outside are dropped.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CropBox {
    pub min_cm: [f32; 3],
    pub max_cm: [f32; 3],
}

impl CropBox {
    pub fn contains(&self, p: Vec3) -> bool {
        p.cmpge(Vec3::from_array(self.min_cm)).all() && p.cmple(Vec3::from_array(self.max_cm)).all()
    }
}

impl Alignment {
    pub fn rotation_quat(&self) -> Quat {
        Quat::from_array(self.rotation)
    }

    pub fn apply(&self, p: Vec3) -> Vec3 {
        self.scale * (self.rotation_quat() * p) + Vec3::from_array(self.translation_cm)
    }
}

pub fn estimate(model: &Model, params: &AlignmentParams, crop: &Crop) -> anyhow::Result<Alignment> {
    let cams = &model.images;
    if cams.len() < 3 {
        bail!(
            "only {} images were registered; at least 3 are needed to align",
            cams.len()
        );
    }
    let mut warnings = Vec::new();

    // 1. Up, first guess: the mean of the cameras' up vectors. A camera held
    //    pitched down tilts every one of them the same way, which no spread
    //    check can see, so the floor corrects it below.
    let camera_up = cams
        .iter()
        .map(|c| c.up())
        .sum::<DVec3>()
        .normalize_or_zero();
    if camera_up == DVec3::ZERO {
        bail!("the cameras' up directions cancel out; was the camera held upright?");
    }
    let up_spread_deg = median(cams.iter().map(|c| angle_deg(c.up(), camera_up)).collect());
    if up_spread_deg > MAX_UP_SPREAD_DEG {
        bail!(
            "the cameras disagree on which way is up by a median of {up_spread_deg:.0}° \
             (at most {MAX_UP_SPREAD_DEG:.0}°); the capture must be taken with the camera upright"
        );
    }

    // 2 and 3, alternating: the walk and the floor under the current up. The
    // floor's normal is the next up, until the two agree.
    let points: Vec<DVec3> = model.points.iter().map(|p| p.position).collect();
    let mut up = camera_up;
    let mut walk = walk_along(cams, up)?;
    let mut floor = None;
    for _ in 0..MAX_UP_ITERATIONS {
        let Some(fit) = fit_floor(&points, cams, up, walk.forward, 0.01 * walk.length) else {
            break;
        };
        let (normal, change) = (fit.normal, angle_deg(fit.normal, up));
        floor = Some((up, walk, fit));
        if change < UP_CONVERGED_DEG {
            break;
        }
        if angle_deg(normal, camera_up) > MAX_UP_CORRECTION_DEG {
            warnings.push(format!(
                "the floor found is {:.0}° off the cameras' up; the floor fit is not trusted to level the scene",
                angle_deg(normal, camera_up)
            ));
            floor = None;
            break;
        }
        match walk_along(cams, normal) {
            Ok(next) => (up, walk) = (normal, next),
            Err(_) => break,
        }
    }
    let (up, walk, floor) = match floor {
        Some((up, walk, fit)) => (up, walk, Some(fit)),
        None => (camera_up, walk_along(cams, camera_up)?, None),
    };
    let up_correction_deg = angle_deg(up, camera_up);
    let forward = walk.forward;
    let path_model = walk.length;
    let sideways_ratio = walk.sideways_ratio;
    if sideways_ratio > MAX_SIDEWAYS_RATIO {
        warnings.push(format!(
            "the camera path spreads {:.0} % as far sideways as along the corridor; \
             this does not look like one straight walk",
            100.0 * sideways_ratio
        ));
    }
    let horizontal = |v: DVec3| v - up * v.dot(up);
    let facing = median(
        cams.iter()
            .map(|c| horizontal(c.view()).normalize_or_zero().dot(forward))
            .collect(),
    );
    if facing < -0.5 {
        warnings.push("most photos look back against the walking direction".into());
    } else if facing < 0.5 {
        warnings.push(
            "most photos look sideways rather than down the corridor; the scene will be \
             sharpest where the camera looked"
                .into(),
        );
    }

    // The frame: Y up, −Z forward, X = Y × Z to the right.
    let y = up;
    let z = -forward;
    let x = y.cross(z);
    let rotation = DMat3::from_cols(x, y, z).transpose();

    // 5. Scale (needed to fall back on a given capture height in step 3).
    let path_length_cm = f64::from(params.capture_path_length_cm);
    let scale = path_length_cm / path_model;

    // 3. The floor, as fitted above.
    let camera_height = median(cams.iter().map(|c| c.center().dot(up)).collect());
    let floor = floor.map(|fit| {
        let summary = FloorFit {
            inliers: fit.inliers,
            candidates: fit.candidates,
            tilt_deg: angle_deg(fit.normal, up),
            capture_height_cm: (scale * (camera_height - fit.height)) as f32,
        };
        (fit.height, summary)
    });
    let (floor_height, capture_height_cm) = match (&floor, params.capture_height_cm) {
        (Some((h, fit)), given) => {
            if fit.tilt_deg > MAX_FLOOR_TILT_DEG {
                warnings.push(format!(
                    "the floor still tilts {:.0}° from up after levelling; the scene may look tilted",
                    fit.tilt_deg
                ));
            }
            if let Some(given) = given {
                let off = (fit.capture_height_cm - given).abs() / given;
                if off > 0.1 {
                    warnings.push(format!(
                        "the floor fit puts the camera {:.0} cm above the floor, but \
                         capture_height_cm is {given:.0} cm ({:.0} % apart): the walked \
                         distance, the given height or the floor fit is wrong",
                        fit.capture_height_cm,
                        100.0 * off
                    ));
                }
            }
            (*h, fit.capture_height_cm)
        }
        (None, Some(given)) => {
            warnings.push(
                "no floor was found; the floor is placed capture_height_cm below the cameras"
                    .into(),
            );
            (camera_height - f64::from(given) / scale, given)
        }
        (None, None) => bail!(
            "no floor was found below the cameras; pass --capture-height-cm to place it from \
             the camera height instead (the scene is then levelled by the cameras alone)"
        ),
    };

    // 4. Origin: the first camera, dropped onto the floor.
    let first = cams[0].center();
    let origin = first - up * (first.dot(up) - floor_height);
    let translation = -scale * (rotation * origin);

    let to_aligned = |p: DVec3| (scale * (rotation * p) + translation).as_vec3();
    let aligned: Vec<Vec3> = model
        .points
        .iter()
        .map(|p| to_aligned(p.position))
        .collect();
    let crop = crop_box(&aligned, path_length_cm as f32, capture_height_cm, crop)?;

    Ok(Alignment {
        scale: scale as f32,
        rotation: DQuat::from_mat3(&rotation).normalize().as_quat().to_array(),
        translation_cm: translation.as_vec3().to_array(),
        images_registered: cams.len(),
        points: model.points.len(),
        mean_reprojection_error_px: model.mean_reprojection_error_px(),
        up_spread_deg,
        up_correction_deg,
        sideways_ratio,
        path_length_cm: path_length_cm as f32,
        capture_height_cm,
        floor: floor.map(|(_, fit)| fit),
        crop,
        warnings,
    })
}

/// Step 6, the crop box: x and y from the 5th–95th percentile of the sparse
/// points along the corridor, z from the path, each widened by the margin.
/// Given extents replace the estimates.
fn crop_box(
    points: &[Vec3],
    path_length_cm: f32,
    capture_height_cm: f32,
    crop: &Crop,
) -> anyhow::Result<CropBox> {
    let m = crop.margin_cm;
    let length = crop.length_cm.unwrap_or(path_length_cm);
    let along: Vec<Vec3> = points
        .iter()
        .copied()
        .filter(|p| p.z <= m && p.z >= -length - m && p.y >= -m)
        .collect();
    if (crop.width_cm.is_none() || crop.height_cm.is_none()) && along.len() < 10 {
        bail!(
            "only {} sparse points lie along the corridor; pass --crop-width-cm and --crop-height-cm",
            along.len()
        );
    }
    let spread = |axis: fn(&Vec3) -> f32, q: f64| {
        percentile(along.iter().map(|p| f64::from(axis(p))).collect(), q) as f32
    };
    let (x_lo, x_hi) = match crop.width_cm {
        Some(w) => (-w / 2.0, w / 2.0),
        None => (spread(|p| p.x, 0.05), spread(|p| p.x, 0.95)),
    };
    let y_hi = match crop.height_cm {
        Some(h) => h,
        None => spread(|p| p.y, 0.95).max(capture_height_cm),
    };
    Ok(CropBox {
        min_cm: [x_lo - m, -m, -length - m],
        max_cm: [x_hi + m, y_hi + m, m],
    })
}

/// The unit direction of greatest spread of `points` in the plane across `up`,
/// and the ratio of the smaller spread to the larger (standard deviations).
fn principal_direction(points: &[DVec3], up: DVec3) -> (DVec3, f64) {
    let e1 = up.any_orthonormal_vector();
    let e2 = up.cross(e1);
    let mean = points.iter().sum::<DVec3>() / points.len() as f64;
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    for p in points {
        let d = *p - mean;
        let (a, b) = (d.dot(e1), d.dot(e2));
        sxx += a * a;
        sxy += a * b;
        syy += b * b;
    }
    // Eigen-decomposition of the symmetric 2×2 [[sxx, sxy], [sxy, syy]].
    let half_trace = (sxx + syy) / 2.0;
    let root = (((sxx - syy) / 2.0).powi(2) + sxy * sxy).sqrt();
    let (l1, l2) = (half_trace + root, (half_trace - root).max(0.0));
    let angle = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let dir = e1 * angle.cos() + e2 * angle.sin();
    let ratio = if l1 > 0.0 { (l2 / l1).sqrt() } else { 1.0 };
    (dir, ratio)
}

#[derive(Clone, Copy, Debug)]
struct Walk {
    /// Unit, across up, pointing from the first photo to the last.
    forward: DVec3,
    sideways_ratio: f64,
    /// First to last camera along `forward`, model units.
    length: f64,
}

/// Step 2: the principal direction of the camera path across `up`.
fn walk_along(cams: &[crate::colmap::Image], up: DVec3) -> anyhow::Result<Walk> {
    let horizontal = |v: DVec3| v - up * v.dot(up);
    let centres: Vec<DVec3> = cams.iter().map(|c| horizontal(c.center())).collect();
    let (forward, sideways_ratio) = principal_direction(&centres, up);
    let path = centres[centres.len() - 1] - centres[0];
    let forward = if path.dot(forward) < 0.0 {
        -forward
    } else {
        forward
    };
    let length = path.dot(forward);
    if length <= 1e-9 * centres.iter().map(|c| c.length()).fold(1.0, f64::max) {
        bail!(
            "the first and last photos were taken at the same place; walk the corridor once, start to end"
        );
    }
    Ok(Walk {
        forward,
        sideways_ratio,
        length,
    })
}

struct Floor {
    /// Along the `up` the floor was searched with.
    height: f64,
    /// Of the fitted plane, on the cameras' side.
    normal: DVec3,
    inliers: usize,
    candidates: usize,
}

/// Step 3: the lowest dense layer of points below the cameras along `up`,
/// spread across the corridor rather than bunched at its sides (the bottom rows
/// of wall texture are dense too, but sit in two lines), with a plane fitted
/// through it.
///
/// Not a free RANSAC plane: a free plane happily tilts to collect floor and
/// wall rows together. The band is searched along `up`, and the plane fit then
/// says how far `up` is off; the caller iterates.
fn fit_floor(
    points: &[DVec3],
    cams: &[crate::colmap::Image],
    up: DVec3,
    forward: DVec3,
    tolerance: f64,
) -> Option<Floor> {
    let lowest_camera = cams
        .iter()
        .map(|c| c.center().dot(up))
        .fold(f64::INFINITY, f64::min);
    let candidates: Vec<DVec3> = points
        .iter()
        .copied()
        .filter(|p| p.dot(up) < lowest_camera)
        .collect();
    if candidates.len() < MIN_FLOOR_INLIERS {
        return None;
    }
    let mut heights: Vec<f64> = candidates.iter().map(|p| p.dot(up)).collect();
    heights.sort_by(f64::total_cmp);
    // Points within one band 2·tolerance thick, starting at each height.
    let window = |i: usize| heights[i..].partition_point(|h| *h <= heights[i] + 2.0 * tolerance);
    let densest = (0..heights.len()).map(window).max()?;
    let threshold = MIN_FLOOR_INLIERS.max(densest / 2);
    let start = (0..heights.len()).find(|&i| window(i) >= threshold)?;
    let band = &heights[start..start + window(start)];
    let floor = band[band.len() / 2];

    // A plane through the band, refined on its own residuals. The first slab
    // is thicker, so a floor seen at an angle is not cut to a strip.
    let across = up.cross(forward);
    let mut inliers: Vec<DVec3> = candidates
        .iter()
        .copied()
        .filter(|p| (p.dot(up) - floor).abs() <= 3.0 * tolerance)
        .collect();
    let mut plane = None;
    for _ in 0..3 {
        let (normal, offset) = fit_plane(&inliers, up, forward, across)?;
        inliers = candidates
            .iter()
            .copied()
            .filter(|p| (p.dot(normal) - offset).abs() <= tolerance)
            .collect();
        if inliers.len() < MIN_FLOOR_INLIERS {
            return None;
        }
        plane = Some(normal);
    }
    let normal = plane?;

    let widths: Vec<f64> = inliers.iter().map(|p| p.dot(across)).collect();
    let (lo, hi) = (
        percentile(widths.clone(), 0.05),
        percentile(widths.clone(), 0.95),
    );
    let (mid_lo, mid_hi) = (lo + 0.25 * (hi - lo), hi - 0.25 * (hi - lo));
    let middle = widths
        .iter()
        .filter(|w| **w > mid_lo && **w < mid_hi)
        .count();
    if (middle as f64) < MIN_FLOOR_MIDDLE_SHARE * inliers.len() as f64 {
        return None;
    }
    Some(Floor {
        height: median(inliers.iter().map(|p| p.dot(up)).collect()),
        normal,
        inliers: inliers.len(),
        candidates: candidates.len(),
    })
}

/// Least-squares `height = c + a·across + b·forward` over `points`, as a unit
/// normal (on `up`'s side) and offset: the plane is `dot(normal, p) = offset`.
fn fit_plane(points: &[DVec3], up: DVec3, forward: DVec3, across: DVec3) -> Option<(DVec3, f64)> {
    if points.len() < 3 {
        return None;
    }
    let n = points.len() as f64;
    let coords: Vec<(f64, f64, f64)> = points
        .iter()
        .map(|p| (p.dot(across), p.dot(forward), p.dot(up)))
        .collect();
    let mean = coords.iter().fold((0.0, 0.0, 0.0), |m, c| {
        (m.0 + c.0 / n, m.1 + c.1 / n, m.2 + c.2 / n)
    });
    let (mut suu, mut suv, mut svv, mut suh, mut svh) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (u, v, h) in coords {
        let (u, v, h) = (u - mean.0, v - mean.1, h - mean.2);
        suu += u * u;
        suv += u * v;
        svv += v * v;
        suh += u * h;
        svh += v * h;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-12 * (suu + svv).powi(2).max(1e-300) {
        return None;
    }
    let a = (suh * svv - svh * suv) / det;
    let b = (svh * suu - suh * suv) / det;
    let c = mean.2 - a * mean.0 - b * mean.1;
    let raw = up - a * across - b * forward;
    let len = raw.length();
    Some((raw / len, c / len))
}

fn angle_deg(a: DVec3, b: DVec3) -> f64 {
    a.normalize_or_zero()
        .dot(b.normalize_or_zero())
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

fn median(v: Vec<f64>) -> f64 {
    percentile(v, 0.5)
}

fn percentile(mut v: Vec<f64>, q: f64) -> f64 {
    assert!(!v.is_empty());
    v.sort_by(f64::total_cmp);
    let i = ((v.len() - 1) as f64 * q).round() as usize;
    v[i]
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::colmap::{Image, Point};

    struct XorShift(u64);

    impl XorShift {
        fn below(&mut self, n: usize) -> usize {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 % n as u64) as usize
        }
    }

    /// A 100 cm wide, 100 cm high corridor walked 500 cm down −Z at 30 cm,
    /// in vstimd's frame, then moved by `to_model` the way SfM would see it.
    pub(crate) fn synthetic_capture(
        to_model: impl Fn(DVec3) -> DVec3,
        to_model_dir: impl Fn(DVec3) -> DVec3,
    ) -> Model {
        let mut rng = XorShift(42);
        let mut jitter = |amount: f64| (rng.below(2001) as f64 / 1000.0 - 1.0) * amount;
        let images = (0..51)
            .map(|i| {
                let c = DVec3::new(jitter(3.0), 30.0 + jitter(2.0), -10.0 * i as f64);
                let view = DVec3::new(jitter(0.1), jitter(0.1), -1.0);
                let up = DVec3::new(jitter(0.1), 1.0, jitter(0.1));
                Image::looking(
                    i + 1,
                    &format!("{:05}.jpg", i + 1),
                    to_model(c),
                    to_model_dir(view),
                    to_model_dir(up),
                )
            })
            .collect();
        let mut points = Vec::new();
        for i in 0..600 {
            let z = 50.0 - 1.1 * i as f64;
            let t = (i % 20) as f64 / 20.0;
            points.push(DVec3::new(-50.0 + 100.0 * t, jitter(0.3), z)); // floor
            points.push(DVec3::new(-50.0, 100.0 * t, z)); // left wall
            points.push(DVec3::new(50.0, 100.0 * t, z)); // right wall
            points.push(DVec3::new(-50.0 + 100.0 * t, 100.0, z)); // ceiling
        }
        let points = points
            .into_iter()
            .map(|p| Point {
                position: to_model(p),
                error: 0.5,
            })
            .collect();
        Model { images, points }
    }

    fn params(height: Option<f32>) -> AlignmentParams {
        AlignmentParams {
            capture_path_length_cm: 500.0,
            capture_height_cm: height,
        }
    }

    #[test]
    fn recovers_a_known_similarity() {
        // SfM's frame: 1/37 the size, turned arbitrarily, somewhere else.
        let q = DQuat::from_euler(glam::EulerRot::YXZ, 2.1, -0.7, 2.9);
        let (s, t) = (1.0 / 37.0, DVec3::new(3.0, -8.0, 1.5));
        let model = synthetic_capture(|p| s * (q * p) + t, |d| q * d);
        let a = estimate(&model, &params(None), &Crop::default()).unwrap();

        let back = |p: DVec3| a.apply((s * (q * p) + t).as_vec3());
        for p in [
            DVec3::ZERO,
            DVec3::new(50.0, 100.0, -500.0),
            DVec3::new(-50.0, 0.0, -250.0),
        ] {
            let got = back(p);
            assert!((got - p.as_vec3()).length() < 3.0, "{p} → {got}");
        }
        assert!((a.capture_height_cm - 30.0).abs() < 3.0, "{a:?}");
        assert!(a.warnings.is_empty(), "{:?}", a.warnings);
        let fit = a.floor.as_ref().unwrap();
        assert!(fit.tilt_deg < 2.0 && fit.inliers >= 500, "{fit:?}");
        // The corridor's walls land inside the crop, with the margin.
        assert!(a.crop.contains(Vec3::new(-50.0, 50.0, -400.0)));
        assert!(!a.crop.contains(Vec3::new(0.0, 50.0, -700.0)));
        assert!(!a.crop.contains(Vec3::new(160.0, 50.0, -200.0)));
    }

    #[test]
    fn a_camera_pitched_down_is_levelled_by_the_floor() {
        // Every camera looks 25° below the horizon: their mean up is 25° off,
        // with no spread to give it away.
        let pitch = DQuat::from_rotation_x(-25f64.to_radians());
        let mut model = synthetic_capture(|p| p, |d| d);
        for img in &mut model.images {
            *img = Image::looking(
                img.id,
                &img.name,
                img.center(),
                pitch * img.view(),
                pitch * img.up(),
            );
        }
        let a = estimate(&model, &params(None), &Crop::default()).unwrap();
        assert!((a.up_correction_deg - 25.0).abs() < 1.5, "{a:?}");
        assert!((a.capture_height_cm - 30.0).abs() < 2.0, "{a:?}");
        for p in [
            DVec3::new(50.0, 100.0, -500.0),
            DVec3::new(-50.0, 0.0, -250.0),
        ] {
            let got = a.apply(p.as_vec3());
            assert!((got - p.as_vec3()).length() < 4.0, "{p} → {got}");
        }
        assert!(a.floor.unwrap().tilt_deg < 0.5);
    }

    #[test]
    fn a_given_capture_height_stands_in_for_a_missing_floor() {
        let mut model = synthetic_capture(|p| p, |d| d);
        model.points.retain(|p| p.position.y > 1.0);
        assert!(
            estimate(&model, &params(None), &Crop::default())
                .unwrap_err()
                .to_string()
                .contains("no floor")
        );
        let a = estimate(&model, &params(Some(30.0)), &Crop::default()).unwrap();
        assert_eq!(a.capture_height_cm, 30.0);
        assert!(a.floor.is_none());
        assert!(
            (a.apply(Vec3::new(0.0, 0.0, -100.0)) - Vec3::new(0.0, 0.0, -100.0)).length() < 3.0
        );
    }

    #[test]
    fn a_disagreeing_height_is_a_warning() {
        let model = synthetic_capture(|p| p, |d| d);
        let a = estimate(&model, &params(Some(60.0)), &Crop::default()).unwrap();
        assert!(
            a.warnings
                .iter()
                .any(|w| w.contains("capture_height_cm is 60")),
            "{:?}",
            a.warnings
        );
    }

    #[test]
    fn tilted_cameras_are_refused() {
        let mut model = synthetic_capture(|p| p, |d| d);
        for (i, img) in model.images.iter_mut().enumerate() {
            let roll = if i % 2 == 0 { 1.2 } else { -1.2 };
            *img = Image::looking(
                img.id,
                &img.name,
                img.center(),
                img.view(),
                DQuat::from_rotation_z(roll) * DVec3::Y,
            );
        }
        let e = estimate(&model, &params(None), &Crop::default()).unwrap_err();
        assert!(e.to_string().contains("upright"), "{e}");
    }

    #[test]
    fn standing_still_is_refused() {
        let mut model = synthetic_capture(|p| p, |d| d);
        for img in &mut model.images {
            *img = Image::looking(
                img.id,
                &img.name,
                DVec3::new(0.0, 30.0, 0.0),
                img.view(),
                img.up(),
            );
        }
        let e = estimate(&model, &params(None), &Crop::default()).unwrap_err();
        assert!(e.to_string().contains("same place"), "{e}");
    }

    #[test]
    fn given_extents_replace_the_estimates() {
        let model = synthetic_capture(|p| p, |d| d);
        let crop = Crop {
            width_cm: Some(40.0),
            height_cm: Some(80.0),
            length_cm: Some(200.0),
            margin_cm: 10.0,
        };
        let a = estimate(&model, &params(None), &crop).unwrap();
        assert_eq!(a.crop.min_cm, [-30.0, -10.0, -210.0]);
        assert_eq!(a.crop.max_cm, [30.0, 90.0, 10.0]);
    }
}
