//! Event loop driver.

use std::sync::Arc;

use voltra_render::glam::Vec2;
use voltra_render::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::KeyCode;
use winit::window::{Window, WindowId};

use crate::input::Input;
use crate::time::Clock;
use crate::window::WindowConfig;

/// World units the camera pans per second.
const PAN_SPEED: f32 = 1.5;
/// Multiplier applied per line of scroll wheel.
const ZOOM_STEP: f32 = 1.1;

#[derive(Default)]
pub struct App {
    config: WindowConfig,
    // Both stay `None` until the event loop resumes and hands us a window.
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    clock: Clock,
    input: Input,
}

impl App {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().expect("failed to create event loop");
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self).expect("event loop failed");
    }

    /// One simulation step. Everything that reads input belongs here, between
    /// the events arriving and `Input::end_frame` wiping the per-frame sets.
    fn update(&mut self) {
        let dt = self.clock.tick().as_secs_f32();

        if let Some(renderer) = self.renderer.as_mut() {
            let pan = Vec2::new(
                self.input.axis(KeyCode::KeyA, KeyCode::KeyD),
                self.input.axis(KeyCode::KeyS, KeyCode::KeyW),
            );
            if pan != Vec2::ZERO {
                // Normalising stops diagonal movement being faster than
                // axis-aligned movement. Scaling by the delta keeps the speed
                // independent of frame rate.
                renderer.camera.position += pan.normalize() * PAN_SPEED * dt;
            }

            let scroll = self.input.scroll_delta();
            if scroll != 0.0 {
                // Multiplicative so each notch feels the same at any zoom, and
                // so zoom can never reach or cross zero.
                renderer.camera.zoom *= ZOOM_STEP.powf(scroll);
            }

            if self.input.was_key_pressed(KeyCode::KeyR) {
                renderer.camera.position = Vec2::ZERO;
                renderer.camera.zoom = 1.0;
                log::info!("camera reset");
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Android and iOS resume more than once; only the first pass builds.
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(self.config.to_attributes())
                .expect("failed to create window"),
        );

        let size = window.inner_size();
        self.renderer = Some(Renderer::new(window.clone(), size.width, size.height));
        self.window = Some(window);

        // Device creation takes long enough to produce a huge first delta;
        // restarting the clock here keeps frame one honest.
        self.clock = Clock::new();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.input.process_window_event(&event);

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                log::info!("close requested, shutting down");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => renderer.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                self.update();

                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.render();
                }

                // Must come after everything that reads input this frame.
                self.input.end_frame();

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
