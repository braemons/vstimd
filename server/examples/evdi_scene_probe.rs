//! End-to-end hardware test: real stimuli through the real Vulkan pipeline,
//! read back, and presented on the evdi (DisplayLink) output — no ZMQ
//! client available on this box (no `uv`/Python zmq bindings installed), so
//! stimuli are created directly via `SceneState::handle_request`, exactly
//! like the crate's own integration tests do.
//!
//! `cargo run --release --example evdi_scene_probe -- [seconds]`

use std::sync::{Arc, RwLock};

use vstimd::proto;
use vstimd::proto::request;
use vstimd::render::backend::{BackendData, DisplayModePref};
use vstimd::render::evdi::EvdiBackend;
use vstimd::render::system_info::HostInfo;
use vstimd::scene::SceneState;

fn sys() -> request::Target {
    request::Target::System(proto::SystemTarget {})
}

fn create_rect(scene: &mut SceneState, x: f32, y: f32, w: f32, h: f32, color: proto::Color) -> u32 {
    let resp = scene.handle_request(
        proto::Request {
            target: Some(sys()),
            body: Some(request::Body::CreateRect(proto::CreateRectRequest {
                center: Some(proto::Vec2 { x, y }),
                width: w,
                height: h,
                fill_color: Some(color),
                ..Default::default()
            })),
        },
        None,
    );
    assert_eq!(resp.code, proto::ErrorCode::Ok as i32, "CreateRect failed: {}", resp.error);
    resp.handle as u32
}

fn main() {
    let hold_secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(15);

    let mut scene_state = SceneState::new();
    create_rect(&mut scene_state, -300.0, 0.0, 300.0, 200.0, proto::Color { r: 0.9, g: 0.15, b: 0.15, a: 1.0 });
    create_rect(&mut scene_state, 300.0, 0.0, 300.0, 200.0, proto::Color { r: 0.15, g: 0.4, b: 0.9, a: 1.0 });
    create_rect(&mut scene_state, 0.0, -250.0, 250.0, 150.0, proto::Color { r: 0.2, g: 0.85, b: 0.3, a: 1.0 });
    println!("created 3 rects (red / blue / green)");

    let scene = Arc::new(RwLock::new(scene_state));

    let data = BackendData {
        scene,
        vtl: None,
        host_info: HostInfo {
            hardware_model: "evdi_scene_probe".to_string(),
            hostname: "localhost".to_string(),
            local_ip: "127.0.0.1".to_string(),
            zmq_port: 0,
            sched: Default::default(),
        },
        overlay_scale: 1.0,
        display_pref: DisplayModePref::default(),
        clock_pref: None,
        rig_config_path: "(none — evdi_scene_probe)".to_string(),
    };

    let log_buffer = vstimd::log_buffer::install(
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).build(),
        std::time::Instant::now(),
    );

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(hold_secs));
        println!("time's up, requesting shutdown");
        vstimd::shutdown::request();
    });

    println!("starting EvdiBackend for {hold_secs}s — check the physical DisplayLink screen");
    EvdiBackend::new(data, log_buffer).run(|| println!("evdi backend ready"));
    println!("stopped");
}
