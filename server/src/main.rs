use std::sync::{Arc, RwLock};

use wonderlamp_server::render::{DrmRenderState, WindowMode, WinitApp};
use wonderlamp_server::scene::SceneState;

fn main() {
    let (render_target, window_mode) = parse_args();
    let scene = Arc::new(RwLock::new(SceneState::new()));
    let _zmq = wonderlamp_server::ipc::spawn_zmq_thread(scene.clone(), "tcp://0.0.0.0:5555");

    match render_target {
        RenderTarget::Drm => DrmRenderState::new(scene).run_loop(),
        RenderTarget::DesktopWayland => {
            let event_loop = winit::event_loop::EventLoop::new().unwrap_or_else(|e| {
                eprintln!("wonderlamp: failed to create event loop: {e}");
                std::process::exit(1);
            });
            event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
            let mut app = WinitApp::new(scene, window_mode);
            event_loop.run_app(&mut app).unwrap();
        }
        RenderTarget::DesktopX11 => {
            eprintln!("Desktop X11 is not (yet) supported");
            return;
        }
    }
}

// ── Argument parsing ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum RenderTarget {
    Drm,
    DesktopWayland,
    DesktopX11,
}
const VALID_RENDER_TARGETS: &[&str] = &["drm", "desktop-wayland", "desktop-x11"];
fn mode_from_str(s: &str) -> Option<RenderTarget> {
    if !VALID_RENDER_TARGETS.contains(&s) {
        return None;
    }
    match s {
        "drm" | "console" => Some(RenderTarget::Drm),
        "desktop-wayland" => Some(RenderTarget::DesktopWayland),
        "desktop-x11" => Some(RenderTarget::DesktopX11),
        _ => None,
    }
}

fn parse_args() -> (RenderTarget, WindowMode) {
    let mut window_mode = WindowMode::default();

    let mut mode = RenderTarget::DesktopWayland;
    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" | "-t" => match args.next().as_deref() {
                Some(target) => mode = mode_from_str(target).unwrap_or(mode),
                None => {}
            },
            "--fullscreen" | "-f" => window_mode = WindowMode::Fullscreen,
            "--windowed" | "-w" => {
                let size = args.next().and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?))
                });
                let (w, h) = size.unwrap_or((800, 600));
                window_mode = WindowMode::Windowed {
                    width: w,
                    height: h,
                };
            }
            other => {
                eprintln!("Unknown argument: {other}");
                eprintln!(
                    "Usage: wonderlamp_server [--mode drm|desktop] \
                     [--fullscreen | --windowed WxH]"
                );
                std::process::exit(1);
            }
        }
    }

    (mode, window_mode)
}
