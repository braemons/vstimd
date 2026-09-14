pub(crate) fn spawn_demo_stimuli(
    scene: &std::sync::Arc<std::sync::RwLock<crate::scene::SceneState>>,
) {
    use crate::scene::{
        Anchor, Grating, GratingParams, LanguageStyle, Shape, ShapeAppearance, ShapeGeometry,
        Stimulus, StimulusIdentity, StimulusSceneEntry, Text, TextRenderParams, Waveform,
    };
    use rand::RngExt;

    let mut rng = rand::rng();

    let mut sc = scene.write().expect("scene lock poisoned");
    let h1 = sc.alloc_stim_handle();
    sc.stimuli.insert(
        h1,
        StimulusSceneEntry::new(
            StimulusIdentity::new(Some("demo_circle".into())),
            Stimulus::from(Shape::new(
                [
                    rng.random_range(-500.0..500.0),
                    rng.random_range(-500.0..500.0),
                ],
                0.0,
                ShapeAppearance {
                    fill_color: crate::Color::new(0.0, 0.8, 0.8, 1.0),
                    ..Default::default()
                },
                ShapeGeometry::Circle { diameter_px: 160.0 },
            )),
        ),
    );
    let h2 = sc.alloc_stim_handle();
    sc.stimuli.insert(
        h2,
        StimulusSceneEntry::new(
            StimulusIdentity::new(Some("demo_rect".into())),
            Stimulus::from(Shape::new(
                [
                    rng.random_range(-500.0..500.0),
                    rng.random_range(-500.0..500.0),
                ],
                30.0,
                ShapeAppearance {
                    fill_color: crate::Color::new(0.8, 0.0, 0.8, 1.0),
                    ..Default::default()
                },
                ShapeGeometry::Rect {
                    size_px: [240.0, 100.0],
                },
            )),
        ),
    );
    let h3 = sc.alloc_stim_handle();
    sc.stimuli.insert(
        h3,
        StimulusSceneEntry::new(
            StimulusIdentity::new(Some("demo_grating".into())),
            Stimulus::from(Grating::new(
                [100.0, -200.0],
                0.0,
                [100.0, 100.0],
                GratingParams {
                    sf_cycles_per_px: 0.05,
                    contrast: 1.0,
                    drift_speed_hz: 1.0,
                    waveform: Waveform::Sin,
                    ..Default::default()
                },
            )),
        ),
    );
    let h4 = sc.alloc_stim_handle();
    sc.stimuli.insert(
        h4,
        StimulusSceneEntry::new(
            StimulusIdentity::new(Some("demo_text".into())),
            Stimulus::from(Text::new(
                [0.0, 200.0],
                0.0,
                [400.0, 80.0],
                "vstimd".into(),
                "".into(), // falls back to DEFAULT_FONT_FAMILY ("Ubuntu Light")
                48.0,
                Anchor::Center,
                LanguageStyle::default(),
                TextRenderParams {
                    color: crate::Color::new(1.0, 1.0, 0.0, 1.0),
                    ..Default::default()
                },
            )),
        ),
    );
    log::info!("Demo: spawned circle #{h1}, rect #{h2}, grating #{h3}, text #{h4}");
}

/// Temporary (until #72 gives 3-D stimuli wire commands): with
/// `VSTIMD_DEBUG_3D` set, put a few 3-D stimuli in the scene at startup so the
/// renderer can be looked at. Run with `--no-web` — the web snapshot cannot
/// describe a 3-D stimulus yet.
///
/// A floor, a cube with a stretched box pushed through it, and a sphere
/// stretched into an ellipsoid, all Phong-lit in front of the default camera:
/// one look checks depth, culling, winding of every primitive, that world +Y is
/// screen up, and — from the ellipsoid's terminator — the normal matrix.
pub(crate) fn spawn_debug_3d_stimuli(
    scene: &std::sync::Arc<std::sync::RwLock<crate::scene::SceneState>>,
) {
    use crate::Color;
    use crate::scene::{
        Material3D, Mesh3d, Mesh3dGeometry, Shading3D, Stimulus, StimulusIdentity,
        StimulusSceneEntry, Transform3D,
    };
    use glam::Vec3;

    let objects = [
        (
            "debug_floor",
            Mesh3dGeometry::Plane {
                size_cm: [120.0, 200.0],
            },
            Vec3::new(0.0, -20.0, -100.0),
            Vec3::ZERO,
            Color::new(0.25, 0.25, 0.3, 1.0),
        ),
        (
            "debug_cube",
            Mesh3dGeometry::Cube { size_cm: [20.0; 3] },
            Vec3::new(-15.0, 0.0, -70.0),
            Vec3::new(30.0, 20.0, 0.0),
            Color::new(0.9, 0.3, 0.2, 1.0),
        ),
        (
            "debug_bar",
            Mesh3dGeometry::Cube {
                size_cm: [30.0, 4.0, 4.0],
            },
            Vec3::new(-2.0, 10.0, -70.0),
            Vec3::new(0.0, 0.0, 15.0),
            Color::new(0.9, 0.8, 0.2, 1.0),
        ),
        (
            "debug_sphere",
            Mesh3dGeometry::Sphere {
                diameter_cm: 18.0,
                rings: 16,
                sectors: 32,
            },
            Vec3::new(20.0, -5.0, -80.0),
            Vec3::ZERO,
            Color::new(0.2, 0.6, 0.9, 1.0),
        ),
    ];
    let mut sc = scene.write().expect("scene lock poisoned");
    for (name, geometry, position_cm, rotation_euler_deg, albedo) in objects {
        let h = sc.alloc_stim_handle();
        let scale = if name == "debug_sphere" {
            Vec3::new(1.0, 1.8, 1.0)
        } else {
            Vec3::ONE
        };
        let mesh = Mesh3d::new(
            Transform3D {
                position_cm,
                rotation_euler_deg,
                scale,
            },
            Material3D {
                albedo,
                shading: Shading3D::Phong,
                ..Default::default()
            },
            geometry,
            None,
        );
        sc.stimuli.insert(
            h,
            StimulusSceneEntry::new(
                StimulusIdentity::new(Some(name.into())),
                Stimulus::from(mesh),
            ),
        );
    }
}
