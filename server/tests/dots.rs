//! Integration tests for the random dot kinematogram, over the protobuf surface.
//!
//! Like the other tests here these call `handle_request` directly on a
//! `SceneState`, so they need neither ZMQ nor a GPU.
//!
//! The centrepiece is `figure_ground`, which builds the stimulus from
//! `stimulusStageFigureRDBackgroundComponentsCLaser_BW.m` — the Psychtoolbox
//! figure-ground RDK this implementation exists to reproduce — and checks the
//! properties that make it the stimulus it is. See `dev/design/RDK_PLAN.md`.

use vstimd::Color;
use vstimd::proto;
use vstimd::proto::request;
use vstimd::scene::SceneState;
use vstimd::scene::stimulus::StimulusBody;

// ── helpers ───────────────────────────────────────────────────────────────────

fn sys() -> request::Target {
    request::Target::System(proto::SystemTarget {})
}

fn stim(handle: u32) -> request::Target {
    request::Target::Stimulus(handle)
}

fn req(target: request::Target, body: request::Body) -> proto::Request {
    proto::Request { target: Some(target), body: Some(body) }
}

fn create_dots(scene: &mut SceneState, params: proto::DotsParams, pos_px: [f32; 2]) -> u32 {
    let resp = scene.handle_request(
        req(
            sys(),
            request::Body::CreateDots(proto::CreateDotsRequest {
                identity: None,
                placement: Some(proto::Transform2D {
                    pos_px: Some(proto::Vec2 { x: pos_px[0], y: pos_px[1] }),
                    rotation_deg: 0.0,
                }),
                params: Some(params),
            }),
        ),
        None,
    );
    assert_eq!(resp.code, proto::ErrorCode::Ok as i32, "CreateDots: {}", resp.error);
    assert!(resp.handle > 0);
    resp.handle as u32
}

fn query(scene: &mut SceneState, handle: u32) -> proto::QueryStimulusResponse {
    let resp = scene.handle_request(
        req(stim(handle), request::Body::QueryStimulus(proto::QueryStimulusRequest {})),
        None,
    );
    assert_eq!(resp.code, proto::ErrorCode::Ok as i32, "QueryStimulus: {}", resp.error);
    match resp.body {
        Some(proto::response::Body::StimulusInfo(i)) => i,
        other => panic!("expected StimulusInfo, got {other:?}"),
    }
}

fn dots_params_of(info: &proto::QueryStimulusResponse) -> &proto::DotsParams {
    match info.params.as_ref().and_then(|p| p.shape.as_ref()) {
        Some(proto::stimulus_params::Shape::Dots(d)) => d,
        other => panic!("expected Dots params, got {other:?}"),
    }
}

/// Advance every dot field in the scene by `frames`, as the render thread would.
fn advance(scene: &mut SceneState, frames: u32, nominal_hz: f32) {
    for _ in 0..frames {
        for entry in scene.config.stimuli.values_mut() {
            if let StimulusBody::Dots(d) = &mut entry.stimulus.body {
                d.advance(nominal_hz);
            }
        }
    }
}

fn positions(scene: &SceneState, handle: u32) -> Vec<[f32; 2]> {
    scene.config.stimuli[&handle]
        .stimulus
        .dots()
        .expect("not a dot field")
        .positions()
        .to_vec()
}

// ── creation and query ────────────────────────────────────────────────────────

#[test]
fn creates_and_reports_itself_as_dots() {
    let mut scene = SceneState::new();
    let h = create_dots(&mut scene, proto::DotsParams::default(), [0.0, 0.0]);
    let info = query(&mut scene, h);
    assert_eq!(info.stimulus_type, proto::StimulusType::Dots as i32);
    // Not the old C++ point cloud, which owns wire value 9.
    assert_ne!(info.stimulus_type, proto::StimulusType::Particle as i32);
}

/// The zero-means-default convention: an empty params message gives a usable
/// field, not an invisible one.
#[test]
fn empty_params_give_working_defaults() {
    let mut scene = SceneState::new();
    let h = create_dots(&mut scene, proto::DotsParams::default(), [0.0, 0.0]);
    let info = query(&mut scene, h);
    let p = dots_params_of(&info);
    assert!(p.dot_count > 0);
    assert!(p.dot_size_px > 0.0);
    assert!(p.field_width_px > 0.0 && p.field_height_px > 0.0);
    assert_eq!(p.coherence, Some(1.0), "an unset coherence is 1, not 0");
    assert_eq!(p.speed_px_per_s, Some(100.0), "an unset speed is not a static field");
    // An unset aperture is the field, not a crop of it.
    let a = p.aperture.as_ref().expect("aperture reported");
    assert_eq!(a.width_px, p.field_width_px);
    assert_eq!(a.height_px, p.field_height_px);
}

/// Every parameter survives the round trip out to the wire and back.
#[test]
fn params_round_trip_through_query() {
    let mut scene = SceneState::new();
    let sent = proto::DotsParams {
        field_width_px: 1920.0,
        field_height_px: 1080.0,
        dot_count: 321,
        aperture: Some(proto::Aperture {
            shape: proto::ApertureShape::Circle as i32,
            width_px: 450.0,
            height_px: 0.0,
            offset_x_px: 120.0,
            offset_y_px: -80.0,
            invert: true,
            clip: proto::ApertureClip::Pixel as i32,
        }),
        dot_size_px: 9.0,
        dot_color: Some(Color::WHITE.into()),
        dot_color_alt: Some(Color::BLACK.into()),
        dot_shape: proto::DotShape::Square as i32,
        direction_deg: 135.0,
        speed_px_per_s: Some(250.0),
        coherence: Some(0.6),
        signal_rule: proto::SignalRule::Different as i32,
        noise_rule: proto::NoiseRule::Walk as i32,
        reinsertion: proto::Reinsertion::Respawn as i32,
        dot_lifetime_frames: 12,
        seed: 4242,
    };
    let h = create_dots(&mut scene, sent.clone(), [0.0, 0.0]);
    let info = query(&mut scene, h);
    let got = dots_params_of(&info);
    assert_eq!(got.dot_count, sent.dot_count);
    assert_eq!(got.dot_size_px, sent.dot_size_px);
    assert_eq!(got.direction_deg, sent.direction_deg);
    assert_eq!(got.speed_px_per_s, sent.speed_px_per_s);
    assert_eq!(got.coherence, sent.coherence);
    assert_eq!(got.dot_shape, sent.dot_shape);
    assert_eq!(got.signal_rule, sent.signal_rule);
    assert_eq!(got.noise_rule, sent.noise_rule);
    assert_eq!(got.reinsertion, sent.reinsertion);
    assert_eq!(got.dot_lifetime_frames, sent.dot_lifetime_frames);
    assert_eq!(got.seed, sent.seed);
    assert_eq!(got.dot_color_alt, sent.dot_color_alt);
    let a = got.aperture.as_ref().unwrap();
    assert_eq!(a.shape, proto::ApertureShape::Circle as i32);
    assert_eq!(a.width_px, 450.0, "a circle is sized by its diameter");
    assert!(a.invert);
    assert_eq!(a.clip, proto::ApertureClip::Pixel as i32);
    assert_eq!((a.offset_x_px, a.offset_y_px), (120.0, -80.0));
}

/// A dot mutation aimed at another stimulus type is refused by name, not silently
/// ignored.
#[test]
fn dot_mutations_refuse_other_stimulus_types() {
    let mut scene = SceneState::new();
    let rect = scene.handle_request(
        req(
            sys(),
            request::Body::CreateRect(proto::CreateRectRequest::default()),
        ),
        None,
    );
    let rect = rect.handle as u32;
    let resp = scene.handle_request(
        req(
            stim(rect),
            request::Body::SetDotsDirection(proto::SetDotsDirectionRequest {
                direction_deg: 90.0,
            }),
        ),
        None,
    );
    assert_eq!(resp.code, proto::ErrorCode::WrongStimulusType as i32);
    assert!(resp.error.contains("Dots"), "error should name the wanted type: {}", resp.error);
}

// ── reproducibility ───────────────────────────────────────────────────────────

/// Replaying a config has to reproduce the stimulus, not merely one like it: the
/// sample is a function of the seed and the frame index alone.
#[test]
fn the_same_seed_replays_the_same_frames() {
    let run = || {
        let mut scene = SceneState::new();
        let h = create_dots(
            &mut scene,
            proto::DotsParams {
                seed: 2024,
                dot_count: 300,
                coherence: Some(0.5),
                speed_px_per_s: Some(200.0),
                ..Default::default()
            },
            [0.0, 0.0],
        );
        advance(&mut scene, 120, 60.0);
        positions(&scene, h)
    };
    assert_eq!(run(), run());
}

/// `SetDotsSeed` redraws the sample and restarts it, even mid-trial.
#[test]
fn setting_the_seed_over_the_wire_restarts_the_field() {
    let mut scene = SceneState::new();
    let h = create_dots(
        &mut scene,
        proto::DotsParams { seed: 1, dot_count: 50, speed_px_per_s: Some(200.0), ..Default::default() },
        [0.0, 0.0],
    );
    let at_start = positions(&scene, h);
    advance(&mut scene, 30, 60.0);
    assert_ne!(positions(&scene, h), at_start);

    let resp = scene.handle_request(
        req(stim(h), request::Body::SetDotsSeed(proto::SetDotsSeedRequest { seed: 1 })),
        None,
    );
    assert_eq!(resp.code, proto::ErrorCode::Ok as i32);
    assert_eq!(positions(&scene, h), at_start, "reseeding must restart the sample");
}

// ── the reproduction target ───────────────────────────────────────────────────

/// The figure-ground RDK of `stimulusStageFigureRDBackgroundComponentsCLaser_BW.m`.
///
/// Two fields over the whole screen, identical in every respect but direction: the
/// background visible only *outside* a 45 deg circle on the receptive field, the
/// figure only inside it. Nothing but motion distinguishes them in any single
/// frame, which is the whole point of the stimulus.
///
/// The MATLAB numbers, converted at the boundary (`dev/design/RDK_PLAN.md` §1.3):
///   - `dotSize = 1.5 deg` is a **radius** → 3 deg across → 60 px at 20 px/deg
///   - `R = 45/2 deg` is a **radius** → 45 deg across → 900 px diameter
///   - `dirAngle = 0` → 0°, `dirAngle = 3*pi/2` → **90°** (MATLAB's Y grows down)
///   - `vel = 50 deg/s` → 1000 px/s
///   - density `1/(5 deg)²` over the screen → `dot_count`
#[test]
fn figure_ground() {
    const PX_PER_DEG: f32 = 20.0;
    const SCREEN: [f32; 2] = [1920.0, 1080.0];
    const RF_CENTER: [f32; 2] = [200.0, -150.0];

    let deg = |d: f32| d * PX_PER_DEG;
    // 1 dot per (5 deg)², over the whole field.
    let density_per_deg2 = 1.0 / 25.0;
    let dot_count =
        ((SCREEN[0] / PX_PER_DEG) * (SCREEN[1] / PX_PER_DEG) * density_per_deg2).round() as u32;

    let common = proto::DotsParams {
        field_width_px: SCREEN[0],
        field_height_px: SCREEN[1],
        dot_count,
        dot_size_px: deg(3.0), // dotSize is a radius; the dot is 3 deg across
        dot_color: Some(Color::WHITE.into()),
        speed_px_per_s: Some(deg(50.0)),
        coherence: Some(1.0), // cohPropBackground = cohPropFigure = 1
        signal_rule: proto::SignalRule::Same as i32,
        noise_rule: proto::NoiseRule::Direction as i32,
        reinsertion: proto::Reinsertion::Wrap as i32,
        dot_lifetime_frames: 0, // the original never reborns a dot
        ..Default::default()
    };
    // R = 45/2 is a radius, so the circle is 45 deg across.
    let figure_circle = proto::Aperture {
        shape: proto::ApertureShape::Circle as i32,
        width_px: deg(45.0),
        height_px: 0.0,
        offset_x_px: RF_CENTER[0],
        offset_y_px: RF_CENTER[1],
        invert: false,
        // Dots overhang the boundary uncut, as the MATLAB's centre-pixel test does.
        clip: proto::ApertureClip::DotCenter as i32,
    };

    let mut scene = SceneState::new();
    let background = create_dots(
        &mut scene,
        proto::DotsParams {
            aperture: Some(proto::Aperture { invert: true, ..figure_circle }),
            direction_deg: 0.0, // dirAngleBackground = 0
            seed: 1,
            ..common.clone()
        },
        [0.0, 0.0],
    );
    let figure = create_dots(
        &mut scene,
        proto::DotsParams {
            aperture: Some(figure_circle),
            direction_deg: 90.0, // dirAngleFigure = 3*pi/2, which is UP
            seed: 2,
            ..common.clone()
        },
        [0.0, 0.0],
    );

    // ── the two fields tile the screen exactly once ──
    //
    // Every dot belongs to exactly one of them: the figure aperture and its
    // inverse partition the field, so no dot is drawn twice and none is dropped.
    // This is what makes the two regions equal in density — the property that
    // leaves motion as the only cue.
    let drawn = |scene: &SceneState, h| {
        let d = scene.config.stimuli[&h].stimulus.dots().unwrap();
        let mut buf = vec![Default::default(); d.live_count()];
        d.write_instances(&mut buf) as usize
    };
    let n_bg = drawn(&scene, background);
    let n_fig = drawn(&scene, figure);
    assert!(n_fig > 0, "the figure drew no dots");
    assert!(n_bg > 0, "the background drew no dots");

    // ── the figure's dots are inside the circle, the background's outside ──
    let radius = deg(45.0) * 0.5;
    let inside = |p: &[f32; 2]| {
        let dx = p[0] - RF_CENTER[0];
        let dy = p[1] - RF_CENTER[1];
        (dx * dx + dy * dy).sqrt() <= radius
    };
    let mut fig_buf = vec![Default::default(); dot_count as usize];
    let d = scene.config.stimuli[&figure].stimulus.dots().unwrap();
    let n = d.write_instances(&mut fig_buf) as usize;
    for i in &fig_buf[..n] {
        assert!(inside(&i.pos_px), "a figure dot at {:?} fell outside the circle", i.pos_px);
    }
    let mut bg_buf = vec![Default::default(); dot_count as usize];
    let d = scene.config.stimuli[&background].stimulus.dots().unwrap();
    let n = d.write_instances(&mut bg_buf) as usize;
    for i in &bg_buf[..n] {
        assert!(!inside(&i.pos_px), "a background dot at {:?} fell inside the circle", i.pos_px);
    }

    // ── the trial: 60 pre-vis frames + 120 visible frames, at 60 Hz ──
    advance(&mut scene, 180, 60.0);

    // Directions are orthogonal and both fields keep moving: the background right,
    // the figure up.
    let step = deg(50.0) / 60.0;
    let mut check = |h, expect: [f32; 2]| {
        let before = positions(&scene, h);
        advance(&mut scene, 1, 60.0);
        let after = positions(&scene, h);
        let moved: Vec<[f32; 2]> = before
            .iter()
            .zip(&after)
            .map(|(b, a)| [a[0] - b[0], a[1] - b[1]])
            .filter(|d| d[0].abs() < 100.0 && d[1].abs() < 100.0) // ignore the frame's wraps
            .collect();
        for m in &moved {
            assert!(
                (m[0] - expect[0] * step).abs() < 1e-2 && (m[1] - expect[1] * step).abs() < 1e-2,
                "expected {expect:?} × {step}, got {m:?}"
            );
        }
    };
    check(background, [1.0, 0.0]);
    check(figure, [0.0, 1.0]);

    // ── density is still exactly what it was: wrapping loses no dots ──
    assert_eq!(positions(&scene, background).len(), dot_count as usize);
    for p in positions(&scene, background) {
        assert!(
            p[0].abs() <= SCREEN[0] * 0.5 + 1e-3 && p[1].abs() <= SCREEN[1] * 0.5 + 1e-3,
            "a dot escaped the field at {p:?}"
        );
    }
}

/// `noFigureFrames`: the figure field moves with the *background's* direction until
/// a given frame, then switches to its own — continuing from wherever its dots are,
/// not from a line through their birth positions.
#[test]
fn a_mid_trial_direction_switch_is_continuous() {
    let mut scene = SceneState::new();
    let h = create_dots(
        &mut scene,
        proto::DotsParams {
            dot_count: 200,
            coherence: Some(1.0),
            direction_deg: 0.0,
            speed_px_per_s: Some(600.0),
            seed: 3,
            ..Default::default()
        },
        [0.0, 0.0],
    );
    advance(&mut scene, 30, 60.0); // the noFigureFrames stretch
    let before = positions(&scene, h);

    let resp = scene.handle_request(
        req(
            stim(h),
            request::Body::SetDotsDirection(proto::SetDotsDirectionRequest { direction_deg: 90.0 }),
        ),
        None,
    );
    assert_eq!(resp.code, proto::ErrorCode::Ok as i32);
    advance(&mut scene, 1, 60.0);

    let step = 600.0 / 60.0;
    for (b, a) in before.iter().zip(positions(&scene, h)) {
        let d = [a[0] - b[0], a[1] - b[1]];
        if d[0].abs() > 100.0 || d[1].abs() > 100.0 {
            continue; // wrapped this frame
        }
        assert!(d[0].abs() < 1e-3, "the switch moved a dot sideways by {}", d[0]);
        assert!((d[1] - step).abs() < 1e-3, "expected +{step} px up, got {}", d[1]);
    }
}

// ── The direction convention ──────────────────────────────────────────────────

/// Every angle in vstimd is CCW degrees with 0° = right, and a dot field is no
/// exception. This pins the three places that convention has to agree, because
/// each of them arrives at it by different arithmetic and nothing but a test
/// connects them: `Transform2D::angle_deg` (documented), the grating's
/// `drift_angle_deg` (projected against the stripe orientation in
/// `grating_phase_inc`), and the dot field's `direction_deg` (a unit vector in
/// `DotsParams::direction_unit`).
///
/// A mismatch here is the kind that renders — a stimulus drifting the wrong way is
/// still a stimulus — so it would be found by eye or not at all.
#[test]
fn dots_and_grating_measure_angles_in_the_same_frame() {
    use vstimd::scene::DotsParams;
    use vstimd::scene::stimulus::grating::{Grating, GratingParams, grating_phase_inc};

    // The dot field steps along (cos θ, sin θ) — 0° right, 90° up.
    for deg in [0.0f32, 45.0, 90.0, 180.0, 270.0] {
        let [x, y] = DotsParams { direction_deg: deg, ..Default::default() }.direction_unit();
        let r = deg.to_radians();
        assert!((x - r.cos()).abs() < 1e-6 && (y - r.sin()).abs() < 1e-6, "at {deg}°");
    }

    // The grating measures `drift_angle_deg` in that same frame: an uncoupled drift
    // along the stripe orientation advances at full rate, and one at right angles
    // to it does not advance at all. If the two used opposite senses of "CCW", this
    // is where it would show.
    let uncoupled = |grating_deg: f32, drift_deg: f32| {
        let g = Grating::new(
            [0.0, 0.0],
            grating_deg,
            [200.0, 200.0],
            GratingParams {
                drift_speed_hz: 1.0,
                drift_coupled: false,
                drift_angle_deg: drift_deg,
                ..Default::default()
            },
        );
        grating_phase_inc(&g, 60.0)
    };
    let coupled = Grating::new(
        [0.0, 0.0],
        30.0,
        [200.0, 200.0],
        GratingParams { drift_speed_hz: 1.0, drift_coupled: true, ..Default::default() },
    );
    let full = grating_phase_inc(&coupled, 60.0);

    assert!((uncoupled(30.0, 30.0) - full).abs() < 1e-6, "aligned drift is full rate");
    assert!(uncoupled(30.0, 120.0).abs() < 1e-6, "perpendicular drift does not advance");
    // 90° apart in the *other* rotational sense is equally perpendicular, so the
    // test above cannot tell the two senses apart on its own. This one can: at 60°
    // the projection is +½ of full rate, and it would be −½ under a mirrored frame.
    assert!(
        (uncoupled(0.0, 60.0) - full * 0.5).abs() < 1e-6,
        "a 60° offset projects to +half rate, not minus"
    );
    let [x, _] = DotsParams { direction_deg: 60.0, ..Default::default() }.direction_unit();
    assert!((x - 0.5).abs() < 1e-6, "and the dot field agrees on the sign of that projection");
}

/// 90° is *up*, in the same sense the shapes are drawn.
///
/// The whole stack maps a pixel `y` to clip `y` without negating — `render::tess`'s
/// `px_to_ndc`, the grating vertex shader, and `dots.wgsl` alike — and the text path
/// fixes clip `+1` as the top of the screen. So a positive `y` step is upward, which
/// is what `Transform2D` documents and what a Psychtoolbox `3*pi/2` has to become.
#[test]
fn ninety_degrees_moves_dots_toward_positive_y() {
    let mut scene = SceneState::new();
    let h = create_dots(
        &mut scene,
        proto::DotsParams {
            dot_count: 32,
            coherence: Some(1.0),
            direction_deg: 90.0,
            speed_px_per_s: Some(60.0),
            seed: 11,
            ..Default::default()
        },
        [0.0, 0.0],
    );
    let before = positions(&scene, h);
    advance(&mut scene, 1, 60.0);
    for (b, a) in before.iter().zip(positions(&scene, h)) {
        assert!((a[0] - b[0]).abs() < 1e-4, "90° must not move a dot sideways");
        assert!((a[1] - b[1] - 1.0).abs() < 1e-4, "90° must move a dot toward +y");
    }
}
