/// Integration tests for protobuf command dispatch.
///
/// These tests call `handle_request` directly on a `SceneState` — no ZMQ, no
/// GPU required — so they can run in any environment.
use prost::Message;
use vstimd::proto;
use vstimd::proto::request;
use vstimd::scene::SceneState;
use vstimd::Color;

// ── helpers ───────────────────────────────────────────────────────────────────

fn sys() -> request::Target {
    request::Target::System(proto::SystemTarget {})
}

fn stim(handle: u32) -> request::Target {
    request::Target::Stimulus(handle)
}

fn create_rect_req(target: request::Target, cmd: proto::CreateRectRequest) -> proto::Request {
    proto::Request {
        target: Some(target),
        body: Some(request::Body::CreateRect(cmd)),
    }
}

fn set_enabled_req(handle: u32, enabled: bool) -> proto::Request {
    proto::Request {
        target: Some(stim(handle)),
        body: Some(request::Body::SetEnabled(proto::SetEnabledRequest { enabled })),
    }
}

fn delete_req(handle: u32) -> proto::Request {
    proto::Request {
        target: Some(stim(handle)),
        body: Some(request::Body::Delete(proto::DeleteRequest {})),
    }
}

fn set_deferred_mode_req(active: bool, cancel: bool) -> proto::Request {
    proto::Request {
        target: Some(sys()),
        body: Some(request::Body::SetDeferredMode(proto::SetDeferredModeRequest { active, cancel })),
    }
}

/// Proto animation target for a list of stimulus handles.
fn anim_target(handles: Vec<u32>) -> proto::AnimationTarget {
    proto::AnimationTarget {
        target: Some(proto::animation_target::Target::Stimuli(
            proto::AnimationStimuli { handles },
        )),
    }
}

fn is_ok(resp: &proto::Response) -> bool {
    resp.code == proto::ErrorCode::Ok as i32
}


/// The 2-D placement out of a query response, or panic — every stimulus in
/// these tests is 2-D.
fn placement_2d(info: &proto::QueryStimulusResponse) -> proto::Transform2D {
    match info.placement.clone() {
        Some(proto::query_stimulus_response::Placement::Transform2d(t)) => t,
        None => panic!("query response carried no placement"),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_create_rect() {
    let mut scene = SceneState::new();
    let resp = scene.handle_request(create_rect_req(
        sys(),
        proto::CreateRectRequest {
            placement: Some(proto::Transform2D {
                pos_px: Some(proto::Vec2 { x: 10.0, y: -20.0 }),
                rotation_deg: 0.0,
            }),
            params: Some(proto::RectParams { width_px: 200.0, height_px: 100.0, ..Default::default() }),
            ..Default::default()
        },
    ), None);
    assert!(is_ok(&resp), "unexpected error: {}", resp.error);
    let new_handle = resp.handle as u32;
    assert!(new_handle > 0, "handle should be positive");
    assert!(scene.stimuli.contains_key(&new_handle), "stimulus should exist in scene");
}

#[test]
fn test_create_rect_with_fill() {
    let mut scene = SceneState::new();
    let fill = proto::Color { r: 0.5, g: 0.25, b: 0.75, a: 1.0 };
    let resp = scene.handle_request(create_rect_req(
        sys(),
        proto::CreateRectRequest {
            placement: Some(proto::Transform2D { pos_px: None, rotation_deg: 0.0 }),
            params: Some(proto::RectParams {
                width_px: 0.0,
                height_px: 0.0,
                appearance: Some(proto::ShapeAppearance { fill_color: Some(fill), ..Default::default() })
            }),
            ..Default::default()
        },
    ), None);
    assert!(is_ok(&resp));
    let h = resp.handle as u32;
    let entry = scene.stimuli.get_mut(&h).unwrap();
    let appearance = entry.stimulus.shape_appearance().expect("expected shape stimulus");
    assert_eq!(appearance.live.fill_color.r, fill.r);
    assert_eq!(appearance.live.fill_color.g, fill.g);
    assert_eq!(appearance.live.fill_color.b, fill.b);
    assert_eq!(appearance.live.fill_color.a, fill.a);
}

#[test]
fn test_create_rect_defaults() {
    let mut scene = SceneState::new();
    let default_fill = scene.default_fill;
    let resp = scene.handle_request(create_rect_req(
        sys(),
        proto::CreateRectRequest {
            placement: Some(proto::Transform2D { pos_px: None, rotation_deg: 0.0 }),
            params: Some(proto::RectParams { width_px: 0.0, height_px: 0.0, ..Default::default() }),
            ..Default::default()
        },
    ), None);
    assert!(is_ok(&resp));
    let h = resp.handle as u32;
    let entry = scene.stimuli.get_mut(&h).unwrap();

    assert_eq!(entry.stimulus.type_name(), "Rect");
    let r = entry.stimulus.shape().expect("expected Rect stimulus");
    // width_px=0 → server default 100
    assert_eq!(r.geometry.live.size_px(), Some([100.0, 100.0]));
    assert_eq!(r.appearance.live.fill_color, default_fill);
}

#[test]
fn test_enable_disable() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;

    assert!(scene.stimuli[&h].stimulus.is_visible());

    let resp = scene.handle_request(set_enabled_req(h, false), None);
    assert!(is_ok(&resp));
    assert_eq!(resp.handle, -1);
    assert!(!scene.stimuli[&h].stimulus.flags().enabled);

    let resp = scene.handle_request(set_enabled_req(h, true), None);
    assert!(is_ok(&resp));
    assert!(scene.stimuli[&h].stimulus.flags().enabled);
}

#[test]
fn test_delete() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;

    assert!(scene.stimuli.contains_key(&h));

    let resp = scene.handle_request(delete_req(h), None);
    assert!(is_ok(&resp));
    assert_eq!(resp.handle, -1);
    assert!(!scene.stimuli.contains_key(&h));
}

#[test]
fn test_delete_nonexistent() {
    let mut scene = SceneState::new();
    let resp = scene.handle_request(delete_req(9999), None);
    assert!(!is_ok(&resp));
    assert_eq!(resp.handle, 0);
    assert!(resp.error.contains("9999"));
}

#[test]
fn test_enable_nonexistent() {
    let mut scene = SceneState::new();
    let resp = scene.handle_request(set_enabled_req(9999, true), None);
    assert!(!is_ok(&resp));
    assert_eq!(resp.handle, 0);
    assert!(resp.error.contains("9999"));
}

#[test]
fn test_empty_body_returns_error() {
    let mut scene = SceneState::new();
    let resp = scene.handle_request(proto::Request { target: Some(sys()), body: None }, None);
    assert!(!is_ok(&resp));
    assert_eq!(resp.handle, 0);
}

#[test]
fn test_create_rect_wrong_handle() {
    let mut scene = SceneState::new();
    // CreateRect with a stimulus target should return an error.
    let resp = scene.handle_request(create_rect_req(stim(5), proto::CreateRectRequest::default()), None);
    assert!(!is_ok(&resp));
    assert_eq!(resp.code, proto::ErrorCode::WrongTarget as i32);
}

#[test]
fn test_proto_roundtrip() {
    let req = proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateRect(proto::CreateRectRequest {
                                                 placement: Some(proto::Transform2D {
                                                     pos_px: Some(proto::Vec2 { x: 1.0, y: 2.0 }),
                                                     rotation_deg: 0.0,
                                                 }),
                                                 params: Some(proto::RectParams {
                                                     width_px: 50.0,
                                                     height_px: 30.0,
                                                     appearance: Some(proto::ShapeAppearance { fill_color: Some(proto::Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }), ..Default::default() })
                                                 }),
                                                 ..Default::default()
                                             })),
    };
    let bytes = req.encode_to_vec();
    let decoded = proto::Request::decode(bytes.as_slice()).unwrap();
    assert_eq!(decoded.target, req.target);

    if let Some(request::Body::CreateRect(c)) = decoded.body {
        let params = c.params.expect("params survive the round trip");
        assert_eq!(params.width_px, 50.0);
        assert_eq!(params.appearance.unwrap().fill_color.unwrap().r, 1.0);
    } else {
        panic!("unexpected body variant");
    }
}

#[test]
fn test_create_ellipse() {
    let mut scene = SceneState::new();
    let resp = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateEllipse(proto::CreateEllipseRequest {
                                                    placement: Some(proto::Transform2D {
                                                        pos_px: Some(proto::Vec2 { x: 0.0, y: 0.0 }),
                                                        rotation_deg: 45.0,
                                                    }),
                                                    params: Some(proto::EllipseParams {
                                                        width_px: 120.0,
                                                        height_px: 60.0,
                                                        appearance: Some(proto::ShapeAppearance { fill_color: Some(proto::Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 }), ..Default::default() })
                                                    }),
                                                    ..Default::default()
                                                })),
    }, None);
    assert!(is_ok(&resp), "unexpected error: {}", resp.error);
    let h = resp.handle as u32;
    assert!(h > 0);
    let stim = &scene.stimuli[&h].stimulus;
    assert_eq!(stim.type_name(), "Ellipse");
    let e = stim.shape().expect("expected Ellipse stimulus");
    assert_eq!(e.geometry.live.size_px(), Some([120.0, 60.0]));
    assert_eq!(e.transform.live.angle_deg, 45.0);
}

#[test]
fn test_set_position() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetPosition(proto::SetPositionRequest { x_px: 42.0, y_px: -7.0 })),
    }, None);
    assert!(is_ok(&resp));
    assert_eq!(scene.stimuli[&h].stimulus.get_pos_2d(), Some([42.0, -7.0]));
}

#[test]
fn test_set_fill_color() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetFillColor(proto::SetFillColorRequest {
            color: Some(proto::Color { r: 1.0, g: 0.0, b: 0.5, a: 0.8 }),
        })),
    }, None);
    assert!(is_ok(&resp));
    let app = scene.stimuli.get(&h).unwrap().stimulus.shape_appearance().expect("expected shape");
    assert_eq!(app.live.fill_color, Color::new(1.0, 0.0, 0.5, 0.8));
}

#[test]
fn test_immediate_mode_composes_mutations_and_marks_dirty() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;
    scene.stimuli.get_mut(&h).unwrap().stimulus.flags_mut().dirty = false;

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetPosition(proto::SetPositionRequest { x_px: 15.0, y_px: 25.0 })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetRotation(proto::SetRotationRequest { rotation_deg: 30.0 })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetFillColor(proto::SetFillColorRequest {
            color: Some(proto::Color { r: 0.1, g: 0.2, b: 0.3, a: 0.4 }),
        })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetAlpha(proto::SetAlphaRequest { opacity: 0.9 })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetDrawMode(proto::SetDrawModeRequest {
            mode: proto::ShapeDrawMode::Outlined as i32,
        })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetOutlineColor(proto::SetOutlineColorRequest {
            color: Some(proto::Color { r: 0.8, g: 0.7, b: 0.6, a: 0.5 }),
        })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetOutlineWidth(proto::SetOutlineWidthRequest { line_width_px: 7.0 })),
    }, None);
    assert!(is_ok(&resp));

    let entry = scene.stimuli.get(&h).unwrap();
    let stim = &entry.stimulus;
    let t = stim.transform2d().expect("expected 2-D stimulus");
    assert_eq!(t.live.pos_px, [15.0, 25.0]);
    assert_eq!(t.live.angle_deg, 30.0);

    let app = stim.shape_appearance().expect("expected shape");
    // SetAlpha writes the shared opacity and leaves the fill's own alpha alone.
    assert_eq!(app.live.fill_color, Color::new(0.1, 0.2, 0.3, 0.4));
    assert_eq!(stim.opacity().live, 0.9);
    assert!(app.live.draw_mode == vstimd::scene::DrawMode::Stroke);
    assert_eq!(app.live.outline_color, Color::new(0.8, 0.7, 0.6, 0.5));
    assert_eq!(app.live.stroke_width_px, 7.0);
    assert!(stim.flags().dirty);
}

#[test]
fn test_ending_deferred_mode_reports_the_frame_the_flip_lands_on() {
    let mut scene = SceneState::new();
    let _ = scene.handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None);
    scene.runtime.frame_count = 41;

    let resp = scene.handle_request(set_deferred_mode_req(true, false), None);
    let Some(proto::response::Body::DeferredMode(begun)) = resp.body else {
        panic!("SetDeferredMode did not report what it did");
    };
    assert!(begun.deferred && !begun.was_deferred && !begun.flip_scheduled);

    let resp = scene.handle_request(set_deferred_mode_req(false, false), None);
    let Some(proto::response::Body::DeferredMode(ended)) = resp.body else {
        panic!("SetDeferredMode did not report what it did");
    };
    assert!(ended.was_deferred, "it was on before the call");
    assert!(!ended.deferred, "and off after it");
    assert!(ended.flip_scheduled);
    // The render thread flips, then counts the frame: the staged state is what
    // frame 42 is drawn from.
    assert_eq!(ended.flip_frame, 42);

    // Cancelling discards instead, so there is nothing to wait for.
    let _ = scene.handle_request(set_deferred_mode_req(true, false), None);
    scene.runtime.pending_flip = false;
    let resp = scene.handle_request(set_deferred_mode_req(false, true), None);
    let Some(proto::response::Body::DeferredMode(cancelled)) = resp.body else {
        panic!("SetDeferredMode did not report what it did");
    };
    assert!(cancelled.was_deferred && !cancelled.deferred);
    assert!(!cancelled.flip_scheduled);
    assert_eq!(cancelled.flip_frame, 0);
}

#[test]
fn test_ending_deferred_mode_that_never_began_leaves_the_scene_alone() {
    // A client that switches deferred mode off defensively — "whatever the last
    // session left, I want it off" — used to schedule a flip of copies that
    // nothing had staged. Every write since would be reverted on the next frame,
    // which reads as a mutation that silently did not take.
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;

    let resp = scene.handle_request(set_deferred_mode_req(false, false), None);
    assert!(is_ok(&resp));
    assert!(!scene.runtime.pending_flip, "nothing was staged, so nothing to flip");

    // And the reply says as much, rather than a bare ack the caller cannot read.
    let Some(proto::response::Body::DeferredMode(status)) = resp.body else {
        panic!("SetDeferredMode did not report what it did");
    };
    assert!(!status.was_deferred);
    assert!(!status.deferred);
    assert!(!status.flip_scheduled);
    assert_eq!(status.flip_frame, 0, "no flip, so no frame to wait for");

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetPosition(proto::SetPositionRequest { x_px: 15.0, y_px: 25.0 })),
    }, None);
    assert!(is_ok(&resp));

    // What the render thread does at the top of every frame.
    if scene.runtime.pending_flip {
        scene.apply_flip();
    }
    let t = scene.stimuli.get(&h).unwrap().stimulus.transform2d().expect("2-D");
    assert_eq!(t.live.pos_px, [15.0, 25.0]);
}

#[test]
fn test_deferred_mode_stages_composed_mutations_until_flip() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;

    let stim_obj = &mut scene.stimuli.get_mut(&h).unwrap().stimulus;
    stim_obj.transform2d_mut().expect("expected 2-D stimulus").live =
        vstimd::scene::Transform2D { pos_px: [1.0, 2.0], angle_deg: 3.0 };
    {
        let app = stim_obj.shape_appearance_mut().expect("expected shape");
        app.live.fill_color = Color::new(0.11, 0.12, 0.13, 0.14);
        app.live.outline_color = Color::new(0.21, 0.22, 0.23, 0.24);
        app.live.stroke_width_px = 2.5;
        app.live.draw_mode = vstimd::scene::DrawMode::FillAndStroke;
    }
    stim_obj.flags_mut().dirty = false;

    let resp = scene.handle_request(set_deferred_mode_req(true, false), None);
    assert!(is_ok(&resp));
    assert!(scene.runtime.deferred_mode);

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetPosition(proto::SetPositionRequest { x_px: 15.0, y_px: 25.0 })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetRotation(proto::SetRotationRequest { rotation_deg: 30.0 })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetFillColor(proto::SetFillColorRequest {
            color: Some(proto::Color { r: 0.1, g: 0.2, b: 0.3, a: 0.4 }),
        })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetAlpha(proto::SetAlphaRequest { opacity: 0.9 })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetDrawMode(proto::SetDrawModeRequest {
            mode: proto::ShapeDrawMode::Outlined as i32,
        })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetOutlineColor(proto::SetOutlineColorRequest {
            color: Some(proto::Color { r: 0.8, g: 0.7, b: 0.6, a: 0.5 }),
        })),
    }, None);
    assert!(is_ok(&resp));
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetOutlineWidth(proto::SetOutlineWidthRequest { line_width_px: 7.0 })),
    }, None);
    assert!(is_ok(&resp));

    let entry = scene.stimuli.get(&h).unwrap();
    let stim = &entry.stimulus;
    let t = stim.transform2d().expect("expected 2-D stimulus");
    assert_eq!(t.live.pos_px, [1.0, 2.0]);
    assert_eq!(t.live.angle_deg, 3.0);
    assert_eq!(t.copy.pos_px, [15.0, 25.0]);
    assert_eq!(t.copy.angle_deg, 30.0);

    let app = stim.shape_appearance().expect("expected shape");
    assert_eq!(app.live.fill_color, Color::new(0.11, 0.12, 0.13, 0.14));
    assert_eq!(app.live.outline_color, Color::new(0.21, 0.22, 0.23, 0.24));
    assert_eq!(app.live.stroke_width_px, 2.5);
    assert!(app.live.draw_mode == vstimd::scene::DrawMode::FillAndStroke);
    assert_eq!(app.copy.fill_color, Color::new(0.1, 0.2, 0.3, 0.4));
    assert_eq!(stim.opacity().copy, 0.9);
    assert_eq!(stim.opacity().live, 1.0, "opacity is staged, not live, in deferred mode");
    assert_eq!(app.copy.outline_color, Color::new(0.8, 0.7, 0.6, 0.5));
    assert_eq!(app.copy.stroke_width_px, 7.0);
    assert!(app.copy.draw_mode == vstimd::scene::DrawMode::Stroke);
    assert!(!stim.flags().dirty);

    let resp = scene.handle_request(set_deferred_mode_req(false, false), None);
    assert!(is_ok(&resp));
    assert!(!scene.runtime.deferred_mode);
    assert!(scene.runtime.pending_flip);

    scene.apply_flip();
    assert!(!scene.runtime.pending_flip);

    let entry = scene.stimuli.get(&h).unwrap();
    let stim = &entry.stimulus;
    let t = stim.transform2d().expect("expected 2-D stimulus");
    assert_eq!(t.live.pos_px, [15.0, 25.0]);
    assert_eq!(t.live.angle_deg, 30.0);
    let app = stim.shape_appearance().expect("expected shape");
    assert_eq!(app.live.fill_color, Color::new(0.1, 0.2, 0.3, 0.4));
    assert_eq!(app.live.outline_color, Color::new(0.8, 0.7, 0.6, 0.5));
    assert_eq!(app.live.stroke_width_px, 7.0);
    assert!(app.live.draw_mode == vstimd::scene::DrawMode::Stroke);
    assert_eq!(stim.opacity().live, 0.9, "the staged opacity flipped with the rest");
    assert!(stim.flags().dirty);
}

#[test]
fn test_set_rect_size() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetRectSize(proto::SetRectSizeRequest {
            width_px: 80.0,
            height_px: 40.0,
        })),
    }, None);
    assert!(is_ok(&resp));
    let r = scene.stimuli[&h].stimulus.shape().expect("expected Rect");
    assert_eq!(r.geometry.live.size_px(), Some([80.0, 40.0]));
}

#[test]
fn test_set_rect_size_wrong_type() {
    let mut scene = SceneState::new();
    let h = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateCircle(proto::CreateCircleRequest::default())),
    }, None).handle as u32;
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetRectSize(proto::SetRectSizeRequest { width_px: 50.0, height_px: 50.0 })),
    }, None);
    assert!(!is_ok(&resp));
    assert_eq!(resp.code, proto::ErrorCode::WrongStimulusType as i32);
}

#[test]
fn test_query_stimulus() {
    let mut scene = SceneState::new();
    let h = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateRect(proto::CreateRectRequest {
                                                 placement: Some(proto::Transform2D {
                                                     pos_px: Some(proto::Vec2 { x: 5.0, y: 10.0 }),
                                                     rotation_deg: 0.0,
                                                 }),
                                                 params: Some(proto::RectParams {
                                                     width_px: 200.0,
                                                     height_px: 100.0,
                                                     appearance: Some(proto::ShapeAppearance { fill_color: Some(proto::Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }), ..Default::default() })
                                                 }),
                                                 ..Default::default()
                                             })),
    }, None).handle as u32;

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::QueryStimulus(proto::QueryStimulusRequest {})),
    }, None);
    assert!(is_ok(&resp), "query error: {}", resp.error);

    if let Some(proto::response::Body::StimulusInfo(info)) = resp.body {
        assert_eq!(info.stimulus_type, proto::StimulusType::Rect as i32);
        assert!(info.enabled);
        let pos_px = placement_2d(&info).pos_px.unwrap();
        assert_eq!(pos_px.x, 5.0);
        assert_eq!(pos_px.y, 10.0);
        if let Some(proto::stimulus_params::Shape::Rect(rp)) = info.params.unwrap().shape {
            assert_eq!(rp.width_px, 200.0);
            assert_eq!(rp.height_px, 100.0);
            // Appearance is shape state, reported with the shape's own params.
            assert_eq!(rp.appearance.unwrap().fill_color.unwrap().r, 1.0);
        } else {
            panic!("expected Rect params");
        }
    } else {
        panic!("expected StimulusInfo in response body");
    }
}

#[test]
fn test_list_stimuli() {
    let mut scene = SceneState::new();
    let h1 = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;
    let h2 = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;

    let resp = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::ListStimuli(proto::ListStimuliRequest {})),
    }, None);
    assert!(is_ok(&resp));

    if let Some(proto::response::Body::StimulusList(list)) = resp.body {
        assert_eq!(list.entries.len(), 2);
        let handles: Vec<u32> = list.entries.iter().map(|e| e.handle).collect();
        assert!(handles.contains(&h1));
        assert!(handles.contains(&h2));
    } else {
        panic!("expected StimulusList in response body");
    }
}

#[test]
fn test_query_server_info() {
    let mut scene = SceneState::new();
    scene.runtime.screen_size = Some((1920, 1080));
    let resp = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::QueryServerInfo(proto::QueryServerInfoRequest {})),
    }, None);
    assert!(is_ok(&resp), "error: {}", resp.error);
    assert!(matches!(resp.body, Some(proto::response::Body::ServerInfo(_))));
}

#[test]
fn test_clear_stimuli() {
    let mut scene = SceneState::new();
    scene.handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None);
    scene.handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None);
    assert_eq!(scene.stimuli.len(), 2);

    let resp = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::ClearStimuli(proto::ClearStimuliRequest {})),
    }, None);
    assert!(is_ok(&resp));
    assert_eq!(scene.stimuli.len(), 0);
}

#[test]
fn test_create_with_name_and_query_returns_name_and_uuid() {
    let mut scene = SceneState::new();
    let resp = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateRect(proto::CreateRectRequest {
                                                 identity: Some(proto::StimulusIdentity { name: "fix_cross".into() }),
                                                 ..Default::default()
                                             })),
    }, None);
    assert!(is_ok(&resp));
    let h = resp.handle as u32;
    assert!(!resp.id.is_empty(), "create response should contain UUID");

    let qresp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::QueryStimulus(proto::QueryStimulusRequest {})),
    }, None);
    assert!(is_ok(&qresp));
    if let Some(proto::response::Body::StimulusInfo(info)) = qresp.body {
        assert_eq!(info.name, "fix_cross");
        assert_eq!(info.id, resp.id);
    } else {
        panic!("expected StimulusInfo");
    }
}

#[test]
fn test_set_name() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetName(proto::SetNameRequest { name: "new_name".into() })),
    }, None);
    assert!(is_ok(&resp));

    let qresp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::QueryStimulus(proto::QueryStimulusRequest {})),
    }, None);
    assert!(is_ok(&qresp));
    if let Some(proto::response::Body::StimulusInfo(info)) = qresp.body {
        assert_eq!(info.name, "new_name");
    } else {
        panic!("expected StimulusInfo");
    }
}

#[test]
fn test_create_text() {
    let mut scene = SceneState::new();
    let resp = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateText(proto::CreateTextRequest {
                                                 placement: Some(proto::Transform2D {
                                                     pos_px: Some(proto::Vec2 { x: 10.0, y: -20.0 }),
                                                     rotation_deg: 0.0,
                                                 }),
                                                 params: Some(proto::TextParams {
                                                     text: "hello".into(),
                                                     font: "Open Sans".into(),
                                                     letter_height_px: 32.0,
                                                     box_size_px: Some(proto::Vec2 { x: 400.0, y: 80.0 }),
                                                     anchor: "center".into(),
                                                     text_color: Some(proto::Color { r: 1.0, g: 1.0, b: 0.0, a: 1.0 }),
                                                     ..Default::default()
                                                 }),
                                                 ..Default::default()
                                             })),
    }, None);
    assert!(is_ok(&resp), "unexpected error: {}", resp.error);
    let h = resp.handle as u32;
    assert!(h > 0);
    assert!(!resp.id.is_empty());

    let t = scene.stimuli[&h].stimulus.text().expect("expected Text stimulus");
    assert_eq!(t.text_live, "hello");
    assert_eq!(t.font_family, "Open Sans");
    assert_eq!(t.letter_height_px, 32.0);
    assert_eq!(t.box_size_px.live, [400.0, 80.0]);
    assert_eq!(t.transform.live.pos_px, [10.0, -20.0]);
    assert_eq!(t.params.live.color, Color::new(1.0, 1.0, 0.0, 1.0));
    assert_eq!(t.params.live.fill_color.a, 0.0); // transparent by default
}

#[test]
fn test_create_text_defaults() {
    let mut scene = SceneState::new();
    let resp = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateText(proto::CreateTextRequest {
                                                 params: Some(proto::TextParams {
                                                     text: "test".into(),
                                                     ..Default::default()
                                                 }),
                                                 ..Default::default()
                                             })),
    }, None);
    assert!(is_ok(&resp), "unexpected error: {}", resp.error);
    let h = resp.handle as u32;
    let t = scene.stimuli[&h].stimulus.text().expect("expected Text stimulus");
    assert_eq!(t.box_size_px.live, [200.0, 100.0]);
    assert_eq!(t.letter_height_px, 32.0);
    assert_eq!(t.params.live.color, Color::WHITE); // white default
}

#[test]
fn test_set_text() {
    let mut scene = SceneState::new();
    let h = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateText(proto::CreateTextRequest {
                                                 params: Some(proto::TextParams {
                                                     text: "before".into(),
                                                     ..Default::default()
                                                 }),
                                                 ..Default::default()
                                             })),
    }, None).handle as u32;

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetText(proto::SetTextRequest { text: "after".into() })),
    }, None);
    assert!(is_ok(&resp));

    let stim = &scene.stimuli[&h].stimulus;
    let t = stim.text().expect("expected Text stimulus");
    assert_eq!(t.text_live, "after");
    assert_eq!(t.text_copy, "after");
    // `dirty` is written by the command layer now, not by `Text::set_text`.
    assert!(stim.flags().dirty);
}

#[test]
fn test_set_text_color() {
    let mut scene = SceneState::new();
    let h = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateText(proto::CreateTextRequest {
                                                 params: Some(proto::TextParams {
                                                     text: "hi".into(),
                                                     ..Default::default()
                                                 }),
                                                 ..Default::default()
                                             })),
    }, None).handle as u32;

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetTextColor(proto::SetTextColorRequest {
            color: Some(proto::Color { r: 0.0, g: 1.0, b: 0.5, a: 0.8 }),
        })),
    }, None);
    assert!(is_ok(&resp));

    let t = scene.stimuli[&h].stimulus.text().expect("expected Text stimulus");
    assert_eq!(t.params.live.color, Color::new(0.0, 1.0, 0.5, 0.8));
}

#[test]
fn test_set_text_wrong_type() {
    let mut scene = SceneState::new();
    let h = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateRect(proto::CreateRectRequest::default())),
    }, None).handle as u32;

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetText(proto::SetTextRequest { text: "oops".into() })),
    }, None);
    assert!(!is_ok(&resp));
    assert_eq!(resp.code, proto::ErrorCode::WrongStimulusType as i32);
}

#[test]
fn test_set_text_color_missing_color() {
    let mut scene = SceneState::new();
    let h = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateText(proto::CreateTextRequest {
                                                 params: Some(proto::TextParams {
                                                     text: "hi".into(),
                                                     ..Default::default()
                                                 }),
                                                 ..Default::default()
                                             })),
    }, None).handle as u32;

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetTextColor(proto::SetTextColorRequest { color: None })),
    }, None);
    assert!(!is_ok(&resp));
    assert_eq!(resp.code, proto::ErrorCode::InvalidArgument as i32);
}

#[test]
fn test_query_text_stimulus() {
    let mut scene = SceneState::new();
    let h = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateText(proto::CreateTextRequest {
                                                 placement: Some(proto::Transform2D {
                                                     pos_px: Some(proto::Vec2 { x: 5.0, y: -10.0 }),
                                                     rotation_deg: 0.0,
                                                 }),
                                                 params: Some(proto::TextParams {
                                                     text: "hello".into(),
                                                     font: "Cairo".into(),
                                                     letter_height_px: 24.0,
                                                     box_size_px: Some(proto::Vec2 { x: 300.0, y: 60.0 }),
                                                     anchor: "top-left".into(),
                                                     text_color: Some(proto::Color { r: 0.5, g: 0.5, b: 1.0, a: 1.0 }),
                                                     ..Default::default()
                                                 }),
                                                 ..Default::default()
                                             })),
    }, None).handle as u32;

    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::QueryStimulus(proto::QueryStimulusRequest {})),
    }, None);
    assert!(is_ok(&resp), "query error: {}", resp.error);

    if let Some(proto::response::Body::StimulusInfo(info)) = resp.body {
        assert_eq!(info.stimulus_type, proto::StimulusType::Text as i32);
        assert!(info.enabled);
        let pos_px = placement_2d(&info).pos_px.unwrap();
        assert_eq!((pos_px.x, pos_px.y), (5.0, -10.0));
        if let Some(proto::stimulus_params::Shape::Text(tp)) = info.params.unwrap().shape {
            assert_eq!(tp.text, "hello");
            assert_eq!(tp.font, "Cairo");
            assert_eq!(tp.letter_height_px, 24.0);
            assert_eq!(tp.anchor, "top-left");
            let size = tp.box_size_px.unwrap();
            assert_eq!((size.x, size.y), (300.0, 60.0));
        } else {
            panic!("expected Text params");
        }
    } else {
        panic!("expected StimulusInfo");
    }
}

#[test]
fn test_text_deferred_set_text_and_color() {
    let mut scene = SceneState::new();
    let h = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateText(proto::CreateTextRequest {
                                                 params: Some(proto::TextParams {
                                                     text: "initial".into(),
                                                     ..Default::default()
                                                 }),
                                                 ..Default::default()
                                             })),
    }, None).handle as u32;

    // Enter deferred mode
    scene.handle_request(set_deferred_mode_req(true, false), None);

    scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetText(proto::SetTextRequest { text: "deferred".into() })),
    }, None);
    scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::SetTextColor(proto::SetTextColorRequest {
            color: Some(proto::Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }),
        })),
    }, None);

    // Live values unchanged before flip
    let t = scene.stimuli[&h].stimulus.text().expect("expected Text stimulus");
    assert_eq!(t.text_live, "initial");
    assert_eq!(t.text_copy, "deferred");
    assert_eq!(t.params.live.color, Color::WHITE);
    assert_eq!(t.params.copy.color, Color::new(1.0, 0.0, 0.0, 1.0));

    // End deferred and flip
    scene.handle_request(set_deferred_mode_req(false, false), None);
    scene.apply_flip();

    let t = scene.stimuli[&h].stimulus.text().expect("expected Text stimulus");
    assert_eq!(t.text_live, "deferred");
    assert_eq!(t.params.live.color, Color::new(1.0, 0.0, 0.0, 1.0));
}

#[test]
fn test_create_text_wrong_target() {
    let mut scene = SceneState::new();
    let h = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateRect(proto::CreateRectRequest::default())),
    }, None).handle as u32;
    // CreateText must use system target
    let resp = scene.handle_request(proto::Request {
        target: Some(stim(h)),
        body: Some(request::Body::CreateText(proto::CreateTextRequest {
                                                 params: Some(proto::TextParams {
                                                     text: "bad".into(),
                                                     ..Default::default()
                                                 }),
                                                 ..Default::default()
                                             })),
    }, None);
    assert!(!is_ok(&resp));
    assert_eq!(resp.code, proto::ErrorCode::WrongTarget as i32);
}

#[test]
fn test_list_stimuli_includes_id_and_name() {
    let mut scene = SceneState::new();
    let h1 = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateRect(proto::CreateRectRequest {
                                                 identity: Some(proto::StimulusIdentity { name: "rect_a".into() }),
                                                 ..Default::default()
                                             })),
    }, None).handle as u32;
    let h2 = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateCircle(proto::CreateCircleRequest {
                                                   identity: Some(proto::StimulusIdentity { name: "disc_b".into() }),
                                                   ..Default::default()
                                               })),
    }, None).handle as u32;

    let resp = scene.handle_request(proto::Request {
        target: Some(sys()),
        body: Some(request::Body::ListStimuli(proto::ListStimuliRequest {})),
    }, None);
    assert!(is_ok(&resp));
    if let Some(proto::response::Body::StimulusList(list)) = resp.body {
        let by_handle: std::collections::HashMap<u32, &proto::StimulusEntry> =
            list.entries.iter().map(|e| (e.handle, e)).collect();
        assert_eq!(by_handle[&h1].name, "rect_a");
        assert_eq!(by_handle[&h2].name, "disc_b");
        assert!(!by_handle[&h1].id.is_empty());
        assert!(!by_handle[&h2].id.is_empty());
    } else {
        panic!("expected StimulusList");
    }
}

// ── Animations: action lines over the wire ────────────────────────────────────
//
// The proto path is what every client actually goes through, and an action line
// is only resolved when its bit is set in the mask — so a mask/line mismatch is
// the easy mistake to make and the one worth pinning down.

fn out_line(bank: u32, bit: u32) -> proto::VirtualTriggerLineHandle {
    use proto::virtual_trigger_line_handle::Handle;
    proto::VirtualTriggerLineHandle {
        handle: Some(Handle::BankBit(proto::VirtualTriggerLineBankBit { bank, bit })),
        kind: proto::VirtualTriggerLineKind::Output as i32,
    }
}

fn in_line(bank: u32, bit: u32) -> proto::VirtualTriggerLineHandle {
    use proto::virtual_trigger_line_handle::Handle;
    proto::VirtualTriggerLineHandle {
        handle: Some(Handle::BankBit(proto::VirtualTriggerLineBankBit { bank, bit })),
        kind: proto::VirtualTriggerLineKind::Input as i32,
    }
}

fn create_flash_req(cmd: proto::CreateAnimationRequest) -> proto::Request {
    proto::Request {
        target: Some(sys()),
        body: Some(request::Body::CreateAnimation(cmd)),
    }
}

/// A rect to hang the animation on; animations with no stimuli are legal but
/// less representative.
fn scene_with_rect() -> (SceneState, u32) {
    let mut scene = SceneState::new();
    let resp = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None);
    assert!(is_ok(&resp), "rect create failed: {}", resp.error);
    (scene, resp.handle as u32)
}

#[test]
fn create_animation_resolves_both_final_action_lines() {
    let (mut scene, h) = scene_with_rect();

    // DISABLE | REARM | FINAL_ACTION_TRIGGER_LINE | DONE_LEVEL
    let resp = scene
        .handle_request(
            create_flash_req(proto::CreateAnimationRequest {
                name: "flash".into(),
                target: Some(anim_target(vec![h])),
                final_action_mask: 0x01 | 0x02 | 0x08 | 0x100,
                final_action_trigger_line: Some(out_line(0, 37)),
                final_action_level_line: Some(out_line(0, 35)),
                body: Some(proto::create_animation_request::Body::FlashForNFrames(
                    proto::FlashForNFrames { duration_frames: 120 },
                )),
                ..Default::default()
            }),
            None,
        );
    assert!(is_ok(&resp), "create rejected: {}", resp.error);

    let anim = scene.animations.values().next().expect("no animation created");
    assert_eq!(anim.final_action_trigger_line.unwrap().bit, 37, "pulse line lost");
    assert_eq!(anim.final_action_level_line.unwrap().bit, 35, "level line lost");
}

/// The level line is only read when DONE_LEVEL is set — a line passed without
/// the bit is ignored rather than silently taking effect.
#[test]
fn level_line_without_the_done_level_bit_is_ignored() {
    let (mut scene, h) = scene_with_rect();

    scene
        .handle_request(
            create_flash_req(proto::CreateAnimationRequest {
                target: Some(anim_target(vec![h])),
                final_action_mask: 0x01, // DISABLE only
                final_action_level_line: Some(out_line(0, 35)),
                body: Some(proto::create_animation_request::Body::FlashForNFrames(
                    proto::FlashForNFrames { duration_frames: 10 },
                )),
                ..Default::default()
            }),
            None,
        );

    let anim = scene.animations.values().next().unwrap();
    assert!(anim.final_action_level_line.is_none());
}

/// An action line must address an output. An input-directed handle here is a
/// wiring mistake that would otherwise have vstimd writing an input line.
#[test]
fn done_level_rejects_an_input_line() {
    let (mut scene, h) = scene_with_rect();

    let resp = scene
        .handle_request(
            create_flash_req(proto::CreateAnimationRequest {
                target: Some(anim_target(vec![h])),
                final_action_mask: 0x100, // DONE_LEVEL
                final_action_level_line: Some(in_line(0, 11)),
                body: Some(proto::create_animation_request::Body::FlashForNFrames(
                    proto::FlashForNFrames { duration_frames: 10 },
                )),
                ..Default::default()
            }),
            None,
        );

    assert!(
        !is_ok(&resp),
        "an input line was accepted as a DONE_LEVEL output"
    );
    assert!(scene.animations.is_empty(), "the rejected animation was still created");
}

/// Query must report back what was created, including the new line — a client
/// that reads its own animation back should see both.
#[test]
fn query_animation_reports_the_level_line() {
    let (mut scene, h) = scene_with_rect();
    scene
        .handle_request(
            create_flash_req(proto::CreateAnimationRequest {
                target: Some(anim_target(vec![h])),
                final_action_mask: 0x100,
                final_action_level_line: Some(out_line(0, 35)),
                body: Some(proto::create_animation_request::Body::FlashForNFrames(
                    proto::FlashForNFrames { duration_frames: 10 },
                )),
                ..Default::default()
            }),
            None,
        );
    let handle = *scene.animations.keys().next().unwrap();

    let resp = scene
        .handle_request(
            proto::Request {
                target: Some(sys()),
                body: Some(request::Body::QueryAnimation(proto::QueryAnimationRequest {
                    handle,
                })),
            },
            None,
        );
    assert!(is_ok(&resp), "query failed: {}", resp.error);

    let Some(proto::response::Body::QueryAnimationResponse(q)) = resp.body else {
        panic!("unexpected query response body");
    };
    let params = q.params.expect("query returned no params");
    assert_eq!(params.final_action_mask & 0x100, 0x100, "DONE_LEVEL not reported");
    assert!(params.final_action_level_line.is_some(), "level line not reported");
}

// ── Shared opacity ────────────────────────────────────────────────────────────

/// One stimulus of every type, so the shared-property tests can loop.
fn one_of_each(scene: &mut SceneState) -> Vec<(&'static str, u32)> {
    let mut out = vec![];
    for (name, body) in [
        ("Rect", request::Body::CreateRect(proto::CreateRectRequest::default())),
        ("Circle", request::Body::CreateCircle(proto::CreateCircleRequest::default())),
        ("Ellipse", request::Body::CreateEllipse(proto::CreateEllipseRequest::default())),
        ("Grating", request::Body::CreateGrating(proto::CreateGratingRequest::default())),
        (
            "Text",
            request::Body::CreateText(proto::CreateTextRequest {
                                          params: Some(proto::TextParams {
                                              text: "hello".into(),
                                              ..Default::default()
                                          }),
                                          ..Default::default()
                                      }),
        ),
    ] {
        let resp = scene.handle_request(
            proto::Request { target: Some(sys()), body: Some(body) },
            None,
        );
        assert!(is_ok(&resp), "creating a {name} failed: {}", resp.error);
        out.push((name, resp.handle as u32));
    }
    out
}

fn set_alpha(scene: &mut SceneState, handle: u32, opacity: f32) -> proto::Response {
    scene.handle_request(
        proto::Request {
            target: Some(stim(handle)),
            body: Some(request::Body::SetAlpha(proto::SetAlphaRequest { opacity })),
        },
        None,
    )
}

/// Opacity is shared state: SetAlpha works on every type, not just shapes.
#[test]
fn test_set_alpha_applies_to_every_stimulus_type() {
    let mut scene = SceneState::new();
    for (name, h) in one_of_each(&mut scene) {
        let resp = set_alpha(&mut scene, h, 0.25);
        assert!(is_ok(&resp), "SetAlpha on a {name} was rejected: {}", resp.error);
        assert_eq!(
            scene.stimuli[&h].stimulus.opacity().live,
            0.25,
            "{name} did not take the opacity",
        );
    }
}

/// Only shapes re-tessellate on an opacity change: they bake it into their
/// vertex colours, while grating and text read it from live state into push
/// constants every frame. Marking text dirty here would re-shape and
/// re-rasterize every glyph, so a fade would cost a full text layout per frame.
#[test]
fn test_set_alpha_only_dirties_what_has_to_be_retessellated() {
    let mut scene = SceneState::new();
    for (name, h) in one_of_each(&mut scene) {
        // Stimuli are born dirty (nothing has drawn them yet); clear it the way
        // the render thread does after uploading a mesh.
        scene.stimuli.get_mut(&h).unwrap().stimulus.flags_mut().dirty = false;
        assert!(is_ok(&set_alpha(&mut scene, h, 0.5)));

        let stim = &scene.stimuli[&h].stimulus;
        assert_eq!(
            stim.flags().dirty,
            stim.shape().is_some(),
            "{name}: dirty should be set only for shapes",
        );
    }
}

/// Opacity multiplies the colours rather than replacing any one of them, so a
/// half-transparent fill under an opaque outline keeps that relationship.
#[test]
fn test_set_alpha_leaves_per_colour_alpha_alone() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;
    for (body, _) in [
        (request::Body::SetFillColor(proto::SetFillColorRequest {
            color: Some(proto::Color { r: 1.0, g: 0.0, b: 0.0, a: 0.5 }),
        }), ()),
        (request::Body::SetOutlineColor(proto::SetOutlineColorRequest {
            color: Some(proto::Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }),
        }), ()),
    ] {
        let resp = scene.handle_request(
            proto::Request { target: Some(stim(h)), body: Some(body) },
            None,
        );
        assert!(is_ok(&resp));
    }
    assert!(is_ok(&set_alpha(&mut scene, h, 0.5)));

    let stim = &scene.stimuli[&h].stimulus;
    let app = stim.shape_appearance().expect("expected a shape");
    assert_eq!(app.live.fill_color.a, 0.5, "the fill's own alpha was overwritten");
    assert_eq!(app.live.outline_color.a, 1.0, "the outline's own alpha was overwritten");
    assert_eq!(stim.opacity().live, 0.5);
}

#[test]
fn test_set_alpha_clamps() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;

    assert!(is_ok(&set_alpha(&mut scene, h, 4.0)));
    assert_eq!(scene.stimuli[&h].stimulus.opacity().live, 1.0);
    assert!(is_ok(&set_alpha(&mut scene, h, -1.0)));
    assert_eq!(scene.stimuli[&h].stimulus.opacity().live, 0.0);
}

#[test]
fn test_set_alpha_is_deferred_with_everything_else() {
    let mut scene = SceneState::new();
    let h = scene
        .handle_request(create_rect_req(sys(), proto::CreateRectRequest::default()), None)
        .handle as u32;

    assert!(is_ok(&scene.handle_request(set_deferred_mode_req(true, false), None)));
    assert!(is_ok(&set_alpha(&mut scene, h, 0.3)));
    assert_eq!(scene.stimuli[&h].stimulus.opacity().live, 1.0, "opacity changed before the flip");

    assert!(is_ok(&scene.handle_request(set_deferred_mode_req(false, false), None)));
    scene.apply_flip();
    assert_eq!(scene.stimuli[&h].stimulus.opacity().live, 0.3);
}

/// The query reports the shared property, for every type — not a per-type
/// synthesis out of whichever colour that type happens to have.
#[test]
fn test_query_reports_shared_opacity() {
    let mut scene = SceneState::new();
    for (name, h) in one_of_each(&mut scene) {
        assert!(is_ok(&set_alpha(&mut scene, h, 0.4)));
        let resp = scene.handle_request(
            proto::Request {
                target: Some(stim(h)),
                body: Some(request::Body::QueryStimulus(proto::QueryStimulusRequest {})),
            },
            None,
        );
        let Some(proto::response::Body::StimulusInfo(q)) = resp.body else {
            panic!("{name}: unexpected query response body");
        };
        assert_eq!(q.opacity, 0.4, "{name} reported the wrong opacity");
    }
}

// ── Sizes are full extents, end to end ────────────────────────────────────────

/// Every create command takes full width_px/height_px and the scene stores exactly
/// that, with no halving anywhere in between. This is the invariant the v3
/// config format rests on, and it was missing for gratings — CreateGrating kept
/// halving into the scene while the query multiplied back, so the wire looked
/// right and the saved config was half size.
#[test]
fn test_create_stores_full_extents() {
    let mut scene = SceneState::new();

    let h = scene
        .handle_request(create_rect_req(
            sys(),
            proto::CreateRectRequest {
                params: Some(proto::RectParams { width_px: 200.0, height_px: 100.0, ..Default::default() }),
                ..Default::default()
            },
        ), None)
        .handle as u32;
    let r = scene.stimuli[&h].stimulus.shape().expect("expected Rect");
    assert_eq!(r.geometry.live.size_px(), Some([200.0, 100.0]));

    let h = scene
        .handle_request(proto::Request {
            target: Some(sys()),
            body: Some(request::Body::CreateEllipse(proto::CreateEllipseRequest {
                                                        params: Some(proto::EllipseParams {
                                                            width_px: 300.0,
                                                            height_px: 120.0,
                                                            ..Default::default()
                                                        }),
                                                        ..Default::default()
                                                    })),
        }, None)
        .handle as u32;
    let e = scene.stimuli[&h].stimulus.shape().expect("expected Ellipse");
    assert_eq!(e.geometry.live.size_px(), Some([300.0, 120.0]));

    let h = scene
        .handle_request(proto::Request {
            target: Some(sys()),
            body: Some(request::Body::CreateGrating(proto::CreateGratingRequest {
                                                        params: Some(proto::GratingParams {
                                                            width_px: 400.0,
                                                            height_px: 250.0,
                                                            ..Default::default()
                                                        }),
                                                        ..Default::default()
                                                    })),
        }, None)
        .handle as u32;
    let g = scene.stimuli[&h].stimulus.grating().expect("expected Grating");
    assert_eq!(g.size_px.live, [400.0, 250.0]);
}

/// …and a query reports the same numbers the create took, for every sized type.
#[test]
fn test_query_reports_the_size_that_was_asked_for() {
    let mut scene = SceneState::new();
    let cases: Vec<(&str, request::Body, fn(&proto::StimulusParams) -> (f32, f32))> = vec![
        (
            "rect",
            request::Body::CreateRect(proto::CreateRectRequest {
                                          params: Some(proto::RectParams {
                                              width_px: 200.0,
                                              height_px: 100.0,
                                              ..Default::default()
                                          }),
                                          ..Default::default()
                                      }),
            |p| match p.shape.as_ref().unwrap() {
                proto::stimulus_params::Shape::Rect(r) => (r.width_px, r.height_px),
                _ => panic!("wrong params type"),
            },
        ),
        (
            "ellipse",
            request::Body::CreateEllipse(proto::CreateEllipseRequest {
                                             params: Some(proto::EllipseParams {
                                                 width_px: 200.0,
                                                 height_px: 100.0,
                                                 ..Default::default()
                                             }),
                                             ..Default::default()
                                         }),
            |p| match p.shape.as_ref().unwrap() {
                proto::stimulus_params::Shape::Ellipse(e) => (e.width_px, e.height_px),
                _ => panic!("wrong params type"),
            },
        ),
        (
            "grating",
            request::Body::CreateGrating(proto::CreateGratingRequest {
                                             params: Some(proto::GratingParams {
                                                 width_px: 200.0,
                                                 height_px: 100.0,
                                                 ..Default::default()
                                             }),
                                             ..Default::default()
                                         }),
            |p| match p.shape.as_ref().unwrap() {
                proto::stimulus_params::Shape::Grating(g) => (g.width_px, g.height_px),
                _ => panic!("wrong params type"),
            },
        ),
    ];

    for (name, body, extract) in cases {
        let h = scene
            .handle_request(proto::Request { target: Some(sys()), body: Some(body) }, None)
            .handle as u32;
        let resp = scene.handle_request(proto::Request {
            target: Some(stim(h)),
            body: Some(request::Body::QueryStimulus(proto::QueryStimulusRequest {})),
        }, None);
        let Some(proto::response::Body::StimulusInfo(info)) = resp.body else {
            panic!("{name}: unexpected query response");
        };
        assert_eq!(
            extract(&info.params.unwrap()),
            (200.0, 100.0),
            "{name}: query did not report the size it was created with",
        );
    }
}

// ── Create-time appearance (proto `appearance` field) ─────────────────────────

/// Creating an outlined shape used to take four round trips — create, then
/// SetDrawMode, SetOutlineColor, SetOutlineWidth — with the stimulus on screen
/// wearing the wrong appearance for three of them. `CreateRect.appearance` makes
/// it one message.
#[test]
fn create_rect_accepts_a_full_appearance() {
    let mut scene = SceneState::new();
    let resp = scene.handle_request(
        create_rect_req(
            sys(),
            proto::CreateRectRequest {
                placement: Some(proto::Transform2D { pos_px: None, rotation_deg: 15.0 }),
                params: Some(proto::RectParams {
                    width_px: 100.0,
                    height_px: 50.0,
                    appearance: Some(proto::ShapeAppearance {
                    fill_color: Some(Color::new(0.1, 0.2, 0.3, 1.0).into()),
                    outline_color: Some(Color::new(0.9, 0.8, 0.7, 1.0).into()),
                    outline_width_px: 6.0,
                    draw_mode: proto::ShapeDrawMode::FilledAndOutlined as i32,
                })
                }),
                ..Default::default()
            },
        ),
        None,
    );
    assert!(is_ok(&resp), "unexpected error: {}", resp.error);
    let h = resp.handle as u32;
    let stim = &scene.stimuli[&h].stimulus;
    let app = stim.shape_appearance().expect("expected a shape").live;
    assert_eq!(app.fill_color, Color::new(0.1, 0.2, 0.3, 1.0));
    assert_eq!(app.outline_color, Color::new(0.9, 0.8, 0.7, 1.0));
    assert_eq!(app.stroke_width_px, 6.0);
    assert_eq!(app.draw_mode, vstimd::scene::DrawMode::FillAndStroke);
    // A rect can now be born rotated, as an ellipse always could.
    assert_eq!(
        stim.transform2d().expect("2-D stimulus").live.angle_deg,
        15.0
    );
}

/// Absent `appearance` must reproduce the pre-field behaviour exactly: fill from
/// `fill_color`, outline from the scene default, stroke width_px and draw mode from
/// `ShapeAppearance::default()`.
#[test]
fn create_rect_without_appearance_is_unchanged() {
    let mut scene = SceneState::new();
    let default_outline = scene.config.default_outline;
    let resp = scene.handle_request(
        create_rect_req(
            sys(),
            proto::CreateRectRequest {
                params: Some(proto::RectParams {
                    appearance: Some(proto::ShapeAppearance { fill_color: Some(Color::new(1.0, 0.0, 0.0, 1.0).into()), ..Default::default() }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ),
        None,
    );
    assert!(is_ok(&resp));
    let app = scene.stimuli[&(resp.handle as u32)]
        .stimulus
        .shape_appearance()
        .expect("expected a shape")
        .live;
    assert_eq!(app.fill_color, Color::new(1.0, 0.0, 0.0, 1.0));
    assert_eq!(app.outline_color, default_outline);
    assert_eq!(app.stroke_width_px, 2.0);
    assert_eq!(app.draw_mode, vstimd::scene::DrawMode::Fill);
}

/// A partially-filled `appearance` inherits the rest rather than zeroing it.
#[test]
fn create_ellipse_appearance_fields_fall_back_individually() {
    let mut scene = SceneState::new();
    let default_outline = scene.config.default_outline;
    let default_fill = scene.config.default_fill;
    let resp = scene.handle_request(
        proto::Request {
            target: Some(sys()),
            body: Some(request::Body::CreateEllipse(proto::CreateEllipseRequest {
                                                        params: Some(proto::EllipseParams {
                                                            appearance: Some(proto::ShapeAppearance {
                    draw_mode: proto::ShapeDrawMode::Outlined as i32,
                    ..Default::default()
                }),
                                                            ..Default::default()
                                                        }),
                                                        ..Default::default()
                                                    })),
        },
        None,
    );
    assert!(is_ok(&resp), "unexpected error: {}", resp.error);
    let app = scene.stimuli[&(resp.handle as u32)]
        .stimulus
        .shape_appearance()
        .expect("expected a shape")
        .live;
    assert_eq!(app.draw_mode, vstimd::scene::DrawMode::Stroke);
    assert_eq!(app.fill_color, default_fill, "fill should fall back");
    assert_eq!(app.outline_color, default_outline, "outline should fall back");
    assert_eq!(app.stroke_width_px, 2.0, "width_px 0 means unset, not hairline");
}
