//! Event loop driver.

use std::sync::Arc;

use voltra_render::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::window::WindowConfig;

#[derive(Default)]
pub struct App {
    config: WindowConfig,
    // Both stay `None` until the event loop resumes and hands us a window.
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
}

impl App {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            window: None,
            renderer: None,
        }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().expect("failed to create event loop");
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self).expect("event loop failed");
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
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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
                renderer.render();
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
