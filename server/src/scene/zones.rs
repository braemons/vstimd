//! Camera zones: regions of the 3-D world whose entry and exit act like a
//! trigger-line input.
//!
//! A zone names a VTL **input** line. Each frame, before animations run, the
//! camera's position is tested against every zone and the result is merged into
//! that line's input edges: rising on entry, falling on exit, level HIGH while
//! inside. Everything that already reacts to a trigger line then reacts to a
//! zone unchanged — `start_trigger`, `cancel_trigger`, `EnableOnTriggerEdge`,
//! `CoupleVisibilityToTriggerLine` — and the edges reach the event stream like a
//! hardware input's. An animation reacting to a zone can in turn pulse an output
//! line to the DAQ, e.g. to open a reward valve.
//!
//! Pick lines the DAQ does not drive: a zone's state is ORed with whatever the
//! hardware reports on the same line.
//!
//! The test uses the camera as rendered on the previous frame, so a reaction
//! starts one frame after the camera crossed — the same latency as a hardware
//! input polled once per frame.
//!
//! Zones are experiment state, saved in the scene-config. With a wrapping
//! corridor, define them within one period `[0, L)`: the camera's `z` never
//! leaves it, so a zone there fires once per lap.

use vtl::{MAX_BANKS, VtlKind};

use crate::vtl_state::{VtlBit, VtlEdges};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CameraZone {
    pub name: String,
    /// The input line this zone drives.
    pub line: VtlBit,
    /// World-space extent along each axis, cm, `[min, max]`. `None` is
    /// unbounded along that axis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_cm: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_cm: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_cm: Option<[f32; 2]>,
    /// Whether the camera was inside on the last evaluation. Runtime only.
    #[serde(skip)]
    pub inside: bool,
}

impl CameraZone {
    pub fn contains(&self, p: glam::Vec3) -> bool {
        let within = |r: Option<[f32; 2]>, v: f32| r.is_none_or(|[lo, hi]| lo <= v && v <= hi);
        within(self.x_cm, p.x) && within(self.y_cm, p.y) && within(self.z_cm, p.z)
    }

    /// Refuse a zone that could never behave: a non-input line, an empty or
    /// inverted range.
    pub fn validate(&self) -> Result<(), String> {
        if self.line.kind != VtlKind::Input {
            return Err(format!("zone '{}': its line must be an input line", self.name));
        }
        if self.line.bank >= MAX_BANKS || self.line.bit >= 64 {
            return Err(format!("zone '{}': line out of range", self.name));
        }
        for (axis, r) in [("x", self.x_cm), ("y", self.y_cm), ("z", self.z_cm)] {
            if let Some([lo, hi]) = r
                && !(lo.is_finite() && hi.is_finite() && lo <= hi)
            {
                return Err(format!("zone '{}': {axis}_cm needs finite min <= max", self.name));
            }
        }
        Ok(())
    }
}

/// Validate a whole zone set: each zone, plus unique names and lines.
pub fn validate_zones(zones: &[CameraZone]) -> Result<(), String> {
    for (i, z) in zones.iter().enumerate() {
        z.validate()?;
        if zones[..i].iter().any(|o| o.name == z.name) {
            return Err(format!("zone name '{}' is used twice", z.name));
        }
        if zones[..i].iter().any(|o| o.line == z.line) {
            return Err(format!(
                "zones '{}' and another share input line ({}, {})",
                z.name, z.line.bank, z.line.bit
            ));
        }
    }
    Ok(())
}

/// Test `camera` against every zone and merge the transitions into `edges`.
/// No allocation. A no-op for an empty zone list.
pub fn evaluate(zones: &mut [CameraZone], camera: glam::Vec3, edges: &mut VtlEdges) {
    for z in zones {
        let inside = z.contains(camera);
        let (bank, mask) = (z.line.bank, 1u64 << z.line.bit);
        if inside {
            edges.current[bank] |= mask;
        }
        if inside && !z.inside {
            edges.rising[bank] |= mask;
        } else if !inside && z.inside {
            edges.falling[bank] |= mask;
        }
        z.inside = inside;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(z: [f32; 2], bit: u8) -> CameraZone {
        CameraZone {
            name: format!("z{bit}"),
            line: VtlBit { bank: 3, bit, kind: VtlKind::Input },
            x_cm: None,
            y_cm: None,
            z_cm: Some(z),
            inside: false,
        }
    }

    #[test]
    fn entering_and_leaving_make_edges_and_a_level() {
        let mut zones = vec![zone([20.0, 30.0], 5)];
        let at = |z: f32| glam::Vec3::new(0.0, 0.0, z);
        let mut e = VtlEdges::default();
        evaluate(&mut zones, at(40.0), &mut e);
        assert_eq!((e.rising[3], e.current[3]), (0, 0));

        let mut e = VtlEdges::default();
        evaluate(&mut zones, at(25.0), &mut e);
        assert_eq!((e.rising[3], e.current[3], e.falling[3]), (1 << 5, 1 << 5, 0));

        let mut e = VtlEdges::default();
        evaluate(&mut zones, at(21.0), &mut e);
        assert_eq!((e.rising[3], e.current[3]), (0, 1 << 5), "no second edge while inside");

        let mut e = VtlEdges::default();
        evaluate(&mut zones, at(10.0), &mut e);
        assert_eq!((e.falling[3], e.current[3]), (1 << 5, 0));
    }

    #[test]
    fn hardware_edges_on_other_lines_survive() {
        let mut zones = vec![zone([0.0, 10.0], 1)];
        let mut e = VtlEdges::default();
        e.rising[0] = 0b100;
        e.current[3] = 1 << 7;
        evaluate(&mut zones, glam::Vec3::new(0.0, 0.0, 5.0), &mut e);
        assert_eq!(e.rising[0], 0b100);
        assert_eq!(e.current[3], (1 << 7) | (1 << 1));
    }

    #[test]
    fn bad_zone_sets_are_refused() {
        let mut out = zone([0.0, 1.0], 1);
        out.line.kind = VtlKind::Output;
        assert!(validate_zones(&[out]).is_err());
        assert!(validate_zones(&[zone([5.0, 1.0], 1)]).is_err());
        assert!(validate_zones(&[zone([0.0, 1.0], 1), zone([2.0, 3.0], 1)]).is_err(), "shared line");
        assert!(validate_zones(&[zone([0.0, 1.0], 1), zone([2.0, 3.0], 2)]).is_ok());
    }
}
