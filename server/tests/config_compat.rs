use vstimd::io_config::load_config;
use vstimd::scene::Stimulus;
use vtl::VtlKind;

#[test]
fn load_v3_reference() {
    let (scene, io) = load_config(std::path::Path::new(
        "tests/configs/vstimd_reference_v3.config.json",
    ))
    .expect("reference v3 config must load without error");

    // Scene structure
    assert_eq!(scene.stimuli.len(), 3, "expected 3 stimuli");
    assert_eq!(scene.background.live, vstimd::Color::new(0.05, 0.05, 0.05, 1.0));

    // Stimulus 1: rect
    let rect_entry = scene.stimuli.values().find(|e| e.name.as_deref() == Some("ref_rect")).expect("ref_rect must exist");
    assert!(matches!(rect_entry.stimulus, Stimulus::Rect(_)));
    if let Stimulus::Rect(ref r) = rect_entry.stimulus {
        assert_eq!(r.common.transform.live.pos, [100.0, -50.0]);
        assert!((r.common.transform.live.angle - 30.0).abs() < 1e-4);
        assert!((r.common.appearance.live.fill_color.r - 1.0).abs() < 1e-6);
        assert!(r.common.flags.enabled);
        // v3 stores full extents — the same numbers CreateRect/SetRectSize take.
        assert_eq!(r.size.live, [400.0, 160.0]);
    }

    // Stimulus 2: circle
    let circle_entry = scene.stimuli.values().find(|e| e.name.as_deref() == Some("ref_circle")).expect("ref_circle must exist");
    assert!(matches!(circle_entry.stimulus, Stimulus::Circle(_)));
    if let Stimulus::Circle(ref c) = circle_entry.stimulus {
        assert_eq!(c.common.transform.live.pos, [-300.0, 200.0]);
        assert!((c.radius.live - 50.0).abs() < 1e-4);
        assert!(!c.common.flags.enabled);
    }

    // Stimulus 3: grating
    let grating_entry = scene.stimuli.values().find(|e| e.name.as_deref() == Some("ref_grating")).expect("ref_grating must exist");
    assert!(matches!(grating_entry.stimulus, Stimulus::Grating(_)));
    if let Stimulus::Grating(ref g) = grating_entry.stimulus {
        assert_eq!(g.size.live, [300.0, 300.0]);
    }

    // I/O: VTL names
    assert_eq!(io.vtl.names.len(), 2);
    assert_eq!(io.vtl.names[0].name, "stim_onset");
    assert_eq!(io.vtl.names[0].bank, 0);
    assert_eq!(io.vtl.names[0].bit, 0);
    assert_eq!(io.vtl.names[0].kind, VtlKind::Output);
    assert_eq!(io.vtl.names[1].name, "trial_gate");
    assert_eq!(io.vtl.names[1].kind, VtlKind::Input);
}

/// Older on-disk formats are rejected rather than silently mis-parsed. This
/// matters most for v2 → v3: the shape fields kept their names but changed
/// meaning (half-extents → full width/height), so a v2 file loaded as v3 would
/// draw every rect, ellipse and grating at half its intended size instead of
/// failing.
#[test]
fn reject_older_references() {
    for (version, path) in [
        (1, "tests/configs/vstimd_reference_v1.config.json"),
        (2, "tests/configs/vstimd_reference_v2.config.json"),
    ] {
        match load_config(std::path::Path::new(path)) {
            Ok(_) => panic!("v{version} config must be rejected after the v3 format break"),
            Err(e) => assert!(
                e.to_string().contains("config version"),
                "expected a version error for v{version}, got: {e}",
            ),
        }
    }
}
