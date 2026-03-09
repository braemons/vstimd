use std::sync::{Arc, RwLock};

use wonderlamp_server::scene::SceneState;

fn main() {
    let scene = Arc::new(RwLock::new(SceneState::new()));

    // Spawn ZMQ server thread before entering the event loop.
    let _zmq_thread = wonderlamp_server::ipc::spawn_zmq_thread(Arc::clone(&scene), "tcp://0.0.0.0:5555");

    // The frame loop: winit fires RedrawRequested → RenderState::tick()
    // ControlFlow::Poll ensures we redraw continuously (no waiting for input).
    // Press D to spawn demo stimuli (cyan disc + magenta rect).
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = wonderlamp_server::app::App::new(scene);
    event_loop.run_app(&mut app).unwrap();
}

