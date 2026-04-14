use std::sync::{Arc, RwLock};

use wonderlamp_server::scene::SceneState;

fn main() {
    let scene = Arc::new(RwLock::new(SceneState::new()));

    // Spawn ZMQ server thread before entering the render loop.
    let _zmq_thread =
        wonderlamp_server::ipc::spawn_zmq_thread(Arc::clone(&scene), "tcp://0.0.0.0:5555");

    #[cfg(feature = "drm")]
    {
        let render = wonderlamp_server::render::RenderState::new(scene);
        render.run_loop();
    }

    #[cfg(not(feature = "drm"))]
    {
        use wonderlamp_server::app::{App, WindowMode};

        fn parse_window_mode() -> WindowMode {
            let mut args = std::env::args().skip(1);
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--fullscreen" | "-f" => return WindowMode::Fullscreen,
                    "--windowed" | "-w" => {
                        let size = args.next().and_then(|s| {
                            let (w, h) = s.split_once('x')?;
                            Some((w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?))
                        });
                        let (width, height) = size.unwrap_or((800, 600));
                        return WindowMode::Windowed { width, height };
                    }
                    _ => {}
                }
            }
            WindowMode::default()
        }

        // The frame loop: winit fires RedrawRequested → RenderState::tick()
        // ControlFlow::Poll ensures we redraw continuously (no waiting for input).
        let event_loop = winit::event_loop::EventLoop::new().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

        let window_mode = parse_window_mode();
        let mut app = App::new(scene).with_window_mode(window_mode);
        event_loop.run_app(&mut app).unwrap();
    }
}
