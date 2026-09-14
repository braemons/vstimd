//! 3-D stimuli, camera and lighting over the protobuf command surface.
//!
//! `handle_request` on a bare `SceneState` — no GPU, no ZMQ.

use vstimd::proto;
use vstimd::proto::request::{self, Body};
use vstimd::scene::SceneState;
use vstimd::scene::stimulus::{Mesh3dGeometry, Shading3D};

fn sys() -> request::Target {
    request::Target::System(proto::SystemTarget {})
}

fn send(scene: &mut SceneState, target: request::Target, body: Body) -> proto::Response {
    scene.handle_request(
        proto::Request {
            target: Some(target),
            body: Some(body),
        },
        None,
    )
}

fn to(scene: &mut SceneState, handle: u32, body: Body) -> proto::Response {
    send(scene, request::Target::Stimulus(handle), body)
}

fn code(resp: &proto::Response) -> proto::ErrorCode {
    proto::ErrorCode::try_from(resp.code).unwrap()
}

fn ok(resp: &proto::Response) {
    assert_eq!(
        code(resp),
        proto::ErrorCode::Ok,
        "unexpected error: {}",
        resp.error
    );
}

fn v3(x: f32, y: f32, z: f32) -> Option<proto::Vec3> {
    Some(proto::Vec3 { x, y, z })
}

fn create_sphere(scene: &mut SceneState, params: proto::Sphere3DParams) -> u32 {
    let resp = send(
        scene,
        sys(),
        Body::CreateSphere3d(proto::CreateSphere3DRequest {
            identity: Some(proto::StimulusIdentity {
                name: "ball".into(),
            }),
            placement: Some(proto::Transform3D {
                position_cm: v3(1.0, 2.0, -50.0),
                rotation_deg: v3(30.0, -15.0, 0.0),
                scale: None,
            }),
            params: Some(params),
        }),
    );
    ok(&resp);
    assert!(!resp.id.is_empty());
    resp.handle as u32
}

fn query(scene: &mut SceneState, handle: u32) -> proto::QueryStimulusResponse {
    let resp = to(
        scene,
        handle,
        Body::QueryStimulus(proto::QueryStimulusRequest {}),
    );
    ok(&resp);
    match resp.body {
        Some(proto::response::Body::StimulusInfo(info)) => info,
        other => panic!("expected StimulusInfo, got {other:?}"),
    }
}

fn mesh(scene: &SceneState, handle: u32) -> &vstimd::scene::Mesh3d {
    scene.stimuli[&handle]
        .stimulus
        .mesh3d()
        .expect("a 3-D stimulus")
}

// ── Creation ──────────────────────────────────────────────────────────────────

#[test]
fn create_sphere_places_and_sizes_it() {
    let mut scene = SceneState::new();
    let h = create_sphere(
        &mut scene,
        proto::Sphere3DParams {
            diameter_cm: 12.0,
            rings: 8,
            sectors: 24,
            ..Default::default()
        },
    );
    let m = mesh(&scene, h);
    assert_eq!(
        m.geometry.live,
        Mesh3dGeometry::Sphere {
            diameter_cm: 12.0,
            rings: 8,
            sectors: 24
        }
    );
    assert_eq!(
        m.transform.live.position_cm,
        glam::Vec3::new(1.0, 2.0, -50.0)
    );
    assert_eq!(
        m.transform.live.scale,
        glam::Vec3::ONE,
        "absent scale is (1, 1, 1)"
    );
    assert_eq!(
        m.material.live.albedo,
        vstimd::Color::WHITE,
        "absent albedo is white"
    );
    assert!(scene.has_3d());
}

#[test]
fn zero_means_default() {
    let mut scene = SceneState::new();
    let h = create_sphere(&mut scene, proto::Sphere3DParams::default());
    assert_eq!(
        mesh(&scene, h).geometry.live,
        Mesh3dGeometry::Sphere {
            diameter_cm: 10.0,
            rings: 16,
            sectors: 32
        }
    );

    let resp = send(
        &mut scene,
        sys(),
        Body::CreateCube3d(proto::CreateCube3DRequest {
            placement: Some(proto::Transform3D {
                scale: v3(2.0, 0.0, 1.0),
                ..Default::default()
            }),
            params: Some(proto::Cube3DParams {
                size_cm: v3(20.0, 0.0, 5.0),
                ..Default::default()
            }),
            ..Default::default()
        }),
    );
    ok(&resp);
    let m = mesh(&scene, resp.handle as u32);
    assert_eq!(
        m.geometry.live,
        Mesh3dGeometry::Cube {
            size_cm: [20.0, 10.0, 5.0]
        }
    );
    assert_eq!(m.transform.live.scale, glam::Vec3::new(2.0, 1.0, 1.0));

    let resp = send(
        &mut scene,
        sys(),
        Body::CreatePlane3d(proto::CreatePlane3DRequest::default()),
    );
    ok(&resp);
    assert_eq!(
        mesh(&scene, resp.handle as u32).geometry.live,
        Mesh3dGeometry::Plane {
            size_cm: [100.0, 100.0]
        }
    );
}

#[test]
fn invalid_sizes_are_refused() {
    let mut scene = SceneState::new();
    for params in [
        proto::Sphere3DParams {
            diameter_cm: -1.0,
            ..Default::default()
        },
        proto::Sphere3DParams {
            diameter_cm: f32::NAN,
            ..Default::default()
        },
        proto::Sphere3DParams {
            rings: 10_000,
            ..Default::default()
        },
    ] {
        let resp = send(
            &mut scene,
            sys(),
            Body::CreateSphere3d(proto::CreateSphere3DRequest {
                params: Some(params),
                ..Default::default()
            }),
        );
        assert_eq!(
            code(&resp),
            proto::ErrorCode::InvalidArgument,
            "{}",
            resp.error
        );
    }
    let resp = send(
        &mut scene,
        sys(),
        Body::CreateCube3d(proto::CreateCube3DRequest {
            placement: Some(proto::Transform3D {
                scale: v3(1.0, -1.0, 1.0),
                ..Default::default()
            }),
            ..Default::default()
        }),
    );
    assert_eq!(code(&resp), proto::ErrorCode::InvalidArgument);
    assert!(
        scene.stimuli.is_empty(),
        "nothing is created by a refused request"
    );
}

#[test]
fn texture_path_is_not_supported_yet() {
    let mut scene = SceneState::new();
    let resp = send(
        &mut scene,
        sys(),
        Body::CreateCube3d(proto::CreateCube3DRequest {
            params: Some(proto::Cube3DParams {
                texture_path: "wall.png".into(),
                ..Default::default()
            }),
            ..Default::default()
        }),
    );
    assert_eq!(code(&resp), proto::ErrorCode::NotSupported);
}

#[test]
fn create_is_refused_when_the_renderer_has_no_3d() {
    let mut scene = SceneState::new();
    scene.runtime.render_3d_unavailable = true;
    let resp = send(
        &mut scene,
        sys(),
        Body::CreateSphere3d(proto::CreateSphere3DRequest::default()),
    );
    assert_eq!(code(&resp), proto::ErrorCode::NotSupported);
}

#[test]
fn create_sent_to_a_stimulus_is_refused() {
    let mut scene = SceneState::new();
    let h = create_sphere(&mut scene, proto::Sphere3DParams::default());
    let resp = to(
        &mut scene,
        h,
        Body::CreateCube3d(proto::CreateCube3DRequest::default()),
    );
    assert_eq!(code(&resp), proto::ErrorCode::WrongTarget);
}

// ── Query ─────────────────────────────────────────────────────────────────────

#[test]
fn query_round_trips_type_placement_and_params() {
    let mut scene = SceneState::new();
    let h = create_sphere(
        &mut scene,
        proto::Sphere3DParams {
            diameter_cm: 7.0,
            material: Some(proto::Material3D {
                albedo: Some(proto::Color {
                    r: 0.6,
                    g: 0.6,
                    b: 0.6,
                    a: 1.0,
                }),
                emissive: v3(0.1, 0.0, 0.0),
                shading: proto::Shading::Phong as i32,
            }),
            ..Default::default()
        },
    );
    let info = query(&mut scene, h);
    assert_eq!(info.stimulus_type, proto::StimulusType::Sphere3d as i32);
    assert_eq!(info.name, "ball");
    let Some(proto::query_stimulus_response::Placement::Transform3d(t)) = info.placement else {
        panic!("expected a 3-D placement");
    };
    let rot = t.rotation_deg.unwrap();
    assert!((rot.x - 30.0).abs() < 1e-4 && (rot.y + 15.0).abs() < 1e-4 && rot.z.abs() < 1e-4);
    let Some(proto::stimulus_params::Shape::Sphere3d(p)) = info.params.and_then(|p| p.shape) else {
        panic!("expected Sphere3DParams");
    };
    assert_eq!((p.diameter_cm, p.rings, p.sectors), (7.0, 16, 32));
    let material = p.material.unwrap();
    assert_eq!(material.shading, proto::Shading::Phong as i32);
    assert_eq!(material.albedo.unwrap().r, 0.6);

    // And the list reports the user-facing type.
    let resp = send(
        &mut scene,
        sys(),
        Body::ListStimuli(proto::ListStimuliRequest {}),
    );
    let Some(proto::response::Body::StimulusList(list)) = resp.body else {
        panic!()
    };
    assert_eq!(
        list.entries[0].stimulus_type,
        proto::StimulusType::Sphere3d as i32
    );
}

#[test]
fn web_snapshot_describes_3d_stimuli() {
    let mut scene = SceneState::new();
    create_sphere(&mut scene, proto::Sphere3DParams::default());
    let snapshot = scene.build_snapshot(None);
    assert_eq!(
        snapshot.stimuli[0].stimulus_type,
        proto::StimulusType::Sphere3d as i32
    );
}

// ── Mutation ──────────────────────────────────────────────────────────────────

#[test]
fn set_transform_and_material() {
    let mut scene = SceneState::new();
    let h = create_sphere(&mut scene, proto::Sphere3DParams::default());
    ok(&to(
        &mut scene,
        h,
        Body::SetTransform3d(proto::SetTransform3DRequest {
            transform: Some(proto::Transform3D {
                position_cm: v3(0.0, 0.0, -80.0),
                rotation_deg: v3(90.0, 0.0, 0.0),
                scale: v3(1.0, 3.0, 1.0),
            }),
        }),
    ));
    ok(&to(
        &mut scene,
        h,
        Body::SetMaterial3d(proto::SetMaterial3DRequest {
            material: Some(proto::Material3D {
                shading: proto::Shading::Phong as i32,
                ..Default::default()
            }),
        }),
    ));
    let m = mesh(&scene, h);
    assert_eq!(m.transform.live.position_cm.z, -80.0);
    assert_eq!(m.transform.live.scale, glam::Vec3::new(1.0, 3.0, 1.0));
    assert_eq!(m.material.live.shading, Shading3D::Phong);
}

#[test]
fn size_setters_check_the_type() {
    let mut scene = SceneState::new();
    let sphere = create_sphere(
        &mut scene,
        proto::Sphere3DParams {
            rings: 8,
            ..Default::default()
        },
    );

    ok(&to(
        &mut scene,
        sphere,
        Body::SetSphere3dDiameter(proto::SetSphere3DDiameterRequest { diameter_cm: 25.0 }),
    ));
    assert_eq!(
        mesh(&scene, sphere).geometry.live,
        Mesh3dGeometry::Sphere {
            diameter_cm: 25.0,
            rings: 8,
            sectors: 32
        },
        "a diameter change keeps the tessellation"
    );

    let resp = to(
        &mut scene,
        sphere,
        Body::SetCube3dSize(proto::SetCube3DSizeRequest {
            size_cm: v3(1.0, 1.0, 1.0),
        }),
    );
    assert_eq!(code(&resp), proto::ErrorCode::WrongStimulusType);
    assert!(
        resp.error.contains("Cube3D") && resp.error.contains("Sphere3D"),
        "{}",
        resp.error
    );
}

#[test]
fn two_d_commands_refuse_3d_stimuli_and_vice_versa() {
    let mut scene = SceneState::new();
    let sphere = create_sphere(&mut scene, proto::Sphere3DParams::default());
    let resp = to(
        &mut scene,
        sphere,
        Body::SetPosition(proto::SetPositionRequest {
            x_px: 1.0,
            y_px: 1.0,
        }),
    );
    assert_eq!(code(&resp), proto::ErrorCode::WrongStimulusType);

    let rect = send(
        &mut scene,
        sys(),
        Body::CreateRect(proto::CreateRectRequest::default()),
    )
    .handle as u32;
    let resp = to(
        &mut scene,
        rect,
        Body::SetTransform3d(proto::SetTransform3DRequest::default()),
    );
    assert_eq!(code(&resp), proto::ErrorCode::WrongStimulusType);
}

#[test]
fn deferred_transform_lands_on_flip() {
    let mut scene = SceneState::new();
    let h = create_sphere(&mut scene, proto::Sphere3DParams::default());
    let deferred = |active| {
        Body::SetDeferredMode(proto::SetDeferredModeRequest {
            active,
            cancel: false,
        })
    };

    ok(&send(&mut scene, sys(), deferred(true)));
    ok(&to(
        &mut scene,
        h,
        Body::SetTransform3d(proto::SetTransform3DRequest {
            transform: Some(proto::Transform3D {
                position_cm: v3(9.0, 0.0, 0.0),
                ..Default::default()
            }),
        }),
    ));
    ok(&send(
        &mut scene,
        sys(),
        Body::SetCamera(proto::SetCameraRequest {
            camera: Some(proto::Camera3D {
                yaw_deg: 45.0,
                ..Default::default()
            }),
        }),
    ));
    assert_eq!(
        mesh(&scene, h).transform.live.position_cm.x,
        1.0,
        "staged, not live"
    );
    assert_eq!(scene.camera.live.yaw_deg, 0.0, "staged, not live");

    ok(&send(&mut scene, sys(), deferred(false)));
    scene.apply_flip();
    assert_eq!(mesh(&scene, h).transform.live.position_cm.x, 9.0);
    assert_eq!(
        scene.camera.live.yaw_deg, 45.0,
        "camera flips with the stimuli"
    );
}

// ── Camera and lighting ───────────────────────────────────────────────────────

#[test]
fn set_and_query_camera() {
    let mut scene = SceneState::new();
    ok(&send(
        &mut scene,
        sys(),
        Body::SetCamera(proto::SetCameraRequest {
            camera: Some(proto::Camera3D {
                position_cm: v3(0.0, 10.0, 30.0),
                yaw_deg: -20.0,
                fov_y_deg: 90.0,
                ..Default::default()
            }),
        }),
    ));
    let resp = send(
        &mut scene,
        sys(),
        Body::QueryCamera(proto::QueryCameraRequest {}),
    );
    let Some(proto::response::Body::Camera(c)) = resp.body else {
        panic!("{resp:?}")
    };
    assert_eq!(c.position_cm.unwrap().y, 10.0);
    assert_eq!((c.yaw_deg, c.fov_y_deg), (-20.0, 90.0));
    assert_eq!(
        (c.near_cm, c.far_cm),
        (1.0, 50_000.0),
        "zero near/far take the defaults"
    );
}

#[test]
fn invalid_camera_is_refused() {
    let mut scene = SceneState::new();
    for camera in [
        None,
        Some(proto::Camera3D {
            fov_y_deg: 180.0,
            ..Default::default()
        }),
        Some(proto::Camera3D {
            near_cm: 10.0,
            far_cm: 5.0,
            ..Default::default()
        }),
        Some(proto::Camera3D {
            near_cm: -1.0,
            ..Default::default()
        }),
    ] {
        let resp = send(
            &mut scene,
            sys(),
            Body::SetCamera(proto::SetCameraRequest { camera }),
        );
        assert_eq!(
            code(&resp),
            proto::ErrorCode::InvalidArgument,
            "{}",
            resp.error
        );
    }
}

#[test]
fn set_and_query_lighting() {
    let mut scene = SceneState::new();
    ok(&send(
        &mut scene,
        sys(),
        Body::SetLighting(proto::SetLightingRequest {
            lighting: Some(proto::Lighting3D {
                ambient_color: v3(0.2, 0.2, 0.2),
                sun_direction: v3(-1.0, 0.0, 0.0),
                sun_color: v3(1.0, 0.9, 0.8),
            }),
        }),
    ));
    let resp = send(
        &mut scene,
        sys(),
        Body::QueryLighting(proto::QueryLightingRequest {}),
    );
    let Some(proto::response::Body::Lighting(l)) = resp.body else {
        panic!("{resp:?}")
    };
    assert_eq!(l.sun_direction.unwrap().x, -1.0);
    assert_eq!(l.sun_color.unwrap().z, 0.8);
}

#[test]
fn zero_sun_direction_is_refused() {
    let mut scene = SceneState::new();
    let resp = send(
        &mut scene,
        sys(),
        Body::SetLighting(proto::SetLightingRequest {
            lighting: Some(proto::Lighting3D {
                sun_direction: v3(0.0, 0.0, 0.0),
                ..Default::default()
            }),
        }),
    );
    assert_eq!(code(&resp), proto::ErrorCode::InvalidArgument);
}
