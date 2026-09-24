//! Gaussian splat stimuli over the protobuf command surface.
//!
//! `handle_request` on a bare `SceneState` — no GPU, no ZMQ. The file is only
//! probed here; loading and drawing are the render thread's.

use std::path::PathBuf;

use vstimd::proto;
use vstimd::proto::request::{self, Body};
use vstimd::scene::SceneState;

fn send(scene: &mut SceneState, target: request::Target, body: Body) -> proto::Response {
    scene.handle_request(proto::Request { target: Some(target), body: Some(body) }, None)
}

fn sys() -> request::Target {
    request::Target::System(proto::SystemTarget {})
}

fn code(resp: &proto::Response) -> proto::ErrorCode {
    proto::ErrorCode::try_from(resp.code).unwrap()
}

/// A `.splat` file of `n` identical splats, unique per test.
fn splat_file(name: &str, n: usize) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vstimd-splat-it-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut record = [0u8; 32];
    for i in [12, 16, 20] {
        record[i..i + 4].copy_from_slice(&1.0f32.to_le_bytes());
    }
    record[24..28].copy_from_slice(&[200, 100, 50, 255]);
    record[28..32].copy_from_slice(&[255, 128, 128, 128]);
    let path = dir.join(name);
    std::fs::write(&path, record.repeat(n)).unwrap();
    path
}

fn create(scene: &mut SceneState, path: &str) -> proto::Response {
    send(
        scene,
        sys(),
        Body::CreateGaussianSplat3d(proto::CreateGaussianSplat3DRequest {
            identity: Some(proto::StimulusIdentity { name: "room".into() }),
            placement: Some(proto::Transform3D {
                position_cm: Some(proto::Vec3 { x: 0.0, y: 0.0, z: -100.0 }),
                rotation_deg: Some(proto::Vec3 { x: 0.0, y: 180.0, z: 0.0 }),
                scale: Some(proto::Vec3 { x: 100.0, y: 100.0, z: 100.0 }),
            }),
            params: Some(proto::GaussianSplat3DParams { path: path.into() }),
        }),
    )
}

#[test]
fn create_and_query() {
    let mut scene = SceneState::new();
    let path = splat_file("a.splat", 3);
    let resp = create(&mut scene, path.to_str().unwrap());
    assert_eq!(code(&resp), proto::ErrorCode::Ok, "{}", resp.error);
    let handle = resp.handle as u32;
    assert!(scene.has_3d());

    let resp = send(
        &mut scene,
        request::Target::Stimulus(handle),
        Body::QueryStimulus(proto::QueryStimulusRequest {}),
    );
    let Some(proto::response::Body::StimulusInfo(info)) = resp.body else { panic!("{resp:?}") };
    assert_eq!(info.stimulus_type, proto::StimulusType::GaussianSplat3d as i32);
    let Some(proto::stimulus_params::Shape::GaussianSplat3d(p)) = info.params.and_then(|p| p.shape)
    else {
        panic!("no splat params");
    };
    assert_eq!(p.path, path.to_str().unwrap());
    let Some(proto::query_stimulus_response::Placement::Transform3d(t)) = info.placement else {
        panic!("no 3-D placement");
    };
    assert_eq!(t.scale.unwrap().x, 100.0);
    assert_eq!(t.rotation_deg.unwrap().y, 180.0);
}

#[test]
fn bad_paths_are_refused_with_the_reason() {
    let mut scene = SceneState::new();
    for (path, needle) in [
        ("", "needs a path"),
        ("/nonexistent/room.splat", "no such splat file"),
        ("/etc/hostname", "expected a .ply or .splat"),
    ] {
        let resp = create(&mut scene, path);
        assert_eq!(code(&resp), proto::ErrorCode::InvalidArgument, "{path}");
        assert!(resp.error.contains(needle), "{path}: {}", resp.error);
    }
    let torn = splat_file("torn.splat", 1);
    std::fs::write(&torn, [0u8; 33]).unwrap();
    let resp = create(&mut scene, torn.to_str().unwrap());
    assert_eq!(code(&resp), proto::ErrorCode::InvalidArgument);
    assert!(scene.stimuli.is_empty(), "nothing is created on a refusal");
}

#[test]
fn transform_applies_but_material_does_not() {
    let mut scene = SceneState::new();
    let path = splat_file("b.splat", 1);
    let handle = create(&mut scene, path.to_str().unwrap()).handle as u32;

    let resp = send(
        &mut scene,
        request::Target::Stimulus(handle),
        Body::SetTransform3d(proto::SetTransform3DRequest {
            transform: Some(proto::Transform3D {
                position_cm: Some(proto::Vec3 { x: 5.0, y: 0.0, z: 0.0 }),
                ..Default::default()
            }),
        }),
    );
    assert_eq!(code(&resp), proto::ErrorCode::Ok, "{}", resp.error);
    let g = scene.stimuli[&handle].stimulus.gaussian_splat().unwrap();
    assert_eq!(g.transform.live.position_cm.0.x, 5.0);

    let resp = send(
        &mut scene,
        request::Target::Stimulus(handle),
        Body::SetMaterial3d(proto::SetMaterial3DRequest::default()),
    );
    assert_eq!(code(&resp), proto::ErrorCode::WrongStimulusType);
    assert!(resp.error.contains("GaussianSplat3D"), "{}", resp.error);
}

#[test]
fn a_splat_round_trips_through_the_scene_config_format() {
    let mut scene = SceneState::new();
    let path = splat_file("c.splat", 2);
    let handle = create(&mut scene, path.to_str().unwrap()).handle as u32;
    let stim = &scene.stimuli[&handle].stimulus;
    let json = serde_json::to_string(stim).unwrap();
    assert!(json.contains("\"GaussianSplat\""), "{json}");
    let back: vstimd::scene::Stimulus = serde_json::from_str(&json).unwrap();
    let g = back.gaussian_splat().expect("still a splat");
    assert_eq!(g.path, path.to_str().unwrap());
    assert_eq!(g.transform.live.scale.x, 100.0);
}

#[test]
fn web_snapshot_describes_splats() {
    let mut scene = SceneState::new();
    let path = splat_file("d.splat", 1);
    create(&mut scene, path.to_str().unwrap());
    let snapshot = scene.build_snapshot(None);
    assert_eq!(snapshot.stimuli[0].stimulus_type, proto::StimulusType::GaussianSplat3d as i32);
}
