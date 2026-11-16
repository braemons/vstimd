//! Static camera movement volumes, independent of render tessellation.
//! JSON contract: dev/CAMERA_MOVEMENT.md. No allocation on the movement path.
use glam::{DVec3, Vec3};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraMovement {
    pub start_position_cm: [f32; 3],
    pub start_yaw_deg: f32,
    pub clearance_cm: f32,
    /// Real milliseconds of black at a finite track end; 0 keeps legacy fades.
    #[serde(default)]
    pub end_blackout_ms: u32,
    pub obstacles: Vec<Obstacle>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Obstacle {
    Box {
        position_cm: [f32; 3],
        yaw_deg: f32,
        size_cm: [f32; 3],
    },
    Sphere {
        position_cm: [f32; 3],
        diameter_cm: f32,
    },
    Cylinder {
        position_cm: [f32; 3],
        yaw_deg: f32,
        size_cm: [f32; 3],
    },
    Diamond {
        position_cm: [f32; 3],
        yaw_deg: f32,
        size_cm: [f32; 3],
    },
}

// Intersection of open interior half spaces / quadrics with a segment.
struct Interval {
    lo: f64,
    hi: f64,
}
impl Interval {
    fn new() -> Self {
        Self {
            lo: f64::NEG_INFINITY,
            hi: f64::INFINITY,
        }
    }
    fn plane(&mut self, p: DVec3, d: DVec3, n: DVec3, bound: f64) {
        let a = n.dot(p) - bound;
        let b = n.dot(d);
        if b.abs() < 1e-12 {
            if a >= -1e-10 {
                self.hi = f64::NEG_INFINITY;
            }
        } else if b > 0.0 {
            self.hi = self.hi.min(-a / b);
        } else {
            self.lo = self.lo.max(-a / b);
        }
    }
    fn slab(&mut self, p: DVec3, d: DVec3, n: DVec3, extent: f64) {
        self.plane(p, d, n, extent);
        self.plane(p, d, -n, extent);
    }
    fn ball(&mut self, p: DVec3, d: DVec3, radius: f64) {
        let a = d.length_squared();
        let c = p.length_squared() - radius * radius;
        if a < 1e-24 {
            if c >= -1e-10 {
                self.hi = f64::NEG_INFINITY;
            }
            return;
        }
        let b = p.dot(d);
        let disc = b * b - a * c;
        if disc <= 0.0 {
            self.hi = f64::NEG_INFINITY;
            return;
        }
        let root = disc.sqrt();
        self.lo = self.lo.max((-b - root) / a);
        self.hi = self.hi.min((-b + root) / a);
    }
    fn hit(&self) -> Option<f64> {
        (self.hi > self.lo.max(0.0) + 1e-10 && self.lo < 1.0 && self.hi > 1e-10)
            .then_some(self.lo.max(0.0))
    }
}

impl Obstacle {
    fn parts(&self) -> ([f32; 3], f32, [f32; 3]) {
        match *self {
            Self::Sphere {
                position_cm,
                diameter_cm,
            } => (position_cm, 0.0, [diameter_cm; 3]),
            Self::Box {
                position_cm,
                yaw_deg,
                size_cm,
            }
            | Self::Cylinder {
                position_cm,
                yaw_deg,
                size_cm,
            }
            | Self::Diamond {
                position_cm,
                yaw_deg,
                size_cm,
            } => (position_cm, yaw_deg, size_cm),
        }
    }
    fn interval(&self, from: DVec3, delta: DVec3, clearance: f64) -> Interval {
        let (position, yaw, size) = self.parts();
        let angle = f64::from(yaw).to_radians();
        let (s, c) = angle.sin_cos();
        // Inverse of world yaw about +Y.
        let local = |v: DVec3| DVec3::new(c * v.x - s * v.z, v.y, s * v.x + c * v.z);
        let p = local(from - Vec3::from_array(position).as_dvec3());
        let d = local(delta);
        let half = Vec3::from_array(size).as_dvec3() * 0.5;
        let mut i = Interval::new();
        match self {
            Self::Sphere { .. } => i.ball(p, d, half.x + clearance),
            Self::Cylinder { .. } => {
                i.ball(
                    DVec3::new(p.x, 0.0, p.z),
                    DVec3::new(d.x, 0.0, d.z),
                    half.x + clearance,
                );
                i.slab(p, d, DVec3::Y, half.y + clearance);
            }
            Self::Diamond { .. } => {
                for x in [-1.0, 1.0] {
                    for y in [-1.0, 1.0] {
                        let n = DVec3::new(x / half.x, y / half.y, 0.0);
                        i.plane(p, d, n, 1.0 + clearance * n.length());
                    }
                }
                i.slab(p, d, DVec3::Z, half.z + clearance);
            }
            Self::Box { .. } => {
                for (n, e) in [(DVec3::X, half.x), (DVec3::Y, half.y), (DVec3::Z, half.z)] {
                    i.slab(p, d, n, e + clearance);
                }
            }
        }
        i
    }
}

impl CameraMovement {
    pub fn check_animation(
        &self,
        target: &super::AnimationTarget,
        animation: &super::animation::Animation,
    ) -> Result<(), String> {
        use super::animation::Animation;
        let wraps = match animation {
            Animation::LinearNav3D { wrap_period_cm, .. } => wrap_period_cm.is_some(),
            Animation::DeviceDrivenTransform { axes, .. } => {
                matches!(target, super::AnimationTarget::Camera)
                    && axes.iter().any(|a| a.wrap.is_some())
            }
            _ => false,
        };
        if wraps {
            Err("camera_movement cannot be combined with wrapping camera motion".into())
        } else {
            Ok(())
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let finite = |v: [f32; 3]| v.iter().all(|x| x.is_finite() && x.abs() <= 1e7);
        if !finite(self.start_position_cm)
            || !self.start_yaw_deg.is_finite()
            || !self.clearance_cm.is_finite()
            || !(0.01..=100.0).contains(&self.clearance_cm)
            || self.end_blackout_ms > 60000
            || self.obstacles.len() > 3000
        {
            return Err("invalid camera_movement origin, margin or obstacle count".into());
        }
        let start = Vec3::from_array(self.start_position_cm).as_dvec3();
        for obstacle in &self.obstacles {
            let (p, yaw, size) = obstacle.parts();
            if !finite(p)
                || !yaw.is_finite()
                || !finite(size)
                || size.iter().any(|x| *x <= 0.0)
                || matches!(obstacle, Obstacle::Cylinder { .. }) && size[0] != size[2]
            {
                return Err("invalid camera_movement obstacle".into());
            }
            if obstacle
                .interval(start, DVec3::ZERO, f64::from(self.clearance_cm))
                .hit()
                .is_some()
            {
                return Err(
                    "camera_movement start overlaps an obstacle (including clearance)".into(),
                );
            }
        }
        Ok(())
    }

    /// Stop at the first obstruction; consume rejected input without storing debt.
    pub fn constrain(&self, from: DVec3, to: DVec3) -> DVec3 {
        let delta = to - from;
        if !from.is_finite() || !to.is_finite() {
            return from;
        }
        let length = delta.length();
        if length < 1e-12 {
            return from;
        }
        let yaw = f64::from(self.start_yaw_deg).to_radians();
        let forward = DVec3::new(-yaw.sin(), 0.0, -yaw.cos());
        let along = (from - Vec3::from_array(self.start_position_cm).as_dvec3()).dot(forward);
        let step = delta.dot(forward);
        let mut t: f64 = 1.0;
        if step < 0.0 {
            t = t.min((along.max(0.0) / -step).clamp(0.0, 1.0));
        }
        for obstacle in &self.obstacles {
            if let Some(hit) = obstacle
                .interval(from, delta, f64::from(self.clearance_cm))
                .hit()
            {
                t = t.min((hit - 0.001 / length).max(0.0));
            }
        }
        from + delta * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn movement(obstacle: Option<Obstacle>) -> CameraMovement {
        CameraMovement {
            start_position_cm: [0.0, 10.0, 0.0],
            start_yaw_deg: 0.0,
            clearance_cm: 1.0,
            end_blackout_ms: 0,
            obstacles: obstacle.into_iter().collect(),
        }
    }
    #[test]
    fn start_plane_blocks_reverse_and_immediately_allows_forward() {
        let m = movement(None);
        let start = DVec3::new(0.0, 10.0, 0.0);
        assert_eq!(m.constrain(start, start + DVec3::Z * 100.0), start);
        let ahead = start - DVec3::Z * 5.0;
        assert_eq!(m.constrain(start, ahead), ahead);
        assert_eq!(m.constrain(ahead, start + DVec3::Z * 100.0), start);
    }
    #[test]
    fn swept_motion_cannot_tunnel_and_reverse_has_no_debt() {
        for obstacle in [
            Obstacle::Box {
                position_cm: [0.0, 10.0, -50.0],
                yaw_deg: 30.0,
                size_cm: [40.0, 20.0, 0.1],
            },
            Obstacle::Sphere {
                position_cm: [0.0, 10.0, -50.0],
                diameter_cm: 10.0,
            },
            Obstacle::Cylinder {
                position_cm: [0.0, 10.0, -50.0],
                yaw_deg: 0.0,
                size_cm: [10.0, 20.0, 10.0],
            },
            Obstacle::Diamond {
                position_cm: [0.0, 10.0, -50.0],
                yaw_deg: 45.0,
                size_cm: [20.0, 20.0, 10.0],
            },
        ] {
            let m = movement(Some(obstacle));
            m.validate().unwrap();
            let a = DVec3::new(0.0, 10.0, 0.0);
            let b = m.constrain(a, DVec3::new(0.0, 10.0, -1000.0));
            assert!(b.z < -30.0 && b.z > -50.0, "{b}");
            let held = m.constrain(b, b - DVec3::Z * 1000.0);
            assert!((held - b).length() < 1e-6);
            assert!(
                (m.constrain(held, held + DVec3::Z * 5.0) - (held + DVec3::Z * 5.0)).length()
                    < 1e-6
            );
        }
    }
    #[test]
    fn height_is_respected_and_start_must_be_free() {
        let mut m = movement(Some(Obstacle::Box {
            position_cm: [0.0, 50.0, -50.0],
            yaw_deg: 0.0,
            size_cm: [40.0, 4.0, 4.0],
        }));
        let a = DVec3::new(0.0, 10.0, 0.0);
        let b = DVec3::new(0.0, 10.0, -100.0);
        assert_eq!(m.constrain(a, b), b);
        m.obstacles.push(Obstacle::Sphere {
            position_cm: [0.0, 10.0, 0.0],
            diameter_cm: 10.0,
        });
        assert!(m.validate().is_err());
    }
}
