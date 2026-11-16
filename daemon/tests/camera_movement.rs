//! Movement constraints through the real animation engine; no GPU or shm needed.
use vstimd::input::devices::{Axis, Backend, InputDevice};
use vstimd::scene::animation::{
    Animation, AnimationEntry, AnimationTarget, AxisMap, AxisRef, Track3D, TransformChannel,
};
use vstimd::scene::camera_movement::{CameraMovement, Obstacle};
use vstimd::scene::{LoadMode, Pos3Cm, SceneState};
use vstimd::scene_config_file::{parse_config_json, retrieve_config_json};
use vstimd::vtl_state::{VtlConfig, VtlEdges, VtlOutputs};

fn scene(wall: bool) -> SceneState {
    let mut s = SceneState::new();
    s.runtime.nominal_frame_rate_hz = 60.0;
    s.camera.live.position_cm = Pos3Cm::new(0.0, 10.0, 0.0);
    s.camera.copy = s.camera.live;
    s.config.camera_movement = Some(CameraMovement {
        start_position_cm: [0.0, 10.0, 0.0],
        start_yaw_deg: 0.0,
        clearance_cm: 1.0,
        end_blackout_ms: 0,
        obstacles: if wall {
            vec![Obstacle::Box {
                position_cm: [0.0, 10.0, -50.0],
                yaw_deg: 0.0,
                size_cm: [100.0, 20.0, 0.1],
            }]
        } else {
            vec![]
        },
    });
    s
}
fn nav(s: &mut SceneState, source: Option<AxisRef>, track: Option<Track3D>) -> u32 {
    let mut e = AnimationEntry::armed(
        Animation::LinearNav3D {
            speed_cm_per_s: 60000.0,
            wrap_period_cm: None,
            source,
            track,
        },
        vec![],
    );
    e.config.target = AnimationTarget::Camera;
    s.add_animation(e)
}
fn advance(s: &mut SceneState) {
    s.advance_animations(
        &VtlEdges::default(),
        &VtlEdges::default(),
        &mut VtlOutputs {
            levels: &mut [0; vtl::MAX_BANKS],
            pulses: &mut [0; vtl::MAX_BANKS],
        },
    );
}
static KEYBOARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct WheelGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}
impl Drop for WheelGuard {
    fn drop(&mut self) {
        vstimd::input::keyboard_axes::set_arrow(vstimd::input::keyboard_axes::Arrow::Up, false);
    }
}
fn wheel(s: &mut SceneState) -> WheelGuard {
    let guard = WheelGuard {
        _lock: KEYBOARD.lock().unwrap_or_else(|e| e.into_inner()),
    };
    vstimd::input::keyboard_axes::set_arrow(vstimd::input::keyboard_axes::Arrow::Up, true);
    s.runtime.input.devices.push(InputDevice::new(
        "wheel",
        vec![Axis {
            name: "wheel".into(),
            semantic: vinput::Semantic::Cumulative,
            scale: 1.0,
            deadzone: 0.0,
        }],
        std::time::Duration::from_secs(1),
        Backend::Keyboard { speed: 0.0 },
    ));
    advance(s); // establish the cumulative baseline before any movement
    guard
}
fn set_delta(s: &mut SceneState, delta: f64) {
    let speed = delta / s.frame_dt_s();
    s.runtime.input.devices[0].backend = Backend::Keyboard { speed };
}

#[test]
fn cumulative_wheel_stops_at_wall_and_reverses_immediately() {
    let mut s = scene(true);
    let _wheel = wheel(&mut s);
    let h = nav(
        &mut s,
        Some(AxisRef {
            device: "wheel".into(),
            axis: "wheel".into(),
        }),
        None,
    );
    set_delta(&mut s, 1000.0);
    advance(&mut s);
    let stopped = s.camera.live.position_cm.0.z;
    assert!(stopped < -48.0 && stopped > -49.0);
    advance(&mut s);
    assert!((s.camera.live.position_cm.0.z - stopped).abs() < 1e-5);
    set_delta(&mut s, -5.0);
    advance(&mut s);
    assert!((s.camera.live.position_cm.0.z - stopped - 5.0).abs() < 1e-5);
    set_delta(&mut s, -1000.0);
    advance(&mut s);
    assert!(s.camera.live.position_cm.0.z.abs() < 1e-5);
    assert!(s.animations[&h].distance_travelled_cm.abs() < 1e-4);
    set_delta(&mut s, 5.0);
    advance(&mut s);
    assert!((s.camera.live.position_cm.0.z + 5.0).abs() < 1e-5);
    assert_eq!(s.camera.live.position_cm, s.camera.copy.position_cm);
}
#[test]
fn device_driven_camera_uses_the_same_collision_sweep() {
    let mut s = scene(true);
    let _wheel = wheel(&mut s);
    let mut e = AnimationEntry::armed(
        Animation::DeviceDrivenTransform {
            device: "wheel".into(),
            axes: vec![AxisMap {
                axis: "wheel".into(),
                channel: TransformChannel::Forward,
                gain: 1.0,
                deadzone: 0.0,
                invert: false,
                clamp: None,
                wrap: None,
            }],
        },
        vec![],
    );
    e.config.target = AnimationTarget::Camera;
    s.add_animation(e);
    set_delta(&mut s, 1000.0);
    advance(&mut s);
    assert!(s.camera.live.position_cm.0.z > -49.0 && s.camera.live.position_cm.0.z < -48.0);
    set_delta(&mut s, -1000.0);
    advance(&mut s);
    assert!(s.camera.live.position_cm.0.z.abs() < 1e-5);
}
#[test]
fn finite_track_return_preserves_start_boundary_and_wall_can_prevent_return() {
    for fade_frames in [0, 3] {
        let mut s = scene(false);
        let _wheel = wheel(&mut s);
        nav(
            &mut s,
            Some(AxisRef {
                device: "wheel".into(),
                axis: "wheel".into(),
            }),
            Some(Track3D {
                length_cm: 20.0,
                fade_frames,
            }),
        );
        set_delta(&mut s, 21.0);
        advance(&mut s);
        set_delta(&mut s, 0.0);
        for _ in 0..fade_frames {
            advance(&mut s);
        }
        assert!(s.camera.live.position_cm.0.z.abs() < 1e-5);
        set_delta(&mut s, -100.0);
        advance(&mut s);
        assert!(s.camera.live.position_cm.0.z.abs() < 1e-5);
    }
    let mut s = scene(true);
    nav(
        &mut s,
        None,
        Some(Track3D {
            length_cm: 100.0,
            fade_frames: 0,
        }),
    );
    for _ in 0..10 {
        advance(&mut s);
    }
    assert!(s.camera.live.position_cm.0.z < -48.0 && s.camera.live.position_cm.0.z > -49.0);
}
#[test]
fn v6_roundtrip_replace_validation_and_v5_compatibility() {
    let s = scene(true);
    let json = retrieve_config_json(&s.config, &VtlConfig::default()).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json).unwrap()["version"],
        6
    );
    let (config, _) = parse_config_json(&json).unwrap();
    let mut restored = SceneState::new();
    restored.load_snapshot(config, LoadMode::Replace);
    nav(&mut restored, None, None);
    advance(&mut restored);
    assert!(restored.camera.live.position_cm.0.z > -49.0);
    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
    value["version"] = 5.into();
    assert!(parse_config_json(&value.to_string()).is_err());
    value["version"] = 6.into();
    value["scene"]["camera_movement"]["obstacles"][0]["size_cm"][0] = (-1).into();
    assert!(parse_config_json(&value.to_string()).is_err());
    let legacy = retrieve_config_json(&SceneState::new().config, &VtlConfig::default()).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&legacy).unwrap()["version"],
        5
    );
    assert!(parse_config_json(&legacy).is_ok());
}
#[test]
fn additive_upload_is_refused_before_saving_or_mutating() {
    use vstimd::proto;
    let mut s = scene(true);
    let request = proto::Request {
        target: Some(proto::request::Target::System(proto::SystemTarget {})),
        body: Some(proto::request::Body::UploadSceneConfig(
            proto::UploadSceneConfigRequest {
                name: "test/blocked".into(),
                json: retrieve_config_json(&s.config, &VtlConfig::default()).unwrap(),
                apply_now: true,
                additive: true,
                overwrite: false,
            },
        )),
    };
    let response = s.handle_request(request, None);
    assert_eq!(response.code, proto::ErrorCode::InvalidArgument as i32);
    assert!(response.error.contains("replace"));
}

#[test]
fn editor_export_loads_and_its_native_wheel_animation_stops_at_rotated_wall() {
    // Generated by editor exportScene, not a scene hand-written to match Rust.
    let json = include_str!("configs/editor_collision_v6.config.json");
    let (config, _) = parse_config_json(json).unwrap();
    assert_eq!(config.camera_movement.as_ref().unwrap().obstacles.len(), 5);
    let mut s = SceneState::new();
    s.load_snapshot(config, LoadMode::Replace);
    let _wheel = wheel(&mut s);
    set_delta(&mut s, 1000.0);
    advance(&mut s);
    assert!(s.camera.live.position_cm.0.z < -45.0 && s.camera.live.position_cm.0.z > -50.0);
    let retrieved = retrieve_config_json(&s.config, &VtlConfig::default()).unwrap();
    let (expected, _) = parse_config_json(json).unwrap();
    let (actual_scene, _) = parse_config_json(&retrieved).unwrap();
    assert_eq!(actual_scene.camera_movement, expected.camera_movement);
    let actual: serde_json::Value = serde_json::from_str(&retrieved).unwrap();
    assert_eq!(actual["version"], 6);
}
#[test]
fn constrained_scene_refuses_wrapping_animation_through_rpc() {
    use vstimd::proto;
    let mut s = scene(false);
    let response = s.handle_request(
        proto::Request {
            target: Some(proto::request::Target::System(proto::SystemTarget {})),
            body: Some(proto::request::Body::CreateAnimation(
                proto::CreateAnimationRequest {
                    target: Some(proto::AnimationTarget {
                        target: Some(proto::animation_target::Target::Camera(
                            proto::AnimationCamera {},
                        )),
                    }),
                    body: Some(proto::create_animation_request::Body::LinearNav3d(
                        proto::LinearNav3D {
                            speed_cm_per_s: 10.0,
                            wrap_period_cm: 100.0,
                            ..Default::default()
                        },
                    )),
                    ..Default::default()
                },
            )),
        },
        None,
    );
    assert_eq!(response.code, proto::ErrorCode::InvalidArgument as i32);
}

#[test]
fn resumed_track_returns_to_declared_start_instead_of_saved_mid_run_camera() {
    let mut s = scene(false);
    s.camera.live.position_cm = Pos3Cm::new(0.0, 10.0, -15.0);
    s.camera.copy = s.camera.live;
    nav(
        &mut s,
        None,
        Some(Track3D {
            length_cm: 20.0,
            fade_frames: 0,
        }),
    );
    advance(&mut s);
    assert_eq!(s.camera.live.position_cm, Pos3Cm::new(0.0, 10.0, 0.0));
}

#[test]
fn end_blackout_is_timed_by_deadline_and_discards_input_before_return() {
    use vstimd::scene::animation::TrackPhase;
    let mut s = scene(false);
    s.config.camera_movement.as_mut().unwrap().end_blackout_ms = 2000;
    let _wheel = wheel(&mut s);
    let h = nav(
        &mut s,
        Some(AxisRef {
            device: "wheel".into(),
            axis: "wheel".into(),
        }),
        Some(Track3D {
            length_cm: 20.0,
            fade_frames: 15,
        }),
    );
    let before = std::time::Instant::now();
    set_delta(&mut s, 1000.0);
    advance(&mut s);
    assert!(s.camera_blackout());
    assert_eq!(s.camera.live.position_cm.0.z, -20.0);
    let TrackPhase::BlackoutUntil(deadline) = s.animations[&h].track_progress.phase else {
        panic!("missing hold")
    };
    assert!(deadline.duration_since(before) >= std::time::Duration::from_millis(2000));
    assert!(deadline.duration_since(before) < std::time::Duration::from_millis(2100));
    // Nominal frame counts and dropped vblanks do not end the monotonic hold.
    s.runtime.vblanks_elapsed = 10000;
    set_delta(&mut s, -1000.0);
    advance(&mut s);
    assert!(s.camera_blackout());
    assert_eq!(s.camera.live.position_cm.0.z, -20.0);
    assert_eq!(
        s.animations[&h].track_progress.phase,
        TrackPhase::BlackoutUntil(deadline)
    );
    // Advance the injected runtime deadline, without sleeping during tests.
    s.animations.get_mut(&h).unwrap().track_progress.phase =
        TrackPhase::BlackoutUntil(std::time::Instant::now() - std::time::Duration::from_millis(1));
    set_delta(&mut s, 1000.0);
    advance(&mut s);
    assert!(!s.camera_blackout());
    assert_eq!(s.camera.live.position_cm, Pos3Cm::new(0.0, 10.0, 0.0));
    assert_eq!(s.animations[&h].track_progress.laps, 1);
    assert!((s.animations[&h].distance_travelled_cm - 20.0).abs() < 1e-6);
    set_delta(&mut s, 5.0);
    advance(&mut s);
    assert_eq!(s.camera.live.position_cm.0.z, -5.0);
}

#[test]
fn inactive_held_navigation_does_not_black_out_another_condition() {
    use vstimd::scene::conditions::ConditionAction;
    let mut s = scene(false);
    s.config.camera_movement.as_mut().unwrap().end_blackout_ms = 2000;
    let h = nav(
        &mut s,
        None,
        Some(Track3D {
            length_cm: 20.0,
            fade_frames: 0,
        }),
    );
    s.animations.get_mut(&h).unwrap().config.conditions = vec![0];
    s.animations.get_mut(&h).unwrap().config.condition_action = ConditionAction::Hold;
    advance(&mut s);
    assert!(s.camera_blackout());

    s.config.conditions.active = 1;
    s.apply_conditions();
    advance(&mut s);
    assert!(
        !s.camera_blackout(),
        "a held animation in another condition must not hide the current scene"
    );
    assert_eq!(s.view_fade_3d(), 0.0);

    s.config.conditions.active = 0;
    s.apply_conditions();
    assert!(s.camera_blackout());
}
