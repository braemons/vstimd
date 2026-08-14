//! The demo configs shipped in `DEMO_CONFIGS` are plain config files — nothing
//! parses them at build time, so a field renamed in the scene model would only
//! surface as a load error on a rig. These tests load every demo the same way
//! `config load` does, and assert the properties each demo is supposed to
//! demonstrate.

use vstimd::io_config::{parse_config_json, retrieve_config_json, DEMO_CONFIGS, DEMO_PREFIX};
use vstimd::scene::animation::{Animation, StartAction};
use vstimd::scene::{AnimState, FinalAction};

/// Find a demo by name, or panic with the list of names that do exist.
fn demo(name: &str) -> &'static str {
    DEMO_CONFIGS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, json)| *json)
        .unwrap_or_else(|| {
            let all: Vec<&str> = DEMO_CONFIGS.iter().map(|(n, _)| *n).collect();
            panic!("no demo named '{name}'; have {all:?}")
        })
}

#[test]
fn every_demo_parses() {
    for (name, json) in DEMO_CONFIGS {
        let (scene, io) = parse_config_json(json)
            .unwrap_or_else(|e| panic!("demo '{name}' failed to parse: {e}"));
        assert!(!scene.stimuli.is_empty(), "demo '{name}' has no stimuli");
        // Handles are allocated from `next_*_handle`; a stale value would hand
        // out a handle that is already taken.
        assert!(
            scene.stimuli.keys().all(|h| *h < scene.next_stim_handle),
            "demo '{name}': next_stim_handle {} does not clear its stimuli",
            scene.next_stim_handle
        );
        assert!(
            scene.animations.keys().all(|h| *h < scene.next_anim_handle),
            "demo '{name}': next_anim_handle {} does not clear its animations",
            scene.next_anim_handle
        );
        // Every animation must drive stimuli that exist in the same file.
        for (h, anim) in &scene.animations {
            for sh in &anim.stimuli {
                assert!(
                    scene.stimuli.contains_key(sh),
                    "demo '{name}': animation {h} drives unknown stimulus {sh}"
                );
            }
        }
        // Named VTL lines are what makes a demo self-describing in the UI.
        for entry in &io.vtl.names {
            assert!(!entry.name.is_empty(), "demo '{name}' has an unnamed VTL line");
        }
    }
}

/// A demo is meant to be self-explanatory on a rig with no client attached, so
/// every one of them puts what it does — and which pins drive it — on screen.
#[test]
fn every_demo_explains_itself_on_screen() {
    for (name, json) in DEMO_CONFIGS {
        let (scene, _) = parse_config_json(json).unwrap();
        let explanation = scene
            .stimuli
            .values()
            .find(|e| e.name.as_deref() == Some("explanation"))
            .unwrap_or_else(|| panic!("demo '{name}' has no 'explanation' stimulus"));
        let vstimd::scene::Stimulus::Text(t) = &explanation.stimulus else {
            panic!("demo '{name}': the explanation is not a text stimulus");
        };
        assert!(t.flags.enabled, "demo '{name}': the explanation is hidden");
        assert!(
            t.text_live.contains(name),
            "demo '{name}': the explanation does not name the demo"
        );
        assert!(
            t.text_live.lines().count() >= 3,
            "demo '{name}': the explanation is a one-liner"
        );
    }
}

#[test]
fn demo_names_are_prefixed_and_unique() {
    let mut names: Vec<&str> = DEMO_CONFIGS.iter().map(|(n, _)| *n).collect();
    for name in &names {
        assert!(
            name.starts_with(DEMO_PREFIX),
            "demo '{name}' does not start with '{DEMO_PREFIX}'"
        );
    }
    let before = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(before, names.len(), "duplicate demo names in DEMO_CONFIGS");
}

/// A hand-written demo file that silently loses a field on load (a typo'd or
/// renamed key is ignored by serde) would still parse. Re-serializing the
/// parsed scene and comparing the JSON trees catches that.
#[test]
fn demo_files_survive_a_reserialize_unchanged() {
    for (name, json) in DEMO_CONFIGS {
        let (scene, io) = parse_config_json(json).unwrap();
        let round = retrieve_config_json(&scene, &io.vtl).unwrap();
        let before: serde_json::Value = serde_json::from_str(json).unwrap();
        let after: serde_json::Value = serde_json::from_str(&round).unwrap();
        assert_eq!(
            before, after,
            "demo '{name}' does not round-trip — a key is misspelled, missing or unknown"
        );
    }
}

#[test]
fn gratings_demo_flashes_two_orientations_on_two_input_triggers() {
    let (scene, io) = parse_config_json(demo("demo_gratings_triggered")).unwrap();

    // Two centred gratings, both hidden until their trigger fires.
    let gratings: Vec<_> = scene
        .stimuli
        .values()
        .filter(|e| matches!(e.stimulus, vstimd::scene::Stimulus::Grating(_)))
        .collect();
    assert_eq!(gratings.len(), 2);
    for g in &gratings {
        assert!(!g.stimulus.flags().enabled, "grating starts visible");
        assert_eq!(g.stimulus.transform().live.pos, [0.0, 0.0], "grating is off-centre");
    }
    let mut angles: Vec<f32> = gratings.iter().map(|g| g.stimulus.transform().live.angle).collect();
    angles.sort_by(f32::total_cmp);
    assert_eq!(angles, vec![45.0, 135.0], "the two gratings share an orientation");

    assert_eq!(scene.animations.len(), 2);
    let mut start_bits = vec![];
    let mut done_bits = vec![];
    for anim in scene.animations.values() {
        assert_eq!(anim.state, AnimState::Armed, "animation is not armed to wait");
        // 2 s at the 60 Hz the Pi 5 rig runs at.
        assert!(
            matches!(anim.animation, Animation::FlashForNFrames { duration_frames: 120 }),
            "animation is not a 120-frame flash"
        );
        let (bit, edge) = anim.start_trigger.expect("no start trigger");
        assert_eq!(edge, vstimd::vtl_state::VtlEdge::Rising);
        assert_eq!(bit.kind, vtl::VtlKind::Input);
        start_bits.push(bit.bit);

        assert!(anim.start_action.contains(StartAction::ENABLE));
        assert!(anim.start_action.contains(StartAction::START_ACTION_TRIGGER_LINE));
        assert!(anim.start_action_trigger_line.is_some());
        assert!(anim.final_action.contains(FinalAction::DISABLE));
        assert!(anim.final_action.contains(FinalAction::FINAL_ACTION_TRIGGER_LINE));
        done_bits.push(anim.final_action_trigger_line.expect("no done trigger line").bit);
    }
    start_bits.sort_unstable();
    done_bits.sort_unstable();
    assert_eq!(start_bits, vec![11, 12], "the two flashes share an input trigger");
    assert_eq!(done_bits.len(), 2);
    assert_ne!(done_bits[0], done_bits[1], "the two flashes share a done line");

    // Every line the animations touch is named, so the overlay and clients show
    // pin names rather than bare bit numbers.
    for anim in scene.animations.values() {
        for line in [anim.start_action_trigger_line, anim.final_action_trigger_line] {
            let bit = line.unwrap();
            assert!(
                io.vtl.names.iter().any(|n| n.bit == bit.bit && n.kind == bit.kind),
                "output bit {} is unnamed",
                bit.bit
            );
        }
        let (bit, _) = anim.start_trigger.unwrap();
        assert!(
            io.vtl.names.iter().any(|n| n.bit == bit.bit && n.kind == bit.kind),
            "input bit {} is unnamed",
            bit.bit
        );
    }
}

/// Both the trigger-driven demos and the free-running ones have to come up
/// doing something without a client attached: an `Idle` animation never starts.
#[test]
fn no_demo_ships_an_idle_animation() {
    for (name, json) in DEMO_CONFIGS {
        let (scene, _) = parse_config_json(json).unwrap();
        for (h, anim) in &scene.animations {
            assert_eq!(
                anim.state,
                AnimState::Armed,
                "demo '{name}': animation {h} ('{}') is not armed",
                anim.name
            );
        }
    }
}

#[test]
fn moving_target_loops_and_pulses_an_output_each_pass() {
    let (scene, io) = parse_config_json(demo("demo_moving_target")).unwrap();
    let anim = scene.animations.values().next().expect("no animation");
    assert!(anim.start_trigger.is_none(), "the sweep should not wait for a trigger");
    assert!(matches!(anim.animation, Animation::MoveAlongSegments2D { .. }));
    assert!(anim.final_action.contains(FinalAction::RESTART), "the sweep does not loop");
    assert!(anim.final_action.contains(FinalAction::FINAL_ACTION_TRIGGER_LINE));
    let bit = anim.final_action_trigger_line.unwrap();
    assert_eq!(bit.kind, vtl::VtlKind::Output);
    assert!(io.vtl.names.iter().any(|n| n.bit == bit.bit && n.kind == bit.kind));
}

#[test]
fn trigger_gate_follows_an_input_level() {
    let (scene, io) = parse_config_json(demo("demo_trigger_gate")).unwrap();
    let anim = scene.animations.values().next().expect("no animation");
    let Animation::CoupleVisibilityToTriggerLine { trigger, polarity } = anim.animation else {
        panic!("demo_trigger_gate is not level-coupled");
    };
    assert!(polarity, "HIGH should show the stimulus");
    assert_eq!(trigger.kind, vtl::VtlKind::Input);
    assert!(io.vtl.names.iter().any(|n| n.bit == trigger.bit && n.kind == trigger.kind));
}

#[test]
fn photodiode_demo_enables_the_patch() {
    let (scene, _) = parse_config_json(demo("demo_photodiode_flicker")).unwrap();
    assert!(scene.photodiode.enabled, "photodiode patch is off");
    assert!(scene.photodiode.flicker, "photodiode patch does not flicker");
    let anim = scene.animations.values().next().expect("no animation");
    assert!(
        matches!(anim.animation, Animation::FlickerForNFrames { total_frames: None, .. }),
        "the field flicker should run until stopped"
    );
}

#[test]
fn drifting_grating_demo_drifts() {
    let (scene, _) = parse_config_json(demo("demo_drifting_grating")).unwrap();
    let g = scene
        .stimuli
        .values()
        .find_map(|e| match &e.stimulus {
            vstimd::scene::Stimulus::Grating(g) => Some(g),
            _ => None,
        })
        .expect("no grating");
    assert!(g.flags.enabled, "the grating starts hidden");
    assert!(g.params.live.drift_speed > 0.0, "the grating does not drift");
}

#[test]
fn seeding_writes_every_demo_and_never_overwrites() {
    let dir = std::env::temp_dir().join(format!("vstimd-demo-seed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let (written, failed) = vstimd::io_config::seed_demo_configs(&dir);
    assert!(failed.is_empty(), "seed errors: {failed:?}");
    assert_eq!(written.len(), DEMO_CONFIGS.len());
    let listed = vstimd::io_config::list_config_names(&dir).unwrap();
    for (name, _) in DEMO_CONFIGS {
        assert!(listed.contains(&name.to_string()), "'{name}' is not listed after seeding");
    }

    // An operator's edit survives the next start.
    let edited = vstimd::io_config::config_path(&dir, DEMO_CONFIGS[0].0);
    std::fs::write(&edited, "edited by the operator").unwrap();
    let (written, failed) = vstimd::io_config::seed_demo_configs(&dir);
    assert!(written.is_empty() && failed.is_empty());
    assert_eq!(std::fs::read_to_string(&edited).unwrap(), "edited by the operator");

    std::fs::remove_dir_all(&dir).unwrap();
}
