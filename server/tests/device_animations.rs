//! Animations driven by input devices (#79): `ExternalPosition2D`,
//! `DeviceDrivenTransform` and a device-sourced `LinearNav3D`.
//!
//! Real `vinput` segments, `advance_animations` on a bare `SceneState` — no GPU.

use std::time::Duration;

use vinput::{AxisDesc, Semantic, VinputOwner};
use vstimd::input::InputDevice;
use vstimd::input::devices::{Axis, Backend, open_checked};
use vstimd::proto;
use vstimd::proto::request::{self, Body};
use vstimd::scene::animation::{Animation, AnimationEntry, AnimationTarget, AxisMap, AxisRef, TransformChannel};
use vstimd::scene::{Pos2Px, SceneState};
use vstimd::vtl_state::{VtlEdges, VtlOutputs};

fn shm(tag: &str) -> String {
    format!("/vstimd_devanim_{}_{tag}", std::process::id())
}

/// A producer with one or more axes of `semantic`, and the scene's device for it.
fn attach(scene: &mut SceneState, tag: &str, axes: &[(&str, Semantic)], scale: f32) -> VinputOwner {
    let name = shm(tag);
    let descs: Vec<AxisDesc> = axes.iter().map(|(n, s)| AxisDesc::new(*n, *s, 1.0)).collect();
    let owner = VinputOwner::create(&name, &descs).unwrap();
    let rig_axes: Vec<Axis> = axes
        .iter()
        .map(|(n, s)| Axis { name: n.to_string(), semantic: *s, scale, deadzone: 0.0 })
        .collect();
    let client = open_checked(&name, &rig_axes).unwrap();
    scene.runtime.input.devices.push(InputDevice::new(
        tag,
        rig_axes,
        Duration::from_millis(200),
        Backend::Shm { shm_name: name, client: Some(client) },
    ));
    owner
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

fn rect(scene: &mut SceneState) -> u32 {
    scene.handle_request(
        proto::Request {
            target: Some(request::Target::System(proto::SystemTarget {})),
            body: Some(Body::CreateRect(proto::CreateRectRequest::default())),
        },
        None,
    )
    .handle as u32
}

fn add_armed(scene: &mut SceneState, animation: Animation, target: AnimationTarget) -> u32 {
    let stimuli = target.stimuli().to_vec();
    let mut entry = AnimationEntry::armed(animation, stimuli);
    entry.config.target = target;
    scene.add_animation(entry)
}

fn map(axis: &str, channel: TransformChannel, gain: f32) -> AxisMap {
    AxisMap { axis: axis.into(), channel, gain, deadzone: 0.0, invert: false, clamp: None, wrap: None }
}

fn scene_60hz() -> SceneState {
    let mut s = SceneState::new();
    s.runtime.nominal_frame_rate_hz = 60.0;
    s
}

#[test]
fn external_position_2d_actually_moves_the_stimulus() {
    let mut scene = scene_60hz();
    let mut owner = attach(&mut scene, "gaze", &[("x", Semantic::Absolute), ("y", Semantic::Absolute)], 1.0);
    let h = rect(&mut scene);
    add_armed(
        &mut scene,
        Animation::ExternalPosition2D { shm_name: "gaze".into(), x_offset_px: 10.0, y_offset_px: -5.0 },
        AnimationTarget::Stimuli { handles: vec![h] },
    );
    owner.write(&[100.0, 50.0]);
    advance(&mut scene, 1);
    assert_eq!(scene.stimuli[&h].stimulus.get_pos_2d(), Some(Pos2Px::new(110.0, 45.0)));
    owner.write(&[-20.0, 30.0]);
    advance(&mut scene, 1);
    assert_eq!(scene.stimuli[&h].stimulus.get_pos_2d(), Some(Pos2Px::new(-10.0, 25.0)));
}

#[test]
fn a_cumulative_axis_moves_once_per_change() {
    let mut scene = scene_60hz();
    let mut owner = attach(&mut scene, "wheel", &[("ticks", Semantic::Cumulative)], 1.0);
    let h = rect(&mut scene);
    add_armed(
        &mut scene,
        Animation::DeviceDrivenTransform { device: "wheel".into(), axes: vec![map("ticks", TransformChannel::PosX, 0.5)] },
        AnimationTarget::Stimuli { handles: vec![h] },
    );
    owner.write(&[0.0]);
    advance(&mut scene, 1); // baseline
    let mut total = 0.0;
    for _ in 0..1000 {
        total += 1.0;
        owner.write(&[total]);
    }
    advance(&mut scene, 1);
    assert_eq!(scene.stimuli[&h].stimulus.get_pos_2d().unwrap().x(), 500.0);
    advance(&mut scene, 1);
    assert_eq!(scene.stimuli[&h].stimulus.get_pos_2d().unwrap().x(), 500.0, "no change, no motion");
}

#[test]
fn a_rate_axis_integrates_over_real_frame_time() {
    let mut scene = scene_60hz();
    let mut owner = attach(&mut scene, "stick", &[("speed", Semantic::Rate)], 1.0);
    add_armed(
        &mut scene,
        Animation::DeviceDrivenTransform { device: "stick".into(), axes: vec![map("speed", TransformChannel::Forward, 1.0)] },
        AnimationTarget::Camera,
    );
    owner.write(&[10.0]); // 10 cm/s
    for _ in 0..60 {
        owner.write(&[10.0]);
        advance(&mut scene, 1);
    }
    let z = scene.camera.live.position_cm.0.z;
    assert!((z + 10.0).abs() < 1e-3, "60 frames at 60 Hz, 10 cm/s: z = {z}");

    // A dropped frame: the render loop reports two vblanks for one frame.
    scene.runtime.vblanks_elapsed = 2;
    owner.write(&[10.0]);
    advance(&mut scene, 1);
    let z2 = scene.camera.live.position_cm.0.z;
    assert!((z2 - z + 2.0 * 10.0 / 60.0).abs() < 1e-4, "must integrate over the doubled interval");
}

#[test]
fn deadzone_invert_and_clamp() {
    let mut scene = scene_60hz();
    let mut owner = attach(&mut scene, "joy", &[("x", Semantic::Absolute)], 1.0);
    let h = rect(&mut scene);
    let mut m = map("x", TransformChannel::PosY, 10.0);
    m.deadzone = 0.1;
    m.invert = true;
    m.clamp = Some([-50.0, 50.0]);
    add_armed(
        &mut scene,
        Animation::DeviceDrivenTransform { device: "joy".into(), axes: vec![m] },
        AnimationTarget::Stimuli { handles: vec![h] },
    );
    let y = |s: &SceneState| s.stimuli[&h].stimulus.get_pos_2d().unwrap().y();
    owner.write(&[0.05]);
    advance(&mut scene, 1);
    assert_eq!(y(&scene), 0.0, "within the deadzone");
    owner.write(&[2.0]);
    advance(&mut scene, 1);
    assert_eq!(y(&scene), -20.0, "inverted");
    owner.write(&[-9.0]);
    advance(&mut scene, 1);
    assert_eq!(y(&scene), 50.0, "clamped");
}

#[test]
fn device_sourced_nav_keeps_distance_and_wraps_position() {
    let mut scene = scene_60hz();
    let mut owner = attach(&mut scene, "treadmill", &[("distance", Semantic::Cumulative)], 0.5);
    let nav = add_armed(
        &mut scene,
        Animation::LinearNav3D {
            speed_cm_per_s: 999.0, // ignored with a source
            wrap_period_cm: Some(100.0),
            source: Some(AxisRef { device: "treadmill".into(), axis: "distance".into() }),
            track: None,
        },
        AnimationTarget::Camera,
    );
    owner.write(&[0.0]);
    advance(&mut scene, 1);
    let mut counts = 0.0;
    for _ in 0..500 {
        counts += 3.0; // 1.5 cm per frame after scale
        owner.write(&[counts]);
        advance(&mut scene, 1);
        let z = scene.camera.live.position_cm.0.z;
        assert!((0.0..100.0).contains(&z), "z = {z}");
    }
    let travelled = scene.animations[&nav].distance_travelled_cm;
    assert!((travelled - 750.0).abs() < 1e-9, "{travelled}");
    let z = f64::from(scene.camera.live.position_cm.0.z);
    assert!((z - (-750.0f64).rem_euclid(100.0)).abs() < 1e-3, "z = {z}");
}

#[test]
fn a_stale_device_stops_motion_without_stopping_the_animation() {
    let mut scene = scene_60hz();
    let mut owner = attach(&mut scene, "flaky", &[("speed", Semantic::Rate)], 1.0);
    let anim = add_armed(
        &mut scene,
        Animation::DeviceDrivenTransform { device: "flaky".into(), axes: vec![map("speed", TransformChannel::Forward, 1.0)] },
        AnimationTarget::Camera,
    );
    owner.write(&[60.0]);
    advance(&mut scene, 1);
    let moving = scene.camera.live.position_cm.0.z;
    assert!(moving < 0.0);
    std::thread::sleep(Duration::from_millis(250)); // past stale_after
    advance(&mut scene, 30);
    assert_eq!(scene.camera.live.position_cm.0.z, moving, "a silent producer must not keep the camera going");
    assert!(matches!(scene.animations[&anim].state, vstimd::scene::AnimState::Running { .. }));
}

#[test]
fn nonsense_pairings_are_refused_at_create() {
    use vstimd::scene::animation::animation_input::check_animation;
    let mut scene = scene_60hz();
    let _o = attach(&mut scene, "mixed", &[("pos", Semantic::Absolute), ("spd", Semantic::Rate)], 1.0);
    let ddt = |channel, axis: &str| Animation::DeviceDrivenTransform {
        device: "mixed".into(),
        axes: vec![map(axis, channel, 1.0)],
    };
    let input = &scene.runtime.input;
    let stimuli = AnimationTarget::Stimuli { handles: vec![] };
    assert!(check_animation(&AnimationTarget::Camera, &ddt(TransformChannel::Forward, "pos"), input).is_err(),
        "absolute on Forward");
    assert!(check_animation(&stimuli, &ddt(TransformChannel::Forward, "spd"), input).is_err(),
        "Forward without the camera");
    assert!(check_animation(&AnimationTarget::Camera, &ddt(TransformChannel::ScaleX, "spd"), input).is_err());
    assert!(check_animation(&stimuli, &ddt(TransformChannel::PosX, "nope"), input).is_err(), "unknown axis");
    assert!(check_animation(&AnimationTarget::Camera, &ddt(TransformChannel::Forward, "spd"), input).is_ok());
    let nav_abs = Animation::LinearNav3D {
        speed_cm_per_s: 0.0,
        wrap_period_cm: None,
        source: Some(AxisRef { device: "mixed".into(), axis: "pos".into() }),
        track: None,
    };
    assert!(check_animation(&AnimationTarget::Camera, &nav_abs, input).is_err());
    let ext = Animation::ExternalPosition2D { shm_name: "nobody".into(), x_offset_px: 0.0, y_offset_px: 0.0 };
    assert!(check_animation(&stimuli, &ext, input).is_err());
}

#[test]
fn a_device_driven_animation_saves_the_device_name() {
    let mut scene = scene_60hz();
    add_armed(
        &mut scene,
        Animation::DeviceDrivenTransform { device: "treadmill".into(), axes: vec![map("distance", TransformChannel::Forward, 1.0)] },
        AnimationTarget::Camera,
    );
    let json = serde_json::to_string(&scene.config).unwrap();
    assert!(json.contains("\"treadmill\""));
    assert!(!json.contains("vstimd_"), "no segment or runtime mapping in the config: {json}");
    let back: vstimd::scene::SceneConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.animations.len(), 1);
}

#[test]
fn list_input_devices_and_query_report_staleness() {
    let mut scene = scene_60hz();
    let mut owner = attach(&mut scene, "listed", &[("speed", Semantic::Rate)], 2.0);
    let anim = add_armed(
        &mut scene,
        Animation::DeviceDrivenTransform { device: "listed".into(), axes: vec![map("speed", TransformChannel::Forward, 1.0)] },
        AnimationTarget::Camera,
    );
    owner.write(&[5.0]);
    advance(&mut scene, 1);
    let sys = || Some(request::Target::System(proto::SystemTarget {}));
    let resp = scene.handle_request(
        proto::Request { target: sys(), body: Some(Body::ListInputDevices(proto::ListInputDevicesRequest {})) },
        None,
    );
    let Some(proto::response::Body::InputDeviceList(list)) = resp.body else { panic!("{resp:?}") };
    let d = &list.devices[0];
    assert_eq!(d.name, "listed");
    assert!(d.connected && !d.stale);
    assert_eq!(d.axes[0].semantic, proto::InputSemantic::Rate as i32);
    assert_eq!(d.axes[0].value, 10.0);

    let query = |scene: &mut SceneState| {
        let resp = scene.handle_request(
            proto::Request {
                target: sys(),
                body: Some(Body::QueryAnimation(proto::QueryAnimationRequest { handle: anim })),
            },
            None,
        );
        let Some(proto::response::Body::QueryAnimationResponse(q)) = resp.body else { panic!() };
        q
    };
    let q = query(&mut scene);
    assert!(q.device_backend.starts_with("shm ") && !q.device_stale);
    std::thread::sleep(Duration::from_millis(250));
    advance(&mut scene, 1);
    assert!(query(&mut scene).device_stale);
}
