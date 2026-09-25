//! Camera zones driving trigger-line animations — no GPU.

use vstimd::proto;
use vstimd::proto::request::{self, Body};
use vstimd::scene::SceneState;
use vstimd::vtl_state::{VtlEdges, VtlOutputs};

fn sys() -> Option<request::Target> {
    Some(request::Target::System(proto::SystemTarget {}))
}

fn send(scene: &mut SceneState, body: Body) -> proto::Response {
    scene.handle_request(proto::Request { target: sys(), body: Some(body) }, None)
}

fn ok(resp: &proto::Response) {
    assert_eq!(resp.code, proto::ErrorCode::Ok as i32, "{}", resp.error);
}

fn input_line(bank: u32, bit: u32) -> Option<proto::VirtualTriggerLineHandle> {
    Some(proto::VirtualTriggerLineHandle {
        handle: Some(proto::virtual_trigger_line_handle::Handle::BankBit(
            proto::VirtualTriggerLineBankBit { bank, bit },
        )),
        kind: proto::VirtualTriggerLineKind::Input as i32,
    })
}

/// One frame the way the render loops run it: zones, then animations.
fn frame(scene: &mut SceneState) -> VtlEdges {
    let mut edges = VtlEdges::default();
    scene.evaluate_camera_zones(&mut edges);
    let (mut levels, mut pulses) = ([0u64; vtl::MAX_BANKS], [0u64; vtl::MAX_BANKS]);
    scene.advance_animations(&edges, &VtlEdges::default(), &mut VtlOutputs { levels: &mut levels, pulses: &mut pulses });
    edges
}

fn zone(name: &str, bit: u32, z: [f32; 2]) -> proto::CameraZone {
    proto::CameraZone {
        name: name.into(),
        line: input_line(3, bit),
        z_min_cm: Some(z[0]),
        z_max_cm: Some(z[1]),
        ..Default::default()
    }
}

#[test]
fn a_stimulus_is_visible_while_the_camera_is_in_its_zone_every_lap() {
    let mut scene = SceneState::new();
    scene.runtime.nominal_frame_rate_hz = 60.0;
    ok(&send(&mut scene, Body::SetCameraZones(proto::SetCameraZonesRequest {
        zones: vec![zone("reward", 0, [40.0, 60.0])],
    })));

    let rect = send(&mut scene, Body::CreateRect(proto::CreateRectRequest::default())).handle as u32;
    let couple = send(&mut scene, Body::CreateAnimation(proto::CreateAnimationRequest {
        target: Some(proto::AnimationTarget {
            target: Some(proto::animation_target::Target::Stimuli(proto::AnimationStimuli { handles: vec![rect] })),
        }),
        body: Some(proto::create_animation_request::Body::CoupleVisibilityToTriggerLine(
            proto::CoupleVisibilityToTriggerLine { trigger: input_line(3, 0), polarity: proto::VtlPolarity::ActiveHigh as i32 },
        )),
        ..Default::default()
    }));
    ok(&couple);
    ok(&send(&mut scene, Body::ArmAnimation(proto::ArmAnimationRequest { handle: couple.handle as u32 })));

    // Walk down a 100 cm corridor at 60 cm/s: 1 cm per frame, wrapping.
    let nav = send(&mut scene, Body::CreateAnimation(proto::CreateAnimationRequest {
        target: Some(proto::AnimationTarget {
            target: Some(proto::animation_target::Target::Camera(proto::AnimationCamera {})),
        }),
        body: Some(proto::create_animation_request::Body::LinearNav3d(proto::LinearNav3D {
            speed_cm_per_s: 60.0,
            wrap_period_cm: 100.0,
            source: None,
            ..Default::default()
        })),
        ..Default::default()
    }));
    ok(&nav);
    ok(&send(&mut scene, Body::ArmAnimation(proto::ArmAnimationRequest { handle: nav.handle as u32 })));

    let (mut entries, mut exits, mut visible_frames) = (0, 0, 0);
    for _ in 0..300 {
        let edges = frame(&mut scene);
        entries += edges.rising[3].count_ones();
        exits += edges.falling[3].count_ones();
        let flags = scene.stimuli[&(rect)].stimulus.flags();
        if flags.anim_enabled {
            visible_frames += 1;
        }
        let inside = edges.current[3] & 1 == 1;
        assert_eq!(flags.anim_enabled, inside, "visibility follows the zone level");
    }
    assert_eq!(entries, 3, "three laps, one entry each");
    assert_eq!(exits, 3);
    assert!((55..=66).contains(&visible_frames), "about 20 of every 100 frames: {visible_frames}");

    let resp = send(&mut scene, Body::ListCameraZones(proto::ListCameraZonesRequest {}));
    let Some(proto::response::Body::CameraZoneList(list)) = resp.body else { panic!() };
    assert_eq!(list.zones[0].zone.as_ref().unwrap().name, "reward");
}

#[test]
fn zones_are_validated_and_saved() {
    let mut scene = SceneState::new();
    let mut out = zone("out", 1, [0.0, 1.0]);
    out.line.as_mut().unwrap().kind = proto::VirtualTriggerLineKind::Output as i32;
    for zones in [
        vec![out],
        vec![zone("a", 1, [5.0, 1.0])],
        vec![zone("a", 1, [0.0, 1.0]), zone("b", 1, [2.0, 3.0])],
        vec![proto::CameraZone { name: "half".into(), line: input_line(3, 2), z_min_cm: Some(1.0), ..Default::default() }],
    ] {
        let resp = send(&mut scene, Body::SetCameraZones(proto::SetCameraZonesRequest { zones }));
        assert_eq!(resp.code, proto::ErrorCode::InvalidArgument as i32, "{}", resp.error);
    }
    ok(&send(&mut scene, Body::SetCameraZones(proto::SetCameraZonesRequest {
        zones: vec![zone("a", 1, [0.0, 1.0]), zone("b", 2, [2.0, 3.0])],
    })));
    let json = serde_json::to_string(&scene.config).unwrap();
    let back: vstimd::scene::SceneConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.camera_zones.len(), 2);
    assert!(!json.contains("inside"), "runtime state is not saved");

    // A scene with no zones writes no key at all.
    let empty = serde_json::to_string(&SceneState::new().config).unwrap();
    assert!(!empty.contains("camera_zones"));
}
