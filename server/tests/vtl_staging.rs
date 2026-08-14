/// Tests for VtlState output behaviour, which has two channels:
///
/// * `staged` — levels. Never reset between frames: ZMQ writes via
///   `set_staged_bit`/`set_staged_bank`, and the animation `DONE_LEVEL` action,
///   persist until something clears them.
/// * `pulses` — one-frame event marks. An animation's start/final/cancel trigger
///   line lands here, is published by the next `commit_staged`, and falls LOW
///   immediately after, so every occurrence produces its own edge.
use vstimd::scene::{
    SceneState,
    animation::{Animation, AnimationEntry, FinalAction, StartAction},
};
use vstimd::vtl_state::{VtlBit, VtlEdges, VtlOutputs};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn no_edges() -> VtlEdges {
    VtlEdges::default()
}

fn bit(bank: usize, b: u8) -> VtlBit {
    VtlBit { bank, bit: b, kind: vtl::VtlKind::Output }
}

/// Simulate the render loop's copy-advance-writeback pattern for the level
/// channel. Pulses are dropped, as the real commit does one frame later.
fn advance_staged(
    scene: &mut SceneState,
    staged: &mut [u64; vtl::MAX_BANKS],
) {
    let mut pulses = [0u64; vtl::MAX_BANKS];
    scene.advance_animations(
        &no_edges(),
        &VtlEdges::default(),
        &mut VtlOutputs { levels: staged, pulses: &mut pulses },
    );
}

/// Advance one frame and return just the pulses raised during it.
fn advance_pulses(
    scene: &mut SceneState,
    staged: &mut [u64; vtl::MAX_BANKS],
) -> [u64; vtl::MAX_BANKS] {
    let mut pulses = [0u64; vtl::MAX_BANKS];
    scene.advance_animations(
        &no_edges(),
        &VtlEdges::default(),
        &mut VtlOutputs { levels: staged, pulses: &mut pulses },
    );
    pulses
}

// ── VtlState::set_staged_bit / set_staged_bank ────────────────────────────────

#[cfg(unix)]
mod vtl_state_tests {
    use super::*;
    use vstimd::vtl_state::VtlState;
    use vtl::VtlOwner;

    fn unique_shm_name() -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        std::thread::current().id().hash(&mut h);
        format!("/vtl_staging_test_{}_{:x}", std::process::id(), h.finish())
    }

    fn make_vtl() -> VtlState {
        let owner = VtlOwner::create(&unique_shm_name(), 1, 1).expect("VtlOwner::create");
        VtlState::new(owner)
    }

    #[test]
    fn set_staged_bit_persists_in_shm_immediately() {
        let mut vtl = make_vtl();
        vtl.set_staged_bit(0, 5, true);
        assert_ne!(vtl.owner().output_state(0) & (1u64 << 5), 0,
            "shm reflects the write immediately");
        assert_ne!(vtl.staged[0] & (1u64 << 5), 0,
            "staged reflects the write");
    }

    #[test]
    fn set_staged_bit_clear_removes_bit() {
        let mut vtl = make_vtl();
        vtl.set_staged_bit(0, 3, true);
        vtl.set_staged_bit(0, 3, false);
        assert_eq!(vtl.staged[0] & (1u64 << 3), 0, "bit cleared in staged");
        assert_eq!(vtl.owner().output_state(0) & (1u64 << 3), 0, "bit cleared in shm");
    }

    #[test]
    fn set_staged_bank_writes_full_word() {
        let mut vtl = make_vtl();
        vtl.set_staged_bank(0, 0b1100_0011);
        assert_eq!(vtl.staged[0], 0b1100_0011);
        assert_eq!(vtl.owner().output_state(0), 0b1100_0011);
    }

    #[test]
    fn commit_staged_writes_to_shm() {
        let mut vtl = make_vtl();
        vtl.staged[0] = 0xAB;
        vtl.commit_staged();
        assert_eq!(vtl.owner().output_state(0), 0xAB,
            "commit_staged writes staged to shm");
    }

    /// The crux of the pulse channel, at the shm boundary the DAQ actually
    /// reads: a pulse is published by one commit and gone by the next, so the
    /// pin goes HIGH for one frame period and falls on its own.
    #[test]
    fn commit_staged_publishes_pulses_then_drops_them() {
        let mut vtl = make_vtl();
        vtl.staged[0] = 1 << 1; // a level, held across commits
        vtl.pulses[0] = 1 << 2; // a mark, this frame only

        vtl.commit_staged();
        assert_eq!(
            vtl.owner().output_state(0),
            (1 << 1) | (1 << 2),
            "commit must publish the level and the pulse together"
        );
        assert_eq!(vtl.pulses[0], 0, "pulses must be consumed by the commit");

        vtl.commit_staged();
        assert_eq!(
            vtl.owner().output_state(0),
            1 << 1,
            "the pulse must fall LOW one frame later, leaving the level up"
        );
    }

    /// The write-back after an animation pass must not clobber a pulse another
    /// thread added while the VTL lock was released — the ZMQ thread firing a
    /// cancel-action line, say. A dropped pulse is a missing event mark in the
    /// recording, invisible until someone analyses the data.
    #[test]
    fn storing_frame_outputs_merges_pulses_added_meanwhile() {
        let mut vtl = make_vtl();

        // Render thread copies the (empty) channels out...
        let levels = vtl.staged;
        let mut pulses = vtl.pulses;
        // ...another thread fires a cancel-action pulse on bit 9 in the window
        // where the VTL lock is free...
        vtl.pulses[0] |= 1 << 9;
        // ...and the animation pass produces its own on bit 4.
        pulses[0] |= 1 << 4;

        vtl.store_frame_outputs(levels, pulses);

        assert_ne!(vtl.pulses[0] & (1 << 4), 0, "the animation's own pulse was lost");
        assert_ne!(
            vtl.pulses[0] & (1 << 9),
            0,
            "a pulse added while the lock was released was clobbered"
        );
    }

    /// Levels are not merged: the animation pass starts from the current staged
    /// value and owns the result, so a bit it cleared must stay cleared.
    #[test]
    fn storing_frame_outputs_lets_the_pass_clear_a_level() {
        let mut vtl = make_vtl();
        vtl.staged[0] = 1 << 5;

        let mut levels = vtl.staged;
        levels[0] &= !(1 << 5); // the pass cleared it (e.g. DONE_LEVEL reset)
        vtl.store_frame_outputs(levels, [0; vtl::MAX_BANKS]);

        assert_eq!(vtl.staged[0] & (1 << 5), 0, "a cleared level came back");
    }

    /// A pulse and a level on the *same* bit: the level wins once the pulse is
    /// spent, rather than the pulse's clear taking the level down with it.
    #[test]
    fn a_pulse_on_a_held_line_does_not_clear_the_level() {
        let mut vtl = make_vtl();
        vtl.staged[0] = 1 << 3;
        vtl.pulses[0] = 1 << 3;

        vtl.commit_staged();
        assert_ne!(vtl.owner().output_state(0) & (1 << 3), 0);
        vtl.commit_staged();
        assert_ne!(
            vtl.owner().output_state(0) & (1 << 3),
            0,
            "the level was dropped when the pulse expired"
        );
    }

    #[test]
    fn staged_survives_advance_animations_cycle() {
        let mut vtl = make_vtl();
        let mut scene = SceneState::new();

        vtl.set_staged_bit(0, 7, true);

        // Simulate three render loop iterations: copy out, advance, write back.
        for _ in 0..3 {
            let mut staged = vtl.staged;
            advance_staged(&mut scene, &mut staged);
            vtl.staged = staged;
        }

        assert_ne!(vtl.staged[0] & (1u64 << 7), 0,
            "ZMQ-set bit persists across advance_animations cycles");
    }

    // ── Intra-server animation chaining (full render-loop pipeline) ───────────
    //
    // These drive the exact [A]/[S] sequence the render backends run each frame
    // (commit_staged → poll → output_edges → advance → write staged back), so
    // they exercise the real output-edge path rather than synthetic edges. This
    // is where the deterministic one-frame reaction of animation-to-animation
    // chaining is proven.

    use vstimd::scene::animation::{AnimState, CancelAction};
    use vstimd::vtl_state::VtlEdge;

    /// One render-loop iteration, mirroring null_backend/drm/winit exactly.
    fn frame(vtl: &mut VtlState, scene: &mut SceneState) {
        vtl.commit_staged();
        let input_edges = vtl.poll();
        let output_edges = vtl.output_edges();
        let mut levels = vtl.staged;
        let mut pulses = vtl.pulses;
        scene.advance_animations(
            &input_edges,
            &output_edges,
            &mut VtlOutputs { levels: &mut levels, pulses: &mut pulses },
        );
        vtl.staged = levels;
        vtl.pulses = pulses;
    }

    fn state(scene: &SceneState, h: u32) -> &AnimState {
        &scene.animations[&h].state
    }

    #[test]
    fn output_edge_chaining_deterministic_one_frame_reaction() {
        // A: 2-frame flash that pulses output bit (0,5) when it completes.
        // B: armed, starts on a rising edge of that OUTPUT line.
        // Expected timeline (the "clean handoff" from vtl_state.rs docs):
        //   frame 0: A Armed→Running; B stays Armed (no output edge yet).
        //   frame 1: A completes, writes bit 5 into staged; B still Armed
        //            (A's mid-pass write is NOT visible as an edge this frame).
        //   frame 2: commit raises bit 5 in shm; output_edges sees the rising
        //            edge; B fires → Running. Exactly one frame after A finished.
        let mut vtl = make_vtl();
        let mut scene = SceneState::new();

        let a = scene.add_animation({
            let mut e = AnimationEntry::armed(
                Animation::FlashForNFrames { duration_frames: 2 },
                vec![],
            );
            e.final_action = FinalAction::FINAL_ACTION_TRIGGER_LINE;
            e.final_action_trigger_line = Some(bit(0, 5));
            e
        });
        let b = scene.add_animation({
            let mut e = AnimationEntry::armed(
                Animation::FlashForNFrames { duration_frames: 3 },
                vec![],
            );
            e.start_trigger = Some((bit(0, 5), VtlEdge::Rising));
            e
        });

        frame(&mut vtl, &mut scene); // frame 0
        assert!(matches!(state(&scene, a), AnimState::Running { .. }), "A running");
        assert_eq!(state(&scene, b), &AnimState::Armed, "B waits");

        frame(&mut vtl, &mut scene); // frame 1: A done, pulses bit 5
        assert_eq!(state(&scene, a), &AnimState::Done, "A done on frame 1");
        assert_ne!(vtl.pulses[0] & (1 << 5), 0, "A pulsed output bit 5");
        assert_eq!(state(&scene, b), &AnimState::Armed,
            "B not started same frame A wrote the bit (no zero-frame cascade)");

        frame(&mut vtl, &mut scene); // frame 2: B sees the committed edge
        assert!(matches!(state(&scene, b), AnimState::Running { .. }),
            "B starts exactly one frame after A's output pulse");
    }

    #[test]
    fn output_edge_chaining_is_iteration_order_independent() {
        // Same as above but B is inserted BEFORE A, so it is likely advanced
        // first within a frame. Because B reads the pre-pass output-edge
        // snapshot (not A's in-progress staged write), the result is identical.
        let mut vtl = make_vtl();
        let mut scene = SceneState::new();

        let b = scene.add_animation({
            let mut e = AnimationEntry::armed(
                Animation::FlashForNFrames { duration_frames: 3 },
                vec![],
            );
            e.start_trigger = Some((bit(0, 6), VtlEdge::Rising));
            e
        });
        let a = scene.add_animation({
            let mut e = AnimationEntry::armed(
                Animation::FlashForNFrames { duration_frames: 2 },
                vec![],
            );
            e.final_action = FinalAction::FINAL_ACTION_TRIGGER_LINE;
            e.final_action_trigger_line = Some(bit(0, 6));
            e
        });

        frame(&mut vtl, &mut scene); // 0
        frame(&mut vtl, &mut scene); // 1: A done, bit set; B still armed
        assert_eq!(state(&scene, a), &AnimState::Done);
        assert_eq!(state(&scene, b), &AnimState::Armed,
            "insertion order does not leak A's mid-pass write to B");
        frame(&mut vtl, &mut scene); // 2
        assert!(matches!(state(&scene, b), AnimState::Running { .. }),
            "B still fires one frame later regardless of iteration order");
    }

    #[test]
    fn output_edge_cancels_running_animation() {
        // A completes and pulses output bit (0,7); B is long-running with a
        // cancel_trigger on that OUTPUT edge → B is cancelled one frame later.
        let mut vtl = make_vtl();
        let mut scene = SceneState::new();

        let a = scene.add_animation({
            let mut e = AnimationEntry::armed(
                Animation::FlashForNFrames { duration_frames: 1 },
                vec![],
            );
            e.final_action = FinalAction::FINAL_ACTION_TRIGGER_LINE;
            e.final_action_trigger_line = Some(bit(0, 7));
            e
        });
        let b = scene.add_animation({
            let mut e = AnimationEntry::armed(
                Animation::FlashForNFrames { duration_frames: 1000 },
                vec![],
            );
            e.cancel_trigger = Some((bit(0, 7), VtlEdge::Rising));
            e.cancel_action = CancelAction::DISABLE;
            e
        });

        frame(&mut vtl, &mut scene); // 0: A done (dur 1), pulses bit 7; B running
        assert_eq!(state(&scene, a), &AnimState::Done);
        assert!(matches!(state(&scene, b), AnimState::Running { .. }), "B running");
        assert_ne!(vtl.pulses[0] & (1 << 7), 0, "A pulsed output bit 7");

        frame(&mut vtl, &mut scene); // 1: B sees the output edge → cancelled
        assert_eq!(state(&scene, b), &AnimState::Done, "B cancelled by A's output edge");
    }

    #[test]
    fn output_edge_fan_out_starts_multiple_animations() {
        // One output edge (bit 0,8) starts two armed animations at once.
        let mut vtl = make_vtl();
        let mut scene = SceneState::new();

        let a = scene.add_animation({
            let mut e = AnimationEntry::armed(
                Animation::FlashForNFrames { duration_frames: 1 },
                vec![],
            );
            e.final_action = FinalAction::FINAL_ACTION_TRIGGER_LINE;
            e.final_action_trigger_line = Some(bit(0, 8));
            e
        });
        let mk_follower = |scene: &mut SceneState| {
            scene.add_animation({
                let mut e = AnimationEntry::armed(
                    Animation::FlashForNFrames { duration_frames: 3 },
                    vec![],
                );
                e.start_trigger = Some((bit(0, 8), VtlEdge::Rising));
                e
            })
        };
        let b = mk_follower(&mut scene);
        let c = mk_follower(&mut scene);

        frame(&mut vtl, &mut scene); // 0: A done, writes bit 8
        assert_eq!(state(&scene, a), &AnimState::Done);
        frame(&mut vtl, &mut scene); // 1: both B and C see the edge
        assert!(matches!(state(&scene, b), AnimState::Running { .. }), "B started");
        assert!(matches!(state(&scene, c), AnimState::Running { .. }), "C started");
    }
}

// ── Animation trigger lines preserve staged state ────────────────────────────

#[test]
fn start_trigger_line_pulses_for_one_frame_only() {
    let mut scene = SceneState::new();
    let mut staged = [0u64; vtl::MAX_BANKS];

    let _a = scene.add_animation({
        let mut e = AnimationEntry::armed(
            Animation::FlashForNFrames { duration_frames: 3 },
            vec![],
        );
        e.start_action = StartAction::START_ACTION_TRIGGER_LINE;
        e.start_action_trigger_line = Some(bit(0, 4));
        e
    });

    // Frame 0: Armed → Running, start trigger fires.
    let pulses = advance_pulses(&mut scene, &mut staged);
    assert_ne!(pulses[0] & (1u64 << 4), 0, "start trigger did not pulse on frame 0");
    assert_eq!(staged[0] & (1u64 << 4), 0, "a trigger-line pulse must not become a level");

    // Frames 1-2: still running, but the mark is over. A line that stayed HIGH
    // here would give a recording system one edge per session instead of one
    // per occurrence.
    for frame in 1..=2 {
        let pulses = advance_pulses(&mut scene, &mut staged);
        assert_eq!(pulses[0] & (1u64 << 4), 0, "start trigger re-pulsed on frame {frame}");
        assert_eq!(staged[0] & (1u64 << 4), 0, "start trigger leaked into levels");
    }
}

#[test]
fn final_trigger_line_pulses_for_one_frame_only() {
    let mut scene = SceneState::new();
    let mut staged = [0u64; vtl::MAX_BANKS];

    let _a = scene.add_animation({
        let mut e = AnimationEntry::armed(
            Animation::FlashForNFrames { duration_frames: 1 },
            vec![],
        );
        e.final_action = FinalAction::FINAL_ACTION_TRIGGER_LINE;
        e.final_action_trigger_line = Some(bit(0, 2));
        e
    });

    // Frame 0: done on the first advance, final trigger fires.
    let pulses = advance_pulses(&mut scene, &mut staged);
    assert_ne!(pulses[0] & (1u64 << 2), 0, "final trigger did not pulse");
    assert_eq!(staged[0] & (1u64 << 2), 0, "a trigger-line pulse must not become a level");

    // Frame 1: the animation is finished and the line is quiet again.
    let pulses = advance_pulses(&mut scene, &mut staged);
    assert_eq!(pulses[0] & (1u64 << 2), 0, "final trigger pulsed again after completion");
}

/// The other half of the pair: DONE_LEVEL is the sticky answer to "has it
/// finished?", and it clears when the animation next starts so each run answers
/// for itself.
#[test]
fn done_level_holds_until_the_animation_starts_again() {
    let mut scene = SceneState::new();
    let mut staged = [0u64; vtl::MAX_BANKS];
    let level = 1u64 << 6;

    let a = scene.add_animation({
        let mut e = AnimationEntry::armed(
            Animation::FlashForNFrames { duration_frames: 2 },
            vec![],
        );
        e.final_action = FinalAction::DONE_LEVEL;
        e.final_action_level_line = Some(bit(0, 6));
        e
    });

    // Running: not finished yet, so the level stays LOW.
    advance_staged(&mut scene, &mut staged);
    assert_eq!(staged[0] & level, 0, "level went HIGH before completion");

    // Completion raises it, and it stays up across later frames.
    advance_staged(&mut scene, &mut staged);
    assert_ne!(staged[0] & level, 0, "level did not go HIGH on completion");
    advance_staged(&mut scene, &mut staged);
    assert_ne!(staged[0] & level, 0, "level did not hold");

    // Arming and starting again clears it: the answer is about this run.
    scene.arm_animation(a);
    advance_staged(&mut scene, &mut staged);
    assert_eq!(staged[0] & level, 0, "level was not cleared when the animation restarted");

    advance_staged(&mut scene, &mut staged);
    assert_ne!(staged[0] & level, 0, "level did not go HIGH again on the second completion");
}

/// Both channels at once: the pulse marks the instant, the level answers later.
#[test]
fn pulse_and_level_can_be_used_together_on_separate_lines() {
    let mut scene = SceneState::new();
    let mut staged = [0u64; vtl::MAX_BANKS];

    let _a = scene.add_animation({
        let mut e = AnimationEntry::armed(
            Animation::FlashForNFrames { duration_frames: 1 },
            vec![],
        );
        e.final_action = FinalAction::FINAL_ACTION_TRIGGER_LINE | FinalAction::DONE_LEVEL;
        e.final_action_trigger_line = Some(bit(0, 2));
        e.final_action_level_line = Some(bit(0, 6));
        e
    });

    let pulses = advance_pulses(&mut scene, &mut staged);
    assert_ne!(pulses[0] & (1u64 << 2), 0, "the mark did not pulse");
    assert_ne!(staged[0] & (1u64 << 6), 0, "the level did not go HIGH");

    let pulses = advance_pulses(&mut scene, &mut staged);
    assert_eq!(pulses[0] & (1u64 << 2), 0, "the mark did not fall LOW");
    assert_ne!(staged[0] & (1u64 << 6), 0, "the level did not hold");
}

#[test]
fn staged_bit_from_earlier_frame_not_overwritten_by_later_frame_with_no_animation() {
    let mut scene = SceneState::new();
    let mut staged = [0u64; vtl::MAX_BANKS];

    // Manually set a bit (simulating a ZMQ override).
    staged[0] |= 1u64 << 10;

    // Run several frames with no animations active.
    for _ in 0..5 {
        advance_staged(&mut scene, &mut staged);
    }

    assert_ne!(staged[0] & (1u64 << 10), 0,
        "manually-set bit survives N frames with no animations");
}

#[test]
fn cascade_prevention_unaffected_by_persistent_staged() {
    // Verify same-frame cascade prevention: animation A's in-progress output
    // write is NOT visible to animation B's output-directed start_trigger within
    // the same pass. Output edges are computed *before* the animation pass (from
    // the committed staged of the previous frame), so B reacts one frame later —
    // not from A's mid-pass write. Here output_edges is empty, so B stays Armed.
    use vstimd::vtl_state::VtlEdge;
    use vstimd::scene::animation::AnimState;

    let mut scene = SceneState::new();
    let mut staged = [0u64; vtl::MAX_BANKS];

    let a = scene.add_animation({
        let mut e = AnimationEntry::armed(
            Animation::FlashForNFrames { duration_frames: 1 },
            vec![],
        );
        e.final_action = FinalAction::FINAL_ACTION_TRIGGER_LINE;
        e.final_action_trigger_line = Some(bit(0, 0));
        e
    });
    let b = scene.add_animation({
        let mut e = AnimationEntry::armed(
            Animation::FlashForNFrames { duration_frames: 1 },
            vec![],
        );
        // B starts on a rising edge of output bit 0. It reads the pre-pass output
        // edges, not A's mid-pass write into staged.
        e.start_trigger = Some((bit(0, 0), VtlEdge::Rising));
        e
    });

    // Frame 0: A completes and pulses bit 0.
    // B's start_trigger sees no output edge this pass — B must stay Armed.
    let pulses = advance_pulses(&mut scene, &mut staged);

    assert_eq!(scene.animations[&a].state, AnimState::Done, "A done");
    assert_ne!(pulses[0] & 1, 0, "A pulsed bit 0");
    assert_eq!(scene.animations[&b].state, AnimState::Armed,
        "B stays Armed — A's mid-pass write is not visible as an output edge this frame");
}
