//! The demo configs shipped in `DEMO_CONFIGS` are plain config files — nothing
//! parses them at build time, so a field renamed in the scene model would only
//! surface as a load error on a rig. These tests load every demo the same way
//! `config load` does, and assert the properties each demo is supposed to
//! demonstrate.

use vstimd::scene_config_file::{
    parse_config_json, retrieve_config_json, DemoSkip, DEMO_CONFIGS, DEMO_PREFIX,
};
use vstimd::scene::animation::{Animation, StartAction};
use vstimd::scene::{AnimState, FinalAction};
use vstimd::vtl_state::VtlPolarity;

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

/// A byte-different but semantically identical stand-in for "the version of
/// this demo an older server shipped". Only the whitespace around the config
/// version changes, so the test does not have to be edited whenever
/// `CONFIG_VERSION` is bumped.
fn older_stand_in(shipped: &str) -> String {
    let from = format!("\"version\": {}", vstimd::scene_config_file::CONFIG_VERSION);
    let to = format!("\"version\":  {}", vstimd::scene_config_file::CONFIG_VERSION);
    let older = shipped.replace(&from, &to);
    assert_ne!(older, shipped, "the stand-in for an older demo is not different");
    older
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
            for sh in anim.target.stimuli() {
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
            .find(|e| e.name() == "explanation")
            .unwrap_or_else(|| panic!("demo '{name}' has no 'explanation' stimulus"));
        let Some(t) = explanation.stimulus.text() else {
            panic!("demo '{name}': the explanation is not a text stimulus");
        };
        assert!(
            explanation.stimulus.flags().enabled,
            "demo '{name}': the explanation is hidden"
        );
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
        .filter(|e| e.stimulus.grating().is_some())
        .collect();
    assert_eq!(gratings.len(), 2);
    for g in &gratings {
        assert!(!g.stimulus.flags().enabled, "grating starts visible");
        assert_eq!(g.stimulus.transform2d().expect("2-D stimulus").live.pos_px, [0.0, 0.0], "grating is off-centre");
    }
    let mut angles: Vec<f32> = gratings.iter().map(|g| g.stimulus.transform2d().expect("2-D stimulus").live.angle_deg).collect();
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
        assert!(
            anim.final_action.contains(FinalAction::REARM),
            "the flash would fire only once per session without REARM"
        );
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
    assert_eq!(
        polarity,
        VtlPolarity::ActiveHigh,
        "HIGH should show the stimulus"
    );
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
    let entry = scene
        .stimuli
        .values()
        .find(|e| e.stimulus.grating().is_some())
        .expect("no grating");
    let g = entry.stimulus.grating().expect("checked above");
    assert!(entry.stimulus.flags().enabled, "the grating starts hidden");
    assert!(g.params.live.drift_speed_hz > 0.0, "the grating does not drift");
}

/// End-to-end for the demo that motivated all this: load the file the way
/// `config load` does, fire the input edge the way the overlay's ↑ button does,
/// and check the grating actually appears, that the onset line pulses, and that
/// 2 s later it disappears and the done line pulses.
///
/// This is the test that catches a load path which drops the config's animation
/// state — an armed animation that loads as `Idle` never observes its trigger,
/// so the demo looks dead while every value in the file is correct.
#[test]
fn loading_the_gratings_demo_leaves_it_waiting_for_its_triggers() {
    use vstimd::scene::{LoadMode, SceneState};
    use vstimd::vtl_state::VtlEdges;

    let (cfg, _io) = parse_config_json(demo("demo_gratings_triggered")).unwrap();
    let mut scene = SceneState::new();
    scene.load_snapshot(cfg, LoadMode::Replace);

    // Handle 1 is the 45° grating, driven by in_pin11 (bank 0, bit 11); its
    // onset line is bit 36 and its done line bit 37.
    let enabled = |s: &SceneState| s.stimuli.get(&1).unwrap().stimulus.flags().enabled;
    assert!(!enabled(&scene), "the grating should start hidden");

    let mut out;
    let no_edges = VtlEdges::default();
    // The demo's marks are one-frame pulses, so watch that channel.
    let advance = |scene: &mut SceneState, edges: &VtlEdges| {
        let mut levels = [0u64; vtl::MAX_BANKS];
        let mut pulses = [0u64; vtl::MAX_BANKS];
        scene.advance_animations(
            edges,
            &VtlEdges::default(),
            &mut vstimd::vtl_state::VtlOutputs { levels: &mut levels, pulses: &mut pulses },
        );
        pulses
    };

    // Idle frames change nothing: the animation is waiting, not running.
    for _ in 0..3 {
        out = advance(&mut scene, &no_edges);
        assert!(!enabled(&scene), "the grating appeared without a trigger");
        assert_eq!(out[0], 0, "an output pulsed without a trigger");
    }

    // Rising edge on in_pin11 — what the overlay's ↑ button latches.
    let mut edges = VtlEdges::default();
    edges.rising[0] = 1 << 11;
    edges.current[0] = 1 << 11;
    out = advance(&mut scene, &edges);
    assert!(enabled(&scene), "the trigger did not show the grating");
    assert_eq!(out[0] & (1 << 36), 1 << 36, "the onset line did not pulse");
    assert_eq!(out[0] & (1 << 37), 0, "the done line pulsed at onset");

    // The other grating is untouched — the two triggers are independent.
    assert!(
        !scene.stimuli.get(&2).unwrap().stimulus.flags().enabled,
        "in_pin11 also fired the 135° grating"
    );

    // 120 frames total; frame 0 was the trigger frame, so 119 more end it.
    for _ in 0..118 {
        advance(&mut scene, &no_edges);
        assert!(enabled(&scene), "the grating vanished before 2 s were up");
    }
    out = advance(&mut scene, &no_edges);
    assert!(!enabled(&scene), "the grating was still visible after 120 frames");
    assert_eq!(out[0] & (1 << 37), 1 << 37, "the done line did not pulse");

    // …and it re-arms, so the next edge fires another presentation. Without
    // this the demo works exactly once per session.
    assert_eq!(
        scene.animations.get(&1).unwrap().state,
        AnimState::Armed,
        "the flash did not re-arm after completing"
    );
    out = advance(&mut scene, &edges);
    assert!(enabled(&scene), "the second trigger did not show the grating");
    assert_eq!(out[0] & (1 << 36), 1 << 36, "the onset line did not pulse again");
}

/// The demos that carry no `start_trigger` are supposed to run the moment they
/// are loaded — the same load-path state loss would leave them frozen too.
#[test]
fn loading_the_moving_target_demo_starts_it_moving() {
    use vstimd::scene::{LoadMode, SceneState};
    use vstimd::vtl_state::VtlEdges;

    let (cfg, _io) = parse_config_json(demo("demo_moving_target")).unwrap();
    let mut scene = SceneState::new();
    scene.load_snapshot(cfg, LoadMode::Replace);

    let pos_px = |s: &SceneState| s.stimuli.get(&1).unwrap().stimulus.transform2d().expect("2-D stimulus").live.pos_px;
    let start = pos_px(&scene);

    let no_edges = VtlEdges::default();
    let mut levels = [0u64; vtl::MAX_BANKS];
    let mut pulses = [0u64; vtl::MAX_BANKS];
    for _ in 0..10 {
        scene.advance_animations(
            &no_edges,
            &no_edges,
            &mut vstimd::vtl_state::VtlOutputs { levels: &mut levels, pulses: &mut pulses },
        );
    }
    assert_ne!(pos_px(&scene), start, "the target never started moving");
}

/// A scratch config dir for a seeding test. Per-test name so the cases can run
/// in parallel without sharing a directory.
fn seed_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vstimd-demo-seed-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Record `content` as the version the server installed for `name`, mirroring
/// what `seed_demo_configs` writes into its sidecar. Lets a test stand in for
/// "an older release wrote this file".
fn stamp_as_installed(dir: &std::path::Path, name: &str, content: &str) {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in content.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let path = dir.join(".vstimd_demo_seed");
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with(&format!("{name} ")))
        .map(str::to_string)
        .collect();
    lines.push(format!("{name} {h}"));
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
}

#[test]
fn seeding_writes_every_demo_and_never_overwrites() {
    let dir = seed_dir("basic");

    let report = vstimd::scene_config_file::seed_demo_configs(&dir);
    assert!(report.failed.is_empty(), "seed errors: {:?}", report.failed);
    assert_eq!(report.installed.len(), DEMO_CONFIGS.len());
    assert!(report.refreshed.is_empty(), "nothing existed to refresh");
    let listed = vstimd::scene_config_file::list_config_names(&dir).unwrap();
    for (name, _) in DEMO_CONFIGS {
        assert!(listed.contains(&name.to_string()), "'{name}' is not listed after seeding");
    }

    // Seeding again is a no-op: nothing is rewritten, nothing errors, and every
    // demo is reported as already up to date.
    let report = vstimd::scene_config_file::seed_demo_configs(&dir);
    assert!(
        report.installed.is_empty() && report.refreshed.is_empty() && report.failed.is_empty(),
        "re-seeding rewrote files"
    );
    assert_eq!(report.kept.len(), DEMO_CONFIGS.len());
    assert!(
        report.kept.iter().all(|(_, why)| *why == DemoSkip::UpToDate),
        "an untouched demo was not reported as up to date"
    );

    // An operator's edit survives the next start.
    let edited = vstimd::scene_config_file::config_path(&dir, DEMO_CONFIGS[0].0);
    std::fs::write(&edited, "edited by the operator").unwrap();
    let report = vstimd::scene_config_file::seed_demo_configs(&dir);
    assert!(report.installed.is_empty() && report.refreshed.is_empty() && report.failed.is_empty());
    assert_eq!(std::fs::read_to_string(&edited).unwrap(), "edited by the operator");
    assert!(
        report.kept.contains(&(DEMO_CONFIGS[0].0, DemoSkip::Modified)),
        "an edited demo was not reported as kept: {:?}",
        report.kept
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// A demo the server installed and the operator never touched must be replaced
/// when the shipped version changes — otherwise a fixed demo never reaches a rig
/// that already has the old file, and it silently keeps behaving like the old
/// version. This is what happened when REARM was added to the gratings demo.
#[test]
fn seeding_refreshes_an_untouched_demo_that_changed_upstream() {
    let (name, shipped) = DEMO_CONFIGS[0];
    let dir = seed_dir("refresh");
    let path = vstimd::scene_config_file::config_path(&dir, name);

    vstimd::scene_config_file::seed_demo_configs(&dir);

    // Simulate "the server shipped an older version of this demo": rewrite the
    // file with different content and stamp it as ours.
    let older = older_stand_in(shipped);
    std::fs::write(&path, &older).unwrap();
    stamp_as_installed(&dir, name, &older);

    let report = vstimd::scene_config_file::seed_demo_configs(&dir);
    assert!(report.failed.is_empty(), "seed errors: {:?}", report.failed);
    assert!(
        report.refreshed.contains(&name),
        "'{name}' was not reported as refreshed: {report:?}"
    );
    assert!(
        !report.installed.contains(&name),
        "a replaced file was reported as newly installed — the log would say the \
         wrong thing to an operator"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        shipped,
        "the refreshed file is not the shipped version"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The refresh must not reach files the server never wrote — a config dir that
/// predates the stamp sidecar is all operator content as far as we know.
#[test]
fn seeding_leaves_an_unstamped_file_alone() {
    let (name, _) = DEMO_CONFIGS[0];
    let dir = seed_dir("unstamped");
    let path = vstimd::scene_config_file::config_path(&dir, name);
    std::fs::write(&path, "someone else's file").unwrap();

    let report = vstimd::scene_config_file::seed_demo_configs(&dir);
    assert!(report.failed.is_empty(), "seed errors: {:?}", report.failed);
    assert!(
        !report.installed.contains(&name) && !report.refreshed.contains(&name),
        "an unstamped file was overwritten"
    );
    assert!(
        report.kept.contains(&(name, DemoSkip::Unstamped)),
        "an unstamped file was not reported as such: {:?}",
        report.kept
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "someone else's file");

    std::fs::remove_dir_all(&dir).unwrap();
}

/// Restoring a demo by hand from the shipped copy leaves it eligible for the
/// next refresh, even though this server never wrote that particular file.
#[test]
fn seeding_adopts_a_file_identical_to_the_shipped_one() {
    let (name, shipped) = DEMO_CONFIGS[0];
    let dir = seed_dir("adopt");
    let path = vstimd::scene_config_file::config_path(&dir, name);
    std::fs::write(&path, shipped).unwrap();

    let report = vstimd::scene_config_file::seed_demo_configs(&dir);
    assert!(
        report.kept.contains(&(name, DemoSkip::UpToDate)),
        "an identical file was not adopted as up to date: {:?}",
        report.kept
    );

    // Now that it is stamped, an upstream change reaches it.
    let older = older_stand_in(shipped);
    std::fs::write(&path, &older).unwrap();
    stamp_as_installed(&dir, name, &older);
    let report = vstimd::scene_config_file::seed_demo_configs(&dir);
    assert!(report.refreshed.contains(&name), "the adopted file was not refreshed");

    std::fs::remove_dir_all(&dir).unwrap();
}
