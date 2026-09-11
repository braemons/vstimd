//! Conditions: the switch, the two gates it drives, name resolution, the
//! per-animation switch policy, and the round-trip through a scene-config.
//!
//! Like the other command tests these drive `handle_request` on a bare
//! `SceneState` — no ZMQ, no GPU.

use vstimd::proto;
use vstimd::proto::request;
use vstimd::scene::animation::{AnimState, Animation, AnimationEntry};
use vstimd::scene::conditions::ConditionAction;
use vstimd::scene::{
    SceneState, Shape, ShapeAppearance, ShapeGeometry, Stimulus, StimulusIdentity,
    StimulusSceneEntry,
};
use vstimd::vtl_state::VtlOutputs;

// ── helpers ───────────────────────────────────────────────────────────────────

fn sys() -> request::Target {
    request::Target::System(proto::SystemTarget {})
}

fn send(sc: &mut SceneState, target: request::Target, body: request::Body) -> proto::Response {
    sc.handle_request(
        proto::Request { target: Some(target), body: Some(body) },
        None,
    )
}

fn is_ok(r: &proto::Response) -> bool {
    r.code == proto::ErrorCode::Ok as i32
}

/// A rect stimulus, enabled, with no condition membership.
fn add_rect(sc: &mut SceneState) -> u32 {
    sc.add_stimulus(StimulusSceneEntry::new(
        StimulusIdentity::new(None),
        Stimulus::from(Shape::new(
            [0.0, 0.0],
            0.0,
            ShapeAppearance::default(),
            ShapeGeometry::Rect { size_px: [50.0, 50.0] },
        )),
    ))
}

/// A flash animation over `stimuli`, armed.
fn add_anim(sc: &mut SceneState, stimuli: Vec<u32>) -> u32 {
    sc.add_animation(AnimationEntry::armed(
        Animation::FlashForNFrames { duration_frames: 10 },
        stimuli,
    ))
}

fn set_condition(sc: &mut SceneState, index: u32) -> proto::Response {
    send(
        sc,
        sys(),
        request::Body::SetCondition(proto::SetConditionRequest {
            condition: Some(proto::set_condition_request::Condition::Index(index)),
        }),
    )
}

fn set_condition_by_name(sc: &mut SceneState, name: &str) -> proto::Response {
    send(
        sc,
        sys(),
        request::Body::SetCondition(proto::SetConditionRequest {
            condition: Some(proto::set_condition_request::Condition::Name(name.into())),
        }),
    )
}

fn set_stimulus_conditions(sc: &mut SceneState, handle: u32, indices: Vec<u32>) -> proto::Response {
    send(
        sc,
        request::Target::Stimulus(handle),
        request::Body::SetStimulusConditions(proto::SetStimulusConditionsRequest {
            condition_indices: indices,
        }),
    )
}

fn set_animation_conditions(
    sc: &mut SceneState,
    handle: u32,
    indices: Vec<u32>,
    action: proto::ConditionAction,
) -> proto::Response {
    send(
        sc,
        sys(),
        request::Body::SetAnimationConditions(proto::SetAnimationConditionsRequest {
            handle,
            condition_indices: indices,
            condition_action: action as i32,
        }),
    )
}

fn declare(sc: &mut SceneState, conditions: &[(u32, &str)]) -> proto::Response {
    send(
        sc,
        sys(),
        request::Body::DeclareConditions(proto::DeclareConditionsRequest {
            conditions: conditions
                .iter()
                .map(|(index, name)| proto::Condition { index: *index, name: (*name).into() })
                .collect(),
        }),
    )
}

fn visible(sc: &SceneState, handle: u32) -> bool {
    sc.stimuli[&handle].stimulus.is_visible()
}

// ── The gate ──────────────────────────────────────────────────────────────────

#[test]
fn a_stimulus_with_no_membership_is_visible_in_every_condition() {
    let mut sc = SceneState::new();
    let h = add_rect(&mut sc);
    for index in [0, 1, 7, 42] {
        assert!(is_ok(&set_condition(&mut sc, index)));
        assert!(visible(&sc, h), "hidden in condition {index}");
    }
}

#[test]
fn a_stimulus_is_visible_only_in_the_conditions_it_belongs_to() {
    let mut sc = SceneState::new();
    let h = add_rect(&mut sc);
    assert!(is_ok(&set_stimulus_conditions(&mut sc, h, vec![1, 3])));

    assert!(!visible(&sc, h), "condition 0 is not in [1, 3]");
    set_condition(&mut sc, 1);
    assert!(visible(&sc, h));
    set_condition(&mut sc, 2);
    assert!(!visible(&sc, h));
    set_condition(&mut sc, 3);
    assert!(visible(&sc, h));
}

#[test]
fn the_condition_gate_leaves_the_operators_enabled_flag_alone() {
    let mut sc = SceneState::new();
    let h = add_rect(&mut sc);
    set_stimulus_conditions(&mut sc, h, vec![0]);

    // Disabled by hand, then hidden and shown again by the protocol: the
    // stimulus must come back disabled, not enabled.
    send(
        &mut sc,
        request::Target::Stimulus(h),
        request::Body::SetEnabled(proto::SetEnabledRequest { enabled: false }),
    );
    set_condition(&mut sc, 1);
    set_condition(&mut sc, 0);

    assert!(!sc.stimuli[&h].stimulus.flags().enabled);
    assert!(sc.stimuli[&h].stimulus.flags().cond_enabled);
    assert!(!visible(&sc, h));
}

/// One of every constructible stimulus type, created the way a client creates
/// them, as `(type name, handle)`.
///
/// `Polygon` is absent because `CreatePolygon` is refused by `ipc/dispatch` —
/// it has a wire value and no scene representation. The 3-D types are absent
/// for the mirror reason: they exist in the scene taxonomy and own no wire
/// value yet. Both will need a row here on the day they become constructible,
/// which is what the count assertion below is for.
fn add_one_of_every_type(sc: &mut SceneState) -> Vec<(&'static str, u32)> {
    let creates: Vec<request::Body> = vec![
        request::Body::CreateRect(proto::CreateRectRequest::default()),
        request::Body::CreateCircle(proto::CreateCircleRequest::default()),
        request::Body::CreateEllipse(proto::CreateEllipseRequest::default()),
        request::Body::CreateGrating(proto::CreateGratingRequest::default()),
        request::Body::CreateText(proto::CreateTextRequest::default()),
        request::Body::CreateDots(proto::CreateDotsRequest::default()),
    ];
    creates
        .into_iter()
        .map(|body| {
            let r = send(sc, sys(), body);
            assert!(is_ok(&r), "create failed: {}", r.error);
            let handle = r.handle as u32;
            (sc.stimuli[&handle].stimulus.type_name(), handle)
        })
        .collect()
}

/// The condition gate is a property of every stimulus, not of the ones that
/// happened to exist when conditions were written.
///
/// It lives on `StimulusFlags`, above the body, and `apply_conditions` walks
/// every entry regardless of type — so this holds by construction. It is
/// asserted anyway because "by construction" is exactly the kind of claim that
/// stops being true when a body type grows its own visibility path.
#[test]
fn every_stimulus_type_carries_the_condition_gate() {
    let mut sc = SceneState::new();
    let stimuli = add_one_of_every_type(&mut sc);

    let mut names: Vec<&str> = stimuli.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["Circle", "Dots", "Ellipse", "Grating", "Rect", "Text"],
        "a stimulus type became constructible without being covered here",
    );

    for (name, h) in &stimuli {
        assert!(is_ok(&set_stimulus_conditions(&mut sc, *h, vec![1])), "{name}");
    }

    set_condition(&mut sc, 0);
    for (name, h) in &stimuli {
        assert!(!visible(&sc, *h), "{name} is visible outside its condition");
    }
    set_condition(&mut sc, 1);
    for (name, h) in &stimuli {
        assert!(visible(&sc, *h), "{name} is hidden inside its condition");
    }
}

/// Membership and the gate it implies are reported back for every type, so a
/// client can see which protocol step a stimulus belongs to whatever it is.
#[test]
fn every_stimulus_type_reports_its_condition_membership() {
    let mut sc = SceneState::new();
    let stimuli = add_one_of_every_type(&mut sc);
    for (_, h) in &stimuli {
        set_stimulus_conditions(&mut sc, *h, vec![2]);
    }
    set_condition(&mut sc, 2);

    for (name, h) in &stimuli {
        let r = send(
            &mut sc,
            request::Target::Stimulus(*h),
            request::Body::QueryStimulus(proto::QueryStimulusRequest {}),
        );
        assert!(is_ok(&r), "{name}: {}", r.error);
        let Some(proto::response::Body::StimulusInfo(info)) = r.body else {
            panic!("{name}: query returned no stimulus body");
        };
        assert_eq!(info.condition_indices, vec![2], "{name}");
        assert!(info.condition_enabled, "{name}");
    }
}

// ── Names ─────────────────────────────────────────────────────────────────────

#[test]
fn a_declared_condition_can_be_switched_to_by_name() {
    let mut sc = SceneState::new();
    assert!(is_ok(&declare(&mut sc, &[(0, "baseline"), (2, "probe")])));
    assert!(is_ok(&set_condition_by_name(&mut sc, "probe")));
    assert_eq!(sc.conditions.active, 2);
    assert_eq!(sc.conditions.active_name(), "probe");
    assert_eq!(sc.conditions.active_label(), "2 (probe)");
}

#[test]
fn an_unknown_name_is_refused_rather_than_guessed_at() {
    let mut sc = SceneState::new();
    declare(&mut sc, &[(1, "probe")]);
    set_condition(&mut sc, 1);

    let resp = set_condition_by_name(&mut sc, "prboe");
    assert_eq!(resp.code, proto::ErrorCode::InvalidArgument as i32);
    assert_eq!(sc.conditions.active, 1, "a typo must not move the protocol");
}

#[test]
fn an_undeclared_index_is_a_valid_nameless_condition() {
    let mut sc = SceneState::new();
    assert!(is_ok(&set_condition(&mut sc, 9)));
    assert_eq!(sc.conditions.active, 9);
    assert_eq!(sc.conditions.active_name(), "");
    assert_eq!(sc.conditions.active_label(), "9");
}

#[test]
fn duplicate_indices_or_names_are_refused() {
    let mut sc = SceneState::new();
    assert_eq!(
        declare(&mut sc, &[(0, "a"), (0, "b")]).code,
        proto::ErrorCode::InvalidArgument as i32,
    );
    assert_eq!(
        declare(&mut sc, &[(0, "a"), (1, "a")]).code,
        proto::ErrorCode::InvalidArgument as i32,
    );
    assert!(sc.conditions.declared.is_empty(), "a refused set is not applied");
    // Two unnamed conditions are not duplicates of each other.
    assert!(is_ok(&declare(&mut sc, &[(0, ""), (1, "")])));
}

#[test]
fn list_conditions_reports_the_declared_set_and_the_active_one() {
    let mut sc = SceneState::new();
    declare(&mut sc, &[(0, "baseline"), (1, "")]);
    set_condition(&mut sc, 1);

    let resp = send(&mut sc, sys(), request::Body::ListConditions(proto::ListConditionsRequest {}));
    let Some(proto::response::Body::ConditionList(list)) = resp.body else {
        panic!("expected a condition list");
    };
    assert_eq!(list.active_index, 1);
    assert_eq!(list.active_name, "");
    assert_eq!(list.conditions.len(), 2);
    assert_eq!(list.conditions[0].name, "baseline");
    assert_eq!(list.conditions[1].name, "");
}

// ── Animations ────────────────────────────────────────────────────────────────

#[test]
fn an_animation_outside_the_active_condition_does_not_advance() {
    let mut sc = SceneState::new();
    let stim = add_rect(&mut sc);
    let anim = add_anim(&mut sc, vec![stim]);
    // HOLD so the switch itself does not change the state, isolating the skip.
    set_animation_conditions(&mut sc, anim, vec![1], proto::ConditionAction::Hold);

    let before = sc.animations[&anim].state.clone();
    let edges = Default::default();
    let mut levels = [0u64; vtl::MAX_BANKS];
    let mut pulses = [0u64; vtl::MAX_BANKS];
    sc.advance_animations(
        &edges,
        &edges,
        &mut VtlOutputs { levels: &mut levels, pulses: &mut pulses },
    );

    assert_eq!(sc.animations[&anim].state, before, "advanced while inactive");
    assert!(!sc.animations[&anim].cond_enabled);
}

#[test]
fn reset_re_arms_on_the_way_in_and_idles_on_the_way_out() {
    let mut sc = SceneState::new();
    let stim = add_rect(&mut sc);
    let anim = add_anim(&mut sc, vec![stim]);
    set_animation_conditions(&mut sc, anim, vec![1], proto::ConditionAction::Reset);

    // Membership excludes the active condition 0, so it was idled on the way out.
    assert_eq!(sc.animations[&anim].state, AnimState::Idle);

    set_condition(&mut sc, 1);
    assert_eq!(sc.animations[&anim].state, AnimState::Armed, "not re-armed on entry");

    set_condition(&mut sc, 0);
    assert_eq!(sc.animations[&anim].state, AnimState::Idle, "not idled on exit");
}

#[test]
fn hold_leaves_the_lifecycle_state_alone_and_stop_only_idles_on_exit() {
    let mut sc = SceneState::new();
    let stim = add_rect(&mut sc);

    let held = add_anim(&mut sc, vec![stim]);
    set_animation_conditions(&mut sc, held, vec![1], proto::ConditionAction::Hold);
    assert_eq!(sc.animations[&held].state, AnimState::Armed, "HOLD must not idle");

    let stopped = add_anim(&mut sc, vec![stim]);
    set_animation_conditions(&mut sc, stopped, vec![1], proto::ConditionAction::Stop);
    assert_eq!(sc.animations[&stopped].state, AnimState::Idle);

    set_condition(&mut sc, 1);
    assert_eq!(sc.animations[&held].state, AnimState::Armed);
    assert_eq!(sc.animations[&stopped].state, AnimState::Idle, "STOP must not re-arm");
}

// ── Persistence ───────────────────────────────────────────────────────────────

#[test]
fn conditions_survive_a_scene_config_round_trip() {
    let mut sc = SceneState::new();
    let always = add_rect(&mut sc);
    let probe_only = add_rect(&mut sc);
    let anim = add_anim(&mut sc, vec![probe_only]);
    declare(&mut sc, &[(0, "baseline"), (1, "probe")]);
    set_stimulus_conditions(&mut sc, probe_only, vec![1]);
    set_animation_conditions(&mut sc, anim, vec![1], proto::ConditionAction::Hold);
    set_condition(&mut sc, 1);

    let json = serde_json::to_string(&sc.config).expect("serialize");
    let restored: vstimd::scene::SceneConfig = serde_json::from_str(&json).expect("deserialize");

    let mut sc2 = SceneState::new();
    sc2.load_snapshot(restored, vstimd::scene::LoadMode::Replace);

    assert_eq!(sc2.conditions.active, 1);
    assert_eq!(sc2.conditions.index_of_name("probe"), Some(1));
    assert_eq!(sc2.stimuli[&probe_only].conditions, vec![1]);
    assert_eq!(sc2.animations[&anim].condition_action, ConditionAction::Hold);
    // And the derived gates are re-derived, not restored from the file.
    assert!(visible(&sc2, always));
    assert!(visible(&sc2, probe_only));
    sc2.set_condition(0);
    assert!(!visible(&sc2, probe_only));
}

#[test]
fn a_scene_with_no_conditions_writes_no_conditions_block() {
    let mut sc = SceneState::new();
    add_rect(&mut sc);
    let json = serde_json::to_string(&sc.config).expect("serialize");
    assert!(!json.contains("conditions"), "empty conditions must not reach the file");
}
