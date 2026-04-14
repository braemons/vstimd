use std::sync::{Arc, RwLock};

use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::render::RenderState;
use crate::scene::{
    Deferred, DiscStimulus, RectStimulus, SceneState, ShapeAppearance, Stimulus, StimulusFlags,
    Transform2D,
};

/// Controls how the window is created at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMode {
    /// Borderless fullscreen on the primary monitor.
    Fullscreen,
    /// Fixed-size windowed mode (width × height in logical pixels).
    Windowed { width: u32, height: u32 },
}

impl Default for WindowMode {
    fn default() -> Self {
        Self::Fullscreen
    }
}

pub struct App {
    /// Held here until the window is created, then moved into `RenderState`.
    scene: Option<Arc<RwLock<SceneState>>>,
    state: Option<RenderState>,
    /// The mode used when the window was first created (and when toggling back
    /// to windowed, so we know what size to restore).
    window_mode: WindowMode,
    /// Tracks whether the window is currently fullscreen so Alt+Enter knows
    /// which direction to toggle.
    is_fullscreen: bool,
    /// Current modifier state, kept up-to-date via `ModifiersChanged`.
    modifiers: winit::event::Modifiers,
}

impl App {
    pub fn new(scene: Arc<RwLock<SceneState>>) -> Self {
        let default_mode = WindowMode::default();
        Self {
            scene: Some(scene),
            state: None,
            is_fullscreen: default_mode == WindowMode::Fullscreen,
            window_mode: default_mode,
            modifiers: winit::event::Modifiers::default(),
        }
    }

    pub fn with_window_mode(mut self, mode: WindowMode) -> Self {
        self.is_fullscreen = mode == WindowMode::Fullscreen;
        self.window_mode = mode;
        self
    }

    /// Toggle between borderless fullscreen and the configured windowed size.
    fn toggle_fullscreen(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let Some(state) = &self.state else { return };

        if self.is_fullscreen {
            // Switch to windowed, restoring the configured size.
            state.window.set_fullscreen(None);
            if let WindowMode::Windowed { width, height } = self.window_mode {
                let size = winit::dpi::LogicalSize::new(width, height);
                let _ = state.window.request_inner_size(size);
            }
            self.is_fullscreen = false;
        } else {
            // Switch to borderless fullscreen on whichever monitor the window
            // is currently on, falling back to primary then any available.
            let monitor = state
                .window
                .current_monitor()
                .or_else(|| event_loop.primary_monitor())
                .or_else(|| event_loop.available_monitors().next());
            state
                .window
                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(monitor)));
            self.is_fullscreen = true;
        }
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.state.is_none() {
            let attrs = match self.window_mode {
                WindowMode::Fullscreen => {
                    // Always open on the primary monitor (fallback: first available).
                    // Borderless fullscreen lets the DXGI flip chain go directly to
                    // the display (Hardware: Independent Flip in PresentMon).
                    let monitor = event_loop
                        .primary_monitor()
                        .or_else(|| event_loop.available_monitors().next());
                    let fullscreen = winit::window::Fullscreen::Borderless(monitor);
                    winit::window::Window::default_attributes()
                        .with_title("Wonderlamp")
                        .with_fullscreen(Some(fullscreen))
                }
                WindowMode::Windowed { width, height } => {
                    let size = winit::dpi::LogicalSize::new(width, height);
                    winit::window::Window::default_attributes()
                        .with_title("Wonderlamp")
                        .with_inner_size(size)
                        .with_resizable(false)
                }
            };

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
            winit::event::WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods;
            }
            winit::event::WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match key {
                KeyCode::F1 => state.show_overlay = !state.show_overlay,
                KeyCode::KeyD => spawn_demo_stimuli(&state.scene),
                KeyCode::Enter if self.modifiers.state().alt_key() => {
                    self.toggle_fullscreen(event_loop);
                }
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
            flags: StimulusFlags {
                enabled: true,
                ..Default::default()
            },
            transform: Deferred::new(Transform2D {
                pos: [-150.0, 0.0],
                angle: 0.0,
            }),
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
            flags: StimulusFlags {
                enabled: true,
                ..Default::default()
            },
            transform: Deferred::new(Transform2D {
                pos: [150.0, 0.0],
                angle: 30.0,
            }),
            appearance: Deferred::new(ShapeAppearance {
                fill_color: [0.8, 0.0, 0.8, 1.0],
                ..Default::default()
            }),
            size: Deferred::new([120.0, 50.0]),
        }),
    );

    eprintln!("Demo: spawned disc (handle {h1}) and rect (handle {h2})");
}
