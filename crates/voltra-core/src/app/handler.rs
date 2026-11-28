//! The event loop's callbacks: the window made, its events, the frame asked
//! for.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use super::{platform, App};

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

        // Held from here, so a second `resumed` while the renderer is still
        // on its way (the web) does not build a second one.
        self.window = Some(window.clone());
        platform::start_renderer(self, window);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, (): ()) {
        // The one event sent to the loop: the web's renderer, built.
        #[cfg(target_arch = "wasm32")]
        if let Some(renderer) = self.take_renderer() {
            self.finish_resume(renderer);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // egui gets first refusal. A keystroke aimed at a text field must not
        // also fly the camera, and a click on a panel must not select in the
        // scene behind it.
        let consumed = match (self.egui.as_mut(), self.window.as_ref()) {
            (Some(egui), Some(window)) => egui.on_window_event(window, &event),
            _ => false,
        };
        if !consumed {
            self.input.process_window_event(&event);
        }
        // Before the renderer check: a key pressed while the device is still
        // on its way is the page's gesture all the same.
        #[cfg(target_arch = "wasm32")]
        self.on_web_event(&event);

        if self.renderer.is_none() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                if self.window_requests.on_close_requested(self.ui.is_some()) {
                    log::info!("close requested, shutting down");
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                // A resize is also how fullscreen arriving, or leaving by
                // the platform's own key, shows.
                self.refresh_game_window();
            }
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => {
                // Another monitor, other modes.
                self.refresh_game_window();
            }
            WindowEvent::RedrawRequested => {
                self.refresh_game_window_shown();
                self.update();

                if self.ui.is_some() {
                    self.redraw_with_ui();
                } else {
                    self.redraw_without_ui();
                }

                // Must come after everything that reads input this frame.
                self.input.end_frame();

                // Between this frame and the next, as the scene changes
                // were: fullscreen, size and vsync change on a window that
                // is not being drawn into.
                self.apply_game_window();

                // A close the UI agreed to, or asked for itself.
                if self.window_requests.should_exit() {
                    log::info!("closing, as the UI asked");
                    event_loop.exit();
                    return;
                }

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
