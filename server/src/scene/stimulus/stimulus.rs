use super::grating::GratingStimulus;
use super::primitive_shapes::{CircleStimulus, EllipseStimulus, RectStimulus};
use super::stimulus_common::StimulusCommon;
use super::stimulus_flags::StimulusFlags;
use super::text::TextStimulus;
use crate::scene::deferred::Deferred;
pub use crate::scene::stimulus::shape_appearance::ShapeAppearance;
use crate::scene::stimulus::transform2d::Transform2D;

// Apply `$body` to the inner struct of whichever shape variant `$self` is, or
// fall through to the non-shape arms. Keeps the shared shape accessors DRY.
macro_rules! shape_arm {
    ($self:expr, $s:ident => $body:expr) => {
        match $self {
            Stimulus::Rect($s) => $body,
            Stimulus::Ellipse($s) => $body,
            Stimulus::Circle($s) => $body,
            _ => unreachable!("shape_arm! on non-shape stimulus"),
        }
    };
}

// ── Stimulus enum ─────────────────────────────────────────────────────────────

/// A 2-D stimulus. Serialized internally-tagged (`{"type": "Rect", ...}`) so the
/// config format matches animations and is friendly to schema-driven tooling.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum Stimulus {
    Rect(RectStimulus),
    Ellipse(EllipseStimulus),
    Circle(CircleStimulus),
    Grating(GratingStimulus),
    Text(TextStimulus),
}

impl Stimulus {
    // ── Common field accessors ────────────────────────────────────────────────

    /// The state every variant shares — flags, transform, opacity.
    pub fn common(&self) -> &StimulusCommon {
        match self {
            Stimulus::Rect(s) => &s.common,
            Stimulus::Ellipse(s) => &s.common,
            Stimulus::Circle(s) => &s.common,
            Stimulus::Grating(s) => &s.common,
            Stimulus::Text(s) => &s.common,
        }
    }

    pub fn common_mut(&mut self) -> &mut StimulusCommon {
        match self {
            Stimulus::Rect(s) => &mut s.common,
            Stimulus::Ellipse(s) => &mut s.common,
            Stimulus::Circle(s) => &mut s.common,
            Stimulus::Grating(s) => &mut s.common,
            Stimulus::Text(s) => &mut s.common,
        }
    }

    pub fn flags(&self) -> &StimulusFlags {
        &self.common().flags
    }

    pub fn flags_mut(&mut self) -> &mut StimulusFlags {
        &mut self.common_mut().flags
    }

    pub fn transform(&self) -> &Deferred<Transform2D> {
        &self.common().transform
    }

    pub fn transform_mut(&mut self) -> &mut Deferred<Transform2D> {
        &mut self.common_mut().transform
    }

    /// Whole-stimulus opacity, a multiplier on every colour's own alpha.
    pub fn opacity(&self) -> &Deferred<f32> {
        &self.common().opacity
    }

    /// Set the shared opacity, clamped to `[0, 1]`. Marks the stimulus dirty
    /// outside deferred mode so the change reaches the next frame.
    pub fn set_opacity(&mut self, deferred: bool, opacity: f32) {
        self.common_mut().opacity.set(deferred, opacity.clamp(0.0, 1.0));
        if !deferred {
            self.flags_mut().mark_dirty();
        }
    }

    /// Shape appearance (fill/outline/draw-mode) — `None` for grating/text.
    pub fn shape_appearance(&self) -> Option<&Deferred<ShapeAppearance>> {
        match self {
            Stimulus::Rect(_) | Stimulus::Ellipse(_) | Stimulus::Circle(_) => {
                Some(shape_arm!(self, s => &s.appearance))
            }
            _ => None,
        }
    }

    pub fn shape_appearance_mut(&mut self) -> Option<&mut Deferred<ShapeAppearance>> {
        match self {
            Stimulus::Rect(_) | Stimulus::Ellipse(_) | Stimulus::Circle(_) => {
                Some(shape_arm!(self, s => &mut s.appearance))
            }
            _ => None,
        }
    }

    /// True for rect/ellipse/circle.
    pub fn is_shape(&self) -> bool {
        matches!(
            self,
            Stimulus::Rect(_) | Stimulus::Ellipse(_) | Stimulus::Circle(_)
        )
    }

    // ── Config load ───────────────────────────────────────────────────────────

    /// Reset render-thread runtime state after loading from config.
    pub fn reset_phase_accum(&mut self) {
        if let Stimulus::Grating(s) = self {
            s.reset_phase_accum();
        }
    }

    // ── Deferred mode ─────────────────────────────────────────────────────────

    /// Snapshot all live state into copy fields. Call at the start of deferred mode.
    pub fn make_copy(&mut self) {
        match self {
            Stimulus::Rect(s) => s.make_copy(),
            Stimulus::Ellipse(s) => s.make_copy(),
            Stimulus::Circle(s) => s.make_copy(),
            Stimulus::Grating(s) => s.make_copy(),
            Stimulus::Text(s) => s.make_copy(),
        }
    }

    /// Promote all copy fields to live. Call at the frame boundary when `pending_flip` is set.
    pub fn flip(&mut self) {
        match self {
            Stimulus::Rect(s) => s.flip(),
            Stimulus::Ellipse(s) => s.flip(),
            Stimulus::Circle(s) => s.flip(),
            Stimulus::Grating(s) => s.flip(),
            Stimulus::Text(s) => s.flip(),
        }
    }

    // ── Spatial commands ──────────────────────────────────────────────────────

    pub fn move_to(&mut self, deferred: bool, x: f32, y: f32) {
        {
            let t = self.transform_mut();
            let angle = if deferred { t.copy.angle } else { t.live.angle };
            t.set(deferred, Transform2D { pos: [x, y], angle });
        }
        if !deferred {
            self.flags_mut().mark_dirty();
        }
    }

    pub fn set_angle(&mut self, deferred: bool, degrees: f32) {
        {
            let t = self.transform_mut();
            let pos = if deferred { t.copy.pos } else { t.live.pos };
            t.set(
                deferred,
                Transform2D {
                    pos,
                    angle: degrees,
                },
            );
        }
        if !deferred {
            self.flags_mut().mark_dirty();
        }
    }

    pub fn get_pos(&self) -> [f32; 2] {
        self.transform().live.pos
    }

    // ── Visibility ────────────────────────────────────────────────────────────

    pub fn is_visible(&self) -> bool {
        self.flags().is_visible()
    }

    // ── Display name ──────────────────────────────────────────────────────────

    pub fn type_name(&self) -> &'static str {
        match self {
            Stimulus::Rect(_) => RectStimulus::TYPE_NAME,
            Stimulus::Ellipse(_) => EllipseStimulus::TYPE_NAME,
            Stimulus::Circle(_) => CircleStimulus::TYPE_NAME,
            Stimulus::Grating(_) => GratingStimulus::TYPE_NAME,
            Stimulus::Text(_) => TextStimulus::TYPE_NAME,
        }
    }
}
