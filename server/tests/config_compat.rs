use vstimd::scene_config_file::load_config;
use vstimd::scene::ShapeGeometry;
use vtl::VtlKind;

#[test]
fn load_current_reference() {
    let (scene, sections) = load_config(std::path::Path::new(
        "tests/configs/vstimd_reference_v5.config.json",
    ))
    .expect("reference v5 config must load without error");

    // Scene structure
    assert_eq!(scene.stimuli.len(), 3, "expected 3 stimuli");
    assert_eq!(scene.background.live, vstimd::Color::new(0.05, 0.05, 0.05, 1.0));

    // Stimulus 1: rect
    let rect_entry = scene.stimuli.values().find(|e| e.name() == "ref_rect").expect("ref_rect must exist");
    assert_eq!(rect_entry.stimulus.type_name(), "Rect");
    let r = rect_entry.stimulus.shape().expect("ref_rect must be a shape");
    assert_eq!(r.transform.live.pos_px, [100.0, -50.0]);
    assert!((r.transform.live.angle_deg - 30.0).abs() < 1e-4);
    assert!((r.appearance.live.fill_color.r - 1.0).abs() < 1e-6);
    assert!(rect_entry.stimulus.flags().enabled);
    // The format stores full extents — the same numbers CreateRect/SetRectSize take.
    assert!(matches!(
        r.geometry.live,
        ShapeGeometry::Rect { size_px } if size_px == [400.0, 160.0]
    ));

    // Stimulus 2: circle
    let circle_entry = scene.stimuli.values().find(|e| e.name() == "ref_circle").expect("ref_circle must exist");
    assert_eq!(circle_entry.stimulus.type_name(), "Circle");
    let c = circle_entry.stimulus.shape().expect("ref_circle must be a shape");
    assert_eq!(c.transform.live.pos_px, [-300.0, 200.0]);
    // A full extent, like every other geometry: 100 px across, not a 100 px radius.
    assert!(matches!(
        c.geometry.live,
        ShapeGeometry::Circle { diameter_px } if (diameter_px - 100.0).abs() < 1e-4
    ));
    assert!(!circle_entry.stimulus.flags().enabled);

    // Stimulus 3: grating
    let grating_entry = scene.stimuli.values().find(|e| e.name() == "ref_grating").expect("ref_grating must exist");
    assert_eq!(grating_entry.stimulus.type_name(), "Grating");
    let g = grating_entry.stimulus.grating().expect("ref_grating must be a grating");
    assert_eq!(g.size_px.live, [300.0, 300.0]);

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
/// matters most where a field kept its name but changed meaning: a v2 file loaded
/// as v3 would draw every rect, ellipse and grating at half its intended size
/// (half-extents → full width_px/height_px), and a v4 file loaded as v5 would read a
/// circle's `radius` as nothing at all now that the field is `diameter_px`.
#[test]
fn reject_older_references() {
    for (version, path) in [
        (1, "tests/configs/vstimd_reference_v1.config.json"),
        (2, "tests/configs/vstimd_reference_v2.config.json"),
        (3, "tests/configs/vstimd_reference_v3.config.json"),
        (4, "tests/configs/vstimd_reference_v4.config.json"),
    ] {
        match load_config(std::path::Path::new(path)) {
            Ok(_) => panic!("v{version} config must be rejected after the v5 format break"),
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
fn current_reference_survives_load_and_save_unchanged() {
    let path = std::path::Path::new("tests/configs/vstimd_reference_v5.config.json");
    let original: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let (scene, _) = load_config(path).expect("reference v5 config must load");
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

/// A 3-D body deserializes — `StimulusBody` has a `Mesh3d` arm and serde takes it —
/// but the 3-D types own no wire value and no query-params arm yet. Before this was
/// refused, such a file loaded happily and then killed whichever thread next walked
/// the scene for a client: `ListStimuli`, `QueryStimulus` or the web snapshot, each
/// reaching an `unimplemented!()` rather than reporting a 2-D type it is not.
#[test]
fn reject_a_stimulus_with_no_wire_type() {
    let reference = std::fs::read_to_string("tests/configs/vstimd_reference_v5.config.json")
        .expect("reference v5 config must be readable");
    let mut file: serde_json::Value = serde_json::from_str(&reference).unwrap();
    file["scene"]["stimuli"] = serde_json::json!({
        "1": {
            "id": "00000000-0000-0000-0000-0000000000ff",
            "name": "a_cube",
            "stimulus": {
                "common": { "flags": { "enabled": true, "protected": false }, "opacity": 1.0 },
                "body": {
                    "type": "Mesh3d",
                    "transform": {
                        "position_cm": [0.0, 0.0, 0.0],
                        "rotation_euler_deg": [0.0, 0.0, 0.0],
                        "scale": [1.0, 1.0, 1.0]
                    },
                    "material": {
                        "albedo": [1.0, 1.0, 1.0, 1.0],
                        "emissive": [0.0, 0.0, 0.0],
                        "shading": "Unlit"
                    },
                    "geometry": { "type": "Cube", "size_cm": [10.0, 10.0, 10.0] },
                    "texture_path": null
                }
            }
        }
    });

    let msg = match vstimd::scene_config_file::parse_config_json(&file.to_string()) {
        Ok(_) => panic!("a config carrying a 3-D stimulus must be refused"),
        Err(e) => e.to_string(),
    };
    assert!(
        msg.contains("a_cube") && msg.contains("Cube3D"),
        "the error must name the stimulus and its user-facing type, got: {msg}"
    );
}
