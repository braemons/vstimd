//! The camera as an animation target, and `LinearNav3D` (#74).
//!
//! Created over `handle_request` and advanced with `advance_animations` — no GPU.

use vstimd::proto;
use vstimd::proto::request::{self, Body};
use vstimd::scene::SceneState;
use vstimd::scene::animation::AnimationTarget;
use vstimd::vtl_state::{VtlEdges, VtlOutputs};

fn sys() -> request::Target {
    request::Target::System(proto::SystemTarget {})
}

fn send(scene: &mut SceneState, body: Body) -> proto::Response {
    scene.handle_request(proto::Request { target: Some(sys()), body: Some(body) }, None)
}

fn code(resp: &proto::Response) -> proto::ErrorCode {
    proto::ErrorCode::try_from(resp.code).unwrap()
}

fn camera_target() -> Option<proto::AnimationTarget> {
    Some(proto::AnimationTarget {
        target: Some(proto::animation_target::Target::Camera(proto::AnimationCamera {})),
    })
}

fn nav(speed_cm_per_s: f32, wrap_period_cm: f32) -> Option<proto::create_animation_request::Body> {
    Some(proto::create_animation_request::Body::LinearNav3d(proto::LinearNav3D {
        speed_cm_per_s,
        wrap_period_cm,
        source: None,
        ..Default::default()
    }))
}

/// Create and arm a camera navigation animation; returns its handle.
fn start_nav(scene: &mut SceneState, speed: f32, wrap: f32) -> u32 {
    let resp = send(scene, Body::CreateAnimation(proto::CreateAnimationRequest {
        target: camera_target(),
        body: nav(speed, wrap),
        ..Default::default()
    }));
    assert_eq!(code(&resp), proto::ErrorCode::Ok, "{}", resp.error);
    let handle = resp.handle as u32;
    let resp = send(scene, Body::ArmAnimation(proto::ArmAnimationRequest { handle }));
    assert_eq!(code(&resp), proto::ErrorCode::Ok, "{}", resp.error);
    handle
}

fn advance(scene: &mut SceneState, frames: u32) {
    let (mut levels, mut pulses) = ([0u64; vtl::MAX_BANKS], [0u64; vtl::MAX_BANKS]);
    for _ in 0..frames {
        scene.advance_animations(
            &VtlEdges::default(),
            &VtlEdges::default(),
            &mut VtlOutputs { levels: &mut levels, pulses: &mut pulses },
        );
    }
}

fn distance(scene: &mut SceneState, handle: u32) -> f64 {
    let resp = send(scene, Body::QueryAnimation(proto::QueryAnimationRequest { handle }));
    let Some(proto::response::Body::QueryAnimationResponse(q)) = resp.body else {
        panic!("{resp:?}");
    };
    q.distance_travelled_cm
}

fn scene_at_60hz() -> SceneState {
    let mut scene = SceneState::new();
    scene.runtime.nominal_frame_rate_hz = 60.0;
    scene
}

#[test]
fn nav_moves_forward_and_wraps_while_distance_does_not() {
    let mut scene = scene_at_60hz();
    let h = start_nav(&mut scene, 10.0, 100.0);

    advance(&mut scene, 25);
    let step = 10.0 / 60.0;
    let travelled = 25.0 * step;
    assert!((distance(&mut scene, h) - travelled).abs() < 1e-9);
    // Forward is -Z, so 4.17 cm ahead of 0 wraps to 100 - 4.17.
    let z = scene.camera.live.position_cm.0.z;
    assert!((f64::from(z) - (100.0 - travelled)).abs() < 1e-4, "z = {z}");

    advance(&mut scene, 675);
    let travelled = 700.0 * step; // 116.67 cm: more than one period
    assert!((distance(&mut scene, h) - travelled).abs() < 1e-9, "distance is never wrapped");
    let z = f64::from(scene.camera.live.position_cm.0.z);
    assert!((z - (100.0 - (travelled - 100.0))).abs() < 1e-3, "z = {z}");
    assert!((0.0..100.0).contains(&z));
    assert_eq!(scene.camera.live.position_cm.0.x, 0.0);
}

#[test]
fn nav_follows_the_camera_yaw_and_holds_height() {
    let mut scene = scene_at_60hz();
    scene.camera.live.yaw_deg = 90.0; // facing -X
    scene.camera.live.position_cm.0.y = 12.0;
    start_nav(&mut scene, 60.0, 0.0);
    advance(&mut scene, 60);
    let p = scene.camera.live.position_cm.0;
    assert!((p.x + 60.0).abs() < 1e-3 && p.z.abs() < 1e-3, "{p}");
    assert_eq!(p.y, 12.0);
}

#[test]
fn long_sessions_keep_the_position_exact() {
    let mut scene = scene_at_60hz();
    let h = start_nav(&mut scene, 250.0, 500.0);
    advance(&mut scene, 60 * 60 * 10); // ten minutes: 1.5 km
    let travelled = distance(&mut scene, h);
    assert!((travelled - 150_000.0).abs() < 1e-6, "{travelled}");
    let z = f64::from(scene.camera.live.position_cm.0.z);
    let expected = (-travelled).rem_euclid(500.0);
    assert!((z - expected).abs() < 1e-2 || (z - expected).abs() > 499.99, "z = {z}, expected {expected}");
}

#[test]
fn set_nav_speed_changes_the_rate_and_reversing_moves_back() {
    let mut scene = scene_at_60hz();
    let h = start_nav(&mut scene, 30.0, 0.0);
    advance(&mut scene, 60);
    let resp = send(&mut scene, Body::SetNavSpeed(proto::SetNavSpeedRequest {
        handle: h,
        speed_cm_per_s: -30.0,
    }));
    assert_eq!(code(&resp), proto::ErrorCode::Ok);
    advance(&mut scene, 60);
    assert!(scene.camera.live.position_cm.0.z.abs() < 1e-3);
    assert!(distance(&mut scene, h).abs() < 1e-9, "net distance");
}

#[test]
fn deferred_blocks_do_not_undo_nav() {
    let mut scene = scene_at_60hz();
    start_nav(&mut scene, 60.0, 0.0);
    send(&mut scene, Body::SetDeferredMode(proto::SetDeferredModeRequest { active: true, cancel: false }));
    advance(&mut scene, 60);
    send(&mut scene, Body::SetDeferredMode(proto::SetDeferredModeRequest { active: false, cancel: false }));
    scene.apply_flip();
    assert!((scene.camera.live.position_cm.0.z + 60.0).abs() < 1e-3, "the flip must not rewind the camera");
}

#[test]
fn camera_pairings_are_checked() {
    let mut scene = SceneState::new();
    // A visibility animation cannot drive the camera.
    let resp = send(&mut scene, Body::CreateAnimation(proto::CreateAnimationRequest {
        target: camera_target(),
        body: Some(proto::create_animation_request::Body::FlashForNFrames(proto::FlashForNFrames {
            duration_frames: 3,
        })),
        ..Default::default()
    }));
    assert_eq!(code(&resp), proto::ErrorCode::InvalidArgument);
    // Nav cannot drive stimuli.
    let resp = send(&mut scene, Body::CreateAnimation(proto::CreateAnimationRequest {
        body: nav(10.0, 0.0),
        ..Default::default()
    }));
    assert_eq!(code(&resp), proto::ErrorCode::InvalidArgument);
    // Stimulus action bits on a camera animation.
    let resp = send(&mut scene, Body::CreateAnimation(proto::CreateAnimationRequest {
        target: camera_target(),
        body: nav(10.0, 0.0),
        final_action_mask: 0x01, // DISABLE
        ..Default::default()
    }));
    assert_eq!(code(&resp), proto::ErrorCode::InvalidArgument);
    assert!(scene.animations.is_empty());
}

#[test]
fn camera_target_round_trips_through_query_and_config() {
    let mut scene = scene_at_60hz();
    let h = start_nav(&mut scene, 10.0, 250.0);
    let resp = send(&mut scene, Body::QueryAnimation(proto::QueryAnimationRequest { handle: h }));
    let Some(proto::response::Body::QueryAnimationResponse(q)) = resp.body else { panic!() };
    let params = q.params.unwrap();
    assert!(matches!(
        params.target.and_then(|t| t.target),
        Some(proto::animation_target::Target::Camera(_))
    ));
    assert_eq!(q.type_name, "LinearNav3D");

    let json = serde_json::to_string(&scene.config).unwrap();
    let loaded: vstimd::scene::SceneConfig = serde_json::from_str(&json).unwrap();
    assert!(matches!(loaded.animations[&h].target, AnimationTarget::Camera));
}

// ── Finite track ──────────────────────────────────────────────────────────────

fn start_track(scene: &mut SceneState, speed: f32, length_cm: f32, fade_frames: u32) -> u32 {
    let resp = send(scene, Body::CreateAnimation(proto::CreateAnimationRequest {
        target: camera_target(),
        body: Some(proto::create_animation_request::Body::LinearNav3d(proto::LinearNav3D {
            speed_cm_per_s: speed,
            track_length_cm: length_cm,
            fade_frames,
            ..Default::default()
        })),
        ..Default::default()
    }));
    assert_eq!(code(&resp), proto::ErrorCode::Ok, "{}", resp.error);
    let handle = resp.handle as u32;
    let resp = send(scene, Body::ArmAnimation(proto::ArmAnimationRequest { handle }));
    assert_eq!(code(&resp), proto::ErrorCode::Ok, "{}", resp.error);
    handle
}

#[test]
fn a_track_fades_out_jumps_back_and_fades_in() {
    let mut scene = scene_at_60hz();
    scene.camera.live.position_cm.0 = glam::Vec3::new(0.0, 5.0, 20.0);
    // 60 cm/s → 1 cm a frame; the end is 30 cm ahead of z = 20.
    let h = start_track(&mut scene, 60.0, 30.0, 4);

    advance(&mut scene, 29);
    assert!((scene.camera.live.position_cm.0.z - -9.0).abs() < 1e-3);
    assert_eq!(scene.view_fade_3d(), 0.0);

    // Frame 30 reaches the end and starts the fade; the camera then holds.
    advance(&mut scene, 1);
    let end_z = scene.camera.live.position_cm.0.z;
    assert!((end_z - -10.0).abs() < 1e-3, "{end_z}");
    let mut fades = Vec::new();
    for _ in 0..3 {
        advance(&mut scene, 1);
        fades.push(scene.view_fade_3d());
        assert_eq!(scene.camera.live.position_cm.0.z, end_z, "holds while fading out");
    }
    assert!(fades.windows(2).all(|w| w[0] < w[1]), "fade rises: {fades:?}");

    // The jump happens on the fully faded frame.
    advance(&mut scene, 1);
    assert_eq!(scene.view_fade_3d(), 1.0);
    assert_eq!(scene.camera.live.position_cm.0, glam::Vec3::new(0.0, 5.0, 20.0));

    // Fading in, the camera moves again, and the fade clears.
    advance(&mut scene, 4);
    assert_eq!(scene.view_fade_3d(), 0.0);
    assert!(scene.camera.live.position_cm.0.z < 20.0);
    // Distance counts real movement only, never the jump.
    assert!((distance(&mut scene, h) - 34.0).abs() < 1e-6, "{}", distance(&mut scene, h));
}

#[test]
fn a_track_without_a_fade_jumps_at_once() {
    let mut scene = scene_at_60hz();
    start_track(&mut scene, 60.0, 10.0, 0);
    advance(&mut scene, 10);
    assert_eq!(scene.camera.live.position_cm.0.z, 0.0);
    assert_eq!(scene.view_fade_3d(), 0.0);
    advance(&mut scene, 3);
    assert!((scene.camera.live.position_cm.0.z - -3.0).abs() < 1e-3);
}

#[test]
fn a_track_and_a_wrap_are_refused_together() {
    let mut scene = scene_at_60hz();
    let resp = send(&mut scene, Body::CreateAnimation(proto::CreateAnimationRequest {
        target: camera_target(),
        body: Some(proto::create_animation_request::Body::LinearNav3d(proto::LinearNav3D {
            speed_cm_per_s: 10.0,
            wrap_period_cm: 100.0,
            track_length_cm: 100.0,
            ..Default::default()
        })),
        ..Default::default()
    }));
    assert_eq!(code(&resp), proto::ErrorCode::InvalidArgument);
}
