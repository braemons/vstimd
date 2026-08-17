use super::grating::Grating;
use super::mesh3d::Mesh3d;
use super::shape::Shape;
use super::stimulus_common::StimulusCommon;
use super::stimulus_flags::StimulusFlags;
use super::text::Text;
use crate::scene::deferred::Deferred;
pub use crate::scene::stimulus::shape_appearance::ShapeAppearance;
use crate::scene::stimulus::transform2d::Transform2D;

// ── Stimulus ──────────────────────────────────────────────────────────────────

/// One stimulus: the state every stimulus has, plus the state its body has.
///
/// Shared state lives *above* the body, so [`flags`](Self::flags),
/// [`opacity`](Self::opacity) and friends are field reads rather than one match
/// arm per variant. Adding a body variant does not touch them.
///
/// Serialized as `{"common": {...}, "body": {"type": "Shape", ...}}`. The config
/// format is the runtime shape — there is no separate DTO. Each type that owns
/// runtime state ([`StimulusFlags`], [`Grating`], [`Text`]) hides it behind its
/// own `serde` impl, so the tree serializes correctly by composition.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Stimulus {
    pub common: StimulusCommon,
    pub body: StimulusBody,
}

/// What a stimulus draws — and, equivalently, which render path draws it.
///
/// Named `Body`, not `Kind`: this enum *is* the stimulus content (each arm carries
/// a whole [`Shape`], [`Grating`], [`Text`] or [`Mesh3d`]), and `Kind` reads as a
/// near-synonym of the wire's `StimulusType` while meaning something strictly
/// coarser — which is exactly the confusion this name avoids.
///
/// **This taxonomy is the renderer's, not the user's.** It is coarser than the
/// wire API on purpose: `Rect`, `Ellipse` and `Circle` are all
/// [`Shape`](StimulusBody::Shape); `Cube3D`, `Sphere3D` and `Plane3D` are all
/// [`Mesh3d`](StimulusBody::Mesh3d). Each arm is one pipeline, one cache, one
/// push-constant layout and one dirty/upload lifecycle.
///
/// The finer user-facing names live in the geometry enums and in `ipc/`. An
/// internal body name must never reach a protocol response or an error message —
/// see [`ShapeGeometry::type_name`](super::ShapeGeometry::type_name).
///
/// Adding an arm here means adding a render path, which is a much bigger job
/// than adding a geometry variant. That the two look alike in a flat enum is
/// exactly what this split fixes.
///
/// The config tag is the *internal* name (`"Shape"`, not `"Rect"`) with the
/// user-facing name one level down in the geometry. Error messages and
/// `StimulusType` still speak the user's taxonomy — see
/// [`Stimulus::type_name`].
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum StimulusBody {
    // ── 2-D — placement in pixels, screen-centre origin, Y-up ──
    /// Rect / ellipse / circle → `solid_pipeline`.
    Shape(Shape),
    /// → `grating_pipeline`.
    Grating(Grating),
    /// → `text_pipeline`.
    Text(Text),

    // ── 3-D — placement in cm, world space, Y-up ──
    /// Cube / sphere / plane → `mesh3d_pipeline`. Placeholder; see
    /// [`mesh3d`](super::mesh3d).
    Mesh3d(Mesh3d),
}

impl StimulusBody {
    /// True when a change of opacity invalidates the cached mesh, because this
    /// kind bakes opacity into its vertex colours instead of pushing it as a
    /// constant.
    ///
    /// This is a render-path fact, so it lives on the body: only the shape
    /// pipeline bakes opacity in (`render::tess`); grating and text carry it in
    /// push constants, rebuilt from live state every frame. It answers a
    /// different question from "does this kind have an appearance" — the two
    /// coincide today and diverge as soon as a kind has an appearance but
    /// per-draw constants, which `Mesh3d` (a material, but push constants)
    /// already is.
    pub fn opacity_is_baked_into_mesh(&self) -> bool {
        matches!(self, StimulusBody::Shape(_))
    }
}

impl Stimulus {
    pub fn new(body: StimulusBody) -> Self {
        Self {
            common: StimulusCommon::new(),
            body,
        }
    }

    // ── Common field accessors ────────────────────────────────────────────────
    //
    // One-line delegations to `common`, kept as methods so the ~70 call sites
    // read the same whichever kind they hold. They need no change when a kind
    // is added.

    pub fn flags(&self) -> &StimulusFlags {
        &self.common.flags
    }

    pub fn flags_mut(&mut self) -> &mut StimulusFlags {
        &mut self.common.flags
    }

    /// Whole-stimulus opacity, a multiplier on every colour's own alpha.
    pub fn opacity(&self) -> &Deferred<f32> {
        &self.common.opacity
    }

    /// Set the shared opacity, clamped to `[0, 1]`.
    ///
    /// Only shapes are marked dirty. `dirty` means "the cached mesh is stale",
    /// and only the shape pipeline bakes opacity into vertex colours
    /// (`render::tess`); grating and text carry it in push constants, which are
    /// rebuilt from live state every frame anyway. Marking text dirty here
    /// would re-shape and re-rasterize every glyph on each opacity change — a
    /// fade would pay for a full text re-layout per frame, for nothing.
    pub fn set_opacity(&mut self, deferred: bool, opacity: f32) {
        self.common
            .opacity
            .set(deferred, opacity.clamp(0.0, 1.0));
        if !deferred && self.body.opacity_is_baked_into_mesh() {
            self.flags_mut().mark_dirty();
        }
    }

    // ── Placement ─────────────────────────────────────────────────────────────

    /// The 2-D transform, or `None` for a 3-D stimulus.
    pub fn transform2d(&self) -> Option<&Deferred<Transform2D>> {
        match &self.body {
            StimulusBody::Shape(s) => Some(&s.transform),
            StimulusBody::Grating(g) => Some(&g.transform),
            StimulusBody::Text(t) => Some(&t.transform),
            StimulusBody::Mesh3d(_) => None,
        }
    }

    pub fn transform2d_mut(&mut self) -> Option<&mut Deferred<Transform2D>> {
        match &mut self.body {
            StimulusBody::Shape(s) => Some(&mut s.transform),
            StimulusBody::Grating(g) => Some(&mut g.config.transform),
            StimulusBody::Text(t) => Some(&mut t.config.transform),
            StimulusBody::Mesh3d(_) => None,
        }
    }

    // ── Kind accessors ────────────────────────────────────────────────────────

    /// The shape, or `None` for another kind. The narrowing hop for code that
    /// holds a `&Stimulus` and needs one specific kind.
    pub fn shape(&self) -> Option<&Shape> {
        match &self.body {
            StimulusBody::Shape(s) => Some(s),
            _ => None,
        }
    }

    pub fn grating(&self) -> Option<&Grating> {
        match &self.body {
            StimulusBody::Grating(g) => Some(g),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<&Text> {
        match &self.body {
            StimulusBody::Text(t) => Some(t),
            _ => None,
        }
    }

    /// Shape appearance (fill/outline/draw-mode) — `None` for other kinds.
    pub fn shape_appearance(&self) -> Option<&Deferred<ShapeAppearance>> {
        match &self.body {
            StimulusBody::Shape(s) => Some(&s.appearance),
            _ => None,
        }
    }

    pub fn shape_appearance_mut(&mut self) -> Option<&mut Deferred<ShapeAppearance>> {
        match &mut self.body {
            StimulusBody::Shape(s) => Some(&mut s.appearance),
            _ => None,
        }
    }

    // ── Config load ───────────────────────────────────────────────────────────

    /// Reset self-advanced runtime state to what a fresh config load produces.
    ///
    /// Some kinds carry state the render thread advances on its own, each frame,
    /// from their config parameters: a grating's `phase_accum` (drift), and
    /// later a random-dot pattern's seed or a movie's frame counter. Loading a
    /// config — or re-arming one mid-session — must zero that state, or the
    /// stimulus resumes from wherever the *previous* session left it.
    ///
    /// This is deliberately not a general "restore original placement": state
    /// driven *externally* (an animation's displacement of a 3-D stimulus or
    /// the camera) is not captured anywhere yet, and is a separate mechanism.
    ///
    /// A match on every kind — like `make_copy`/`flip` — so a new kind cannot
    /// compile without deciding whether it has dynamic state to reset.
    pub fn reset_dynamic_state(&mut self) {
        match &mut self.body {
            StimulusBody::Shape(_) | StimulusBody::Text(_) => {}
            StimulusBody::Grating(g) => g.reset_phase_accum(),
            // No dynamic state yet — Phase B meshes are static placeholders.
            StimulusBody::Mesh3d(_) => {}
        }
    }

    // ── Deferred mode ─────────────────────────────────────────────────────────

    /// Snapshot all live state into copy fields. Call at the start of deferred mode.
    pub fn make_copy(&mut self) {
        self.common.make_copy();
        match &mut self.body {
            StimulusBody::Shape(s) => s.make_copy(),
            StimulusBody::Grating(g) => g.make_copy(),
            StimulusBody::Text(t) => t.make_copy(),
            StimulusBody::Mesh3d(m) => m.make_copy(),
        }
    }

    /// Promote all copy fields to live. Call at the frame boundary when `pending_flip` is set.
    pub fn flip(&mut self) {
        self.common.flip();
        match &mut self.body {
            StimulusBody::Shape(s) => s.flip(),
            StimulusBody::Grating(g) => g.flip(),
            StimulusBody::Text(t) => t.flip(),
            StimulusBody::Mesh3d(m) => m.flip(),
        }
    }

    // ── Spatial commands ──────────────────────────────────────────────────────

    /// Move a 2-D stimulus. Fails with [`WrongDimension`] for a 3-D one.
    ///
    /// §9.3 of the roadmap proposes mapping `(x, y)` onto a 3-D stimulus'
    /// `position.xz`. It should not: both coordinate systems are Y-up (§3.1,
    /// §3.2), so routing `y` into `z` would make "move up" mean "move forward",
    /// and the units differ regardless — pixels into centimetres has no correct
    /// interpretation. 3-D placement gets its own commands (`SetTransform3D`)
    /// and its own animation kinds.
    pub fn move_to_2d(&mut self, deferred: bool, x: f32, y: f32) -> Result<(), WrongDimension> {
        let Some(t) = self.transform2d_mut() else {
            return Err(WrongDimension);
        };
        let angle = if deferred { t.copy.angle } else { t.live.angle };
        t.set(deferred, Transform2D { pos: [x, y], angle });
        if !deferred {
            self.flags_mut().mark_dirty();
        }
        Ok(())
    }

    /// Rotate a 2-D stimulus. Fails with [`WrongDimension`] for a 3-D one — see
    /// [`move_to_2d`](Self::move_to_2d).
    pub fn set_angle_2d(&mut self, deferred: bool, degrees: f32) -> Result<(), WrongDimension> {
        let Some(t) = self.transform2d_mut() else {
            return Err(WrongDimension);
        };
        let pos = if deferred { t.copy.pos } else { t.live.pos };
        t.set(deferred, Transform2D { pos, angle: degrees });
        if !deferred {
            self.flags_mut().mark_dirty();
        }
        Ok(())
    }

    /// 2-D position, or `None` for a 3-D stimulus.
    pub fn get_pos_2d(&self) -> Option<[f32; 2]> {
        self.transform2d().map(|t| t.live.pos)
    }

    // ── Visibility ────────────────────────────────────────────────────────────

    pub fn is_visible(&self) -> bool {
        self.flags().is_visible()
    }

    // ── Display name ──────────────────────────────────────────────────────────

    /// The **user-facing** type name — what the config `"type"` tag holds and
    /// what a `WRONG_STIMULUS_TYPE` error quotes back. Sourced from the geometry
    /// so a client never sees an internal kind name.
    pub fn type_name(&self) -> &'static str {
        match &self.body {
            StimulusBody::Shape(s) => s.geometry.live.type_name(),
            StimulusBody::Grating(_) => Grating::TYPE_NAME,
            StimulusBody::Text(_) => Text::TYPE_NAME,
            StimulusBody::Mesh3d(m) => m.geometry.live.type_name(),
        }
    }
}

// ── Errors ────────────────────────────────────────────────────────────────────

/// The operation addresses a 2-D placement, but the stimulus lives in 3-D
/// world space (or vice versa). Unit type: the caller holds the stimulus and
/// can name it in its own error — this only carries *why* the write was
/// refused, not *whom* it was refused to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrongDimension;

impl std::fmt::Display for WrongDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("stimulus placement is in a different dimension")
    }
}

impl std::error::Error for WrongDimension {}

// ── Kind → Stimulus ───────────────────────────────────────────────────────────
//
// So a create path reads `Stimulus::from(Shape::new(..))` rather than naming
// `StimulusCommon` at every call site.

impl From<Shape> for Stimulus {
    fn from(s: Shape) -> Self {
        Self::new(StimulusBody::Shape(s))
    }
}

impl From<Grating> for Stimulus {
    fn from(g: Grating) -> Self {
        Self::new(StimulusBody::Grating(g))
    }
}

impl From<Text> for Stimulus {
    fn from(t: Text) -> Self {
        Self::new(StimulusBody::Text(t))
    }
}

impl From<Mesh3d> for Stimulus {
    fn from(m: Mesh3d) -> Self {
        Self::new(StimulusBody::Mesh3d(m))
    }
}
