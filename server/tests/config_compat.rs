use vstimd::scene_config_file::load_config;
use vstimd::scene::ShapeGeometry;
use vtl::VtlKind;

#[test]
fn load_v3_reference() {
    let (scene, sections) = load_config(std::path::Path::new(
        "tests/configs/vstimd_reference_v4.config.json",
    ))
    .expect("reference v4 config must load without error");

    // Scene structure
    assert_eq!(scene.stimuli.len(), 3, "expected 3 stimuli");
    assert_eq!(scene.background.live, vstimd::Color::new(0.05, 0.05, 0.05, 1.0));

    // Stimulus 1: rect
    let rect_entry = scene.stimuli.values().find(|e| e.name() == "ref_rect").expect("ref_rect must exist");
    assert_eq!(rect_entry.stimulus.type_name(), "Rect");
    let r = rect_entry.stimulus.shape().expect("ref_rect must be a shape");
    assert_eq!(r.transform.live.pos, [100.0, -50.0]);
    assert!((r.transform.live.angle - 30.0).abs() < 1e-4);
    assert!((r.appearance.live.fill_color.r - 1.0).abs() < 1e-6);
    assert!(rect_entry.stimulus.flags().enabled);
    // The format stores full extents — the same numbers CreateRect/SetRectSize take.
    assert!(matches!(
        r.geometry.live,
        ShapeGeometry::Rect { size } if size == [400.0, 160.0]
    ));

    // Stimulus 2: circle
    let circle_entry = scene.stimuli.values().find(|e| e.name() == "ref_circle").expect("ref_circle must exist");
    assert_eq!(circle_entry.stimulus.type_name(), "Circle");
    let c = circle_entry.stimulus.shape().expect("ref_circle must be a shape");
    assert_eq!(c.transform.live.pos, [-300.0, 200.0]);
    assert!(matches!(
        c.geometry.live,
        ShapeGeometry::Circle { radius } if (radius - 50.0).abs() < 1e-4
    ));
    assert!(!circle_entry.stimulus.flags().enabled);

    // Stimulus 3: grating
    let grating_entry = scene.stimuli.values().find(|e| e.name() == "ref_grating").expect("ref_grating must exist");
    assert_eq!(grating_entry.stimulus.type_name(), "Grating");
    let g = grating_entry.stimulus.grating().expect("ref_grating must be a grating");
    assert_eq!(g.size.live, [300.0, 300.0]);

    // I/O: VTL names
    assert_eq!(sections.vtl.names.len(), 2);
    assert_eq!(sections.vtl.names[0].name, "stim_onset");
    assert_eq!(sections.vtl.names[0].bank, 0);
    assert_eq!(sections.vtl.names[0].bit, 0);
    assert_eq!(sections.vtl.names[0].kind, VtlKind::Output);
    assert_eq!(sections.vtl.names[1].name, "trial_gate");
    assert_eq!(sections.vtl.names[1].kind, VtlKind::Input);
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
        (3, "tests/configs/vstimd_reference_v3.config.json"),
    ] {
        match load_config(std::path::Path::new(path)) {
            Ok(_) => panic!("v{version} config must be rejected after the v4 format break"),
            Err(e) => assert!(
                e.to_string().contains("config version"),
                "expected a version error for v{version}, got: {e}",
            ),
        }
    }
}

/// The config format *is* the runtime shape (there is no DTO), so the reference
/// file has to be exactly what `save_config` emits. Loading it and saving it
/// again must reproduce the stimulus section byte-for-byte — that is what keeps
/// the checked-in reference honest as the scene model changes, and it is the test
/// that fails first if a field is renamed.
#[test]
fn v4_reference_survives_load_and_save_unchanged() {
    let path = std::path::Path::new("tests/configs/vstimd_reference_v4.config.json");
    let original: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let (scene, _) = load_config(path).expect("reference v4 config must load");
    let saved: serde_json::Value = serde_json::from_str(
        &vstimd::scene_config_file::retrieve_config_json(
            &scene,
            &vstimd::vtl_state::VtlConfig::default(),
        )
        .expect("save must succeed"),
    )
    .unwrap();

    assert_eq!(
        original["scene"]["stimuli"], saved["scene"]["stimuli"],
        "the reference config's stimulus section must survive load+save byte-for-byte"
    );
}
