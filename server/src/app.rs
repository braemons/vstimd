use std::sync::{Arc, RwLock};

use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::render::RenderState;
use crate::scene::{
    Deferred, DiscStimulus, RectStimulus, SceneState, ShapeAppearance, Stimulus, StimulusFlags,
    Transform2D,
};

pub struct App {
    /// Held here until the window is created, then moved into `RenderState`.
    scene: Option<Arc<RwLock<SceneState>>>,
    state: Option<RenderState>,
}

impl App {
    pub fn new(scene: Arc<RwLock<SceneState>>) -> Self {
        Self { scene: Some(scene), state: None }
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.state.is_none() {
            // Always open on the primary monitor (fallback: first available).
            // Borderless fullscreen lets the DXGI flip chain go directly to
            // the display (Hardware: Independent Flip in PresentMon).
            let monitor = event_loop
                .primary_monitor()
                .or_else(|| event_loop.available_monitors().next());
            let fullscreen = winit::window::Fullscreen::Borderless(monitor);

            let attrs = winit::window::Window::default_attributes()
                .with_title("Wonderlamp")
                .with_fullscreen(Some(fullscreen));
            let window = std::sync::Arc::new(event_loop.create_window(attrs).unwrap());
            let scene = self.scene.take().expect("scene already consumed");
            self.state = Some(RenderState::new(
                window,
                scene,
                #[cfg(feature = "overlay")]
                event_loop,
            ));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let Some(state) = &mut self.state else { return };

        #[cfg(feature = "overlay")]
        if let Some(overlay) = &mut state.overlay {
            let response = overlay.on_window_event(&state.window, &event);
            if response.consumed {
                return;
            }
        }

        match event {
            winit::event::WindowEvent::CloseRequested => event_loop.exit(),
            winit::event::WindowEvent::Resized(size) => state.resize(size),
            winit::event::WindowEvent::KeyboardInput {
                event: winit::event::KeyEvent {
                    physical_key: PhysicalKey::Code(key),
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => match key {
                KeyCode::F1 => state.show_overlay = !state.show_overlay,
                KeyCode::KeyD => spawn_demo_stimuli(&state.scene),
                _ => {}
            },
            winit::event::WindowEvent::RedrawRequested => state.tick(),
            _ => {}
        }
    }
}

/// Pressing D inserts a cyan disc and a magenta rect for quick visual testing.
fn spawn_demo_stimuli(scene: &Arc<RwLock<SceneState>>) {
    let mut scene = scene.write().expect("scene lock poisoned");

    let h1 = scene.alloc_stim_handle();
    scene.stimuli.insert(
        h1,
        Stimulus::Disc(DiscStimulus {
            flags:      StimulusFlags { enabled: true, ..Default::default() },
            transform:  Deferred::new(Transform2D { pos: [-150.0, 0.0], angle: 0.0 }),
            appearance: Deferred::new(ShapeAppearance {
                fill_color: [0.0, 0.8, 0.8, 1.0],
                ..Default::default()
            }),
            radius: Deferred::new(80.0),
        }),
    );

    let h2 = scene.alloc_stim_handle();
    scene.stimuli.insert(
        h2,
        Stimulus::Rect(RectStimulus {
            flags:      StimulusFlags { enabled: true, ..Default::default() },
            transform:  Deferred::new(Transform2D { pos: [150.0, 0.0], angle: 30.0 }),
            appearance: Deferred::new(ShapeAppearance {
                fill_color: [0.8, 0.0, 0.8, 1.0],
                ..Default::default()
            }),
            size: Deferred::new([120.0, 50.0]),
        }),
    );

    eprintln!("Demo: spawned disc (handle {h1}) and rect (handle {h2})");
}

