use uuid::Uuid;
use vstimd::scene_config_file::{parse_config_json, retrieve_config_json};
use vstimd::scene::animation::AnimState;
use vstimd::scene::{
    CircleStimulus, Deferred, LoadMode, RectStimulus, SceneConfig, SceneState, ShapeAppearance,
    StimulusCommon, StimulusFlags, Stimulus, StimulusSceneEntry,
};
use vstimd::vtl_state::{VtlConfig, VtlNameEntry};
use vtl::VtlKind;

fn make_rect_entry() -> StimulusSceneEntry {
    StimulusSceneEntry::new(
        Uuid::new_v4(),
        Some("test_rect".into()),
        Stimulus::Rect(RectStimulus {
            common: StimulusCommon::new([100.0, -50.0], 45.0),
            appearance: Deferred::new(ShapeAppearance {
                fill_color: vstimd::Color::new(1.0, 0.5, 0.0, 1.0),
                ..Default::default()
            }),
            size: Deferred::new([200.0, 80.0]),
        }),
    )
}

fn make_circle_entry() -> StimulusSceneEntry {
    StimulusSceneEntry::new(
        Uuid::new_v4(),
        Some("test_circle".into()),
        Stimulus::Circle(CircleStimulus {
            // Disabled on purpose: the round-trip must carry the flag, not just
            // the geometry.
            common: StimulusCommon { flags: StimulusFlags::enabled(false), ..StimulusCommon::new([-200.0, 300.0], 0.0) },
            appearance: Deferred::new(ShapeAppearance {
                fill_color: vstimd::Color::new(0.0, 0.0, 1.0, 1.0),
                ..Default::default()
            }),
            radius: Deferred::new(75.0),
        }),
    )
}

#[test]
fn roundtrip_empty_scene() {
    let scene = SceneConfig::default();
    let vtl = VtlConfig::default();
    let json = retrieve_config_json(&scene, &vtl).unwrap();
    let (loaded, _io) = parse_config_json(&json).unwrap();
    assert_eq!(loaded.stimuli.len(), 0);
    assert_eq!(loaded.animations.len(), 0);
}

#[test]
fn roundtrip_rect_stimulus() {
    let mut scene = SceneState::new();
    scene.add_stimulus(make_rect_entry());

    let vtl = VtlConfig::default();
    let json = retrieve_config_json(&scene.config, &vtl).unwrap();
    let (loaded, _io) = parse_config_json(&json).unwrap();

    assert_eq!(loaded.stimuli.len(), 1);
    let entry = loaded.stimuli.values().next().unwrap();
    assert_eq!(entry.name.as_deref(), Some("test_rect"));
    let Stimulus::Rect(rect) = &entry.stimulus else {
        panic!("expected rect");
    };
    assert_eq!(rect.common.transform.live.pos, [100.0, -50.0]);
    assert!((rect.appearance.live.fill_color.r - 1.0).abs() < 1e-6);
}

#[test]
fn roundtrip_multiple_stimuli() {
    let mut scene = SceneState::new();
    scene.add_stimulus(make_rect_entry());
    scene.add_stimulus(make_circle_entry());

    let vtl = VtlConfig::default();
    let json = retrieve_config_json(&scene.config, &vtl).unwrap();
    let (loaded, _io) = parse_config_json(&json).unwrap();

    assert_eq!(loaded.stimuli.len(), 2);
}

#[test]
fn roundtrip_vtl_names() {
    let scene = SceneConfig::default();
    let vtl = VtlConfig {
        names: vec![
            VtlNameEntry {
                name: "stim_onset".into(),
                bank: 0,
                bit: 0,
                kind: VtlKind::Output,
            },
            VtlNameEntry {
                name: "trial_start".into(),
                bank: 0,
                bit: 1,
                kind: VtlKind::Input,
            },
        ],
    };
    let json = retrieve_config_json(&scene, &vtl).unwrap();
    let (_loaded, sections) = parse_config_json(&json).unwrap();

    assert_eq!(sections.vtl.names.len(), 2);
    assert_eq!(sections.vtl.names[0].name, "stim_onset");
    assert_eq!(sections.vtl.names[1].name, "trial_start");
    assert_eq!(sections.vtl.names[0].kind, VtlKind::Output);
    assert_eq!(sections.vtl.names[1].kind, VtlKind::Input);
}

#[test]
fn roundtrip_background_color() {
    let scene = SceneConfig {
        background: Deferred::new(vstimd::Color::new(0.2, 0.3, 0.4, 1.0)),
        ..Default::default()
    };

    let vtl = VtlConfig::default();
    let json = retrieve_config_json(&scene, &vtl).unwrap();
    let (loaded, _io) = parse_config_json(&json).unwrap();

    assert_eq!(
        loaded.background.live,
        vstimd::Color::new(0.2, 0.3, 0.4, 1.0)
    );
}

#[test]
fn roundtrip_additive_load_remaps_handles() {
    let mut scene = SceneState::new();
    let h1 = scene.add_stimulus(make_rect_entry());

    let vtl = VtlConfig::default();
    let json = retrieve_config_json(&scene.config, &vtl).unwrap();
    let (snap, _io) = parse_config_json(&json).unwrap();

    // Load the same snapshot additively
    scene.load_snapshot(snap, LoadMode::Additive);

    // Should now have 2 stimuli with no handle collision
    assert_eq!(scene.stimuli.len(), 2);
    let handles: Vec<u32> = scene.stimuli.keys().copied().collect();
    assert_eq!(
        handles
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        2
    );
    assert!(handles.contains(&h1));
}

#[test]
fn roundtrip_replace_load() {
    let mut scene = SceneState::new();
    scene.add_stimulus(make_rect_entry());
    scene.add_stimulus(make_circle_entry());

    // Serialize only 1-stimulus scene
    let mut one_stim = SceneState::new();
    one_stim.add_stimulus(make_rect_entry());
    let vtl = VtlConfig::default();
    let json = retrieve_config_json(&one_stim.config, &vtl).unwrap();
    let (snap, _io) = parse_config_json(&json).unwrap();

    // Replace the 2-stimulus scene with the 1-stimulus snapshot
    scene.load_snapshot(snap, LoadMode::Replace);
    assert_eq!(scene.stimuli.len(), 1);
}

#[test]
fn config_version_mismatch_rejected() {
    let json = r#"{"version":99,"scene":{"background":[0,0,0,1],"default_fill":[1,1,1,1],"default_outline":[0,0,0,1],"photodiode":{"lit":false,"live":[1,1,1,1],"copy":[1,1,1,1],"position":"BottomLeft","size":0.05},"stimuli":{},"next_stim_handle":1,"animations":{},"next_anim_handle":1},"io":{"vtl":{"names":[]}}}"#;
    assert!(parse_config_json(json).is_err());
}

// ── Animation state across a load ─────────────────────────────────────────────
//
// A config's animation state is intent, not a resumable snapshot. `Armed` has
// to survive a load — that is what lets a saved (or shipped) scene come up
// waiting for its trigger — while `Running` must not resume mid-run and `Done`
// must not silently re-run.

/// Build a scene holding one animation in `state`, round-trip it through the
/// config format, and return the state it loads back as.
fn state_after_roundtrip(state: AnimState) -> AnimState {
    use vstimd::scene::animation::{Animation, AnimationEntry};

    let mut saved = SceneState::new();
    let h = saved.add_stimulus(make_rect_entry());
    let mut entry = AnimationEntry::new(
        Animation::FlashForNFrames {
            duration_frames: 30,
        },
        vec![h],
    );
    entry.state = state;
    saved.add_animation(entry);

    let json = retrieve_config_json(&saved.config, &VtlConfig::default()).unwrap();
    let (snap, _io) = parse_config_json(&json).unwrap();
    let mut scene = SceneState::new();
    scene.load_snapshot(snap, LoadMode::Replace);
    scene.animations.values().next().unwrap().state.clone()
}

#[test]
fn armed_animations_load_back_armed() {
    assert_eq!(state_after_roundtrip(AnimState::Armed), AnimState::Armed);
}

#[test]
fn running_animations_load_back_armed_not_mid_run() {
    assert_eq!(
        state_after_roundtrip(AnimState::Running { frame_counter: 17 }),
        AnimState::Armed,
        "a mid-run save must restart from the beginning, not resume"
    );
}

#[test]
fn idle_and_done_animations_load_back_idle() {
    assert_eq!(state_after_roundtrip(AnimState::Idle), AnimState::Idle);
    assert_eq!(state_after_roundtrip(AnimState::Done), AnimState::Idle);
}

/// The additive path applies the same mapping — it remaps handles, which is no
/// reason for an armed animation to come back idle.
#[test]
fn additive_load_preserves_armed_too() {
    use vstimd::scene::animation::{Animation, AnimationEntry};

    let mut saved = SceneState::new();
    let h = saved.add_stimulus(make_rect_entry());
    saved.add_animation(AnimationEntry::armed(
        Animation::FlashForNFrames {
            duration_frames: 30,
        },
        vec![h],
    ));
    let json = retrieve_config_json(&saved.config, &VtlConfig::default()).unwrap();
    let (snap, _io) = parse_config_json(&json).unwrap();

    let mut scene = SceneState::new();
    scene.add_stimulus(make_circle_entry());
    scene.load_snapshot(snap, LoadMode::Additive);

    let anim = scene.animations.values().next().unwrap();
    assert_eq!(anim.state, AnimState::Armed);
    assert!(
        scene.stimuli.contains_key(&anim.target.stimuli()[0]),
        "additive load left the animation pointing at a remapped-away stimulus"
    );
}

/// `FinalAction` widened to u16 for `DONE_LEVEL` (0x100). A mask above the old
/// u8 ceiling must survive save/load, as must the line the bit addresses —
/// truncation would silently turn a two-line animation back into a one-line one.
#[test]
fn wide_final_action_bits_and_the_level_line_round_trip() {
    use vstimd::scene::animation::{Animation, AnimationEntry, FinalAction};
    use vstimd::scene::VtlBit;

    let mut saved = SceneState::new();
    let h = saved.add_stimulus(make_rect_entry());
    saved.add_animation({
        let mut e = AnimationEntry::armed(
            Animation::FlashForNFrames { duration_frames: 30 },
            vec![h],
        );
        e.final_action = FinalAction::DISABLE
            | FinalAction::REARM
            | FinalAction::FINAL_ACTION_TRIGGER_LINE
            | FinalAction::DONE_LEVEL;
        e.final_action_trigger_line = Some(VtlBit { bank: 0, bit: 37, kind: VtlKind::Output });
        e.final_action_level_line = Some(VtlBit { bank: 0, bit: 35, kind: VtlKind::Output });
        e
    });
    assert!(saved.animations[&1].final_action.bits() > u8::MAX as u16);

    let json = retrieve_config_json(&saved.config, &VtlConfig::default()).unwrap();
    let (snap, _io) = parse_config_json(&json).unwrap();
    let mut scene = SceneState::new();
    scene.load_snapshot(snap, LoadMode::Replace);

    let anim = scene.animations.values().next().unwrap();
    assert!(anim.final_action.contains(FinalAction::DONE_LEVEL), "DONE_LEVEL was truncated away");
    assert!(anim.final_action.contains(FinalAction::REARM));
    assert_eq!(anim.final_action_level_line.unwrap().bit, 35);
    assert_eq!(anim.final_action_trigger_line.unwrap().bit, 37);
}

/// Configs written before `final_action_level_line` existed must still load —
/// the field is `#[serde(default)]`, and every shipped demo predates it.
#[test]
fn a_config_without_the_level_line_field_still_loads() {
    use vstimd::scene::animation::{Animation, AnimationEntry};

    let mut saved = SceneState::new();
    let h = saved.add_stimulus(make_rect_entry());
    saved.add_animation(AnimationEntry::armed(
        Animation::FlashForNFrames { duration_frames: 30 },
        vec![h],
    ));
    let json = retrieve_config_json(&saved.config, &VtlConfig::default()).unwrap();

    // Strip the key, as an older writer would have left it.
    let mut tree: serde_json::Value = serde_json::from_str(&json).unwrap();
    let anim = &mut tree["scene"]["animations"]["1"];
    assert!(anim.as_object_mut().unwrap().remove("final_action_level_line").is_some());
    let stripped = serde_json::to_string(&tree).unwrap();

    let (snap, _io) = parse_config_json(&stripped).expect("older config failed to load");
    let mut scene = SceneState::new();
    scene.load_snapshot(snap, LoadMode::Replace);
    assert!(scene.animations.values().next().unwrap().final_action_level_line.is_none());
}
