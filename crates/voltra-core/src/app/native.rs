//! Starting the loop on a desktop: block on the device, run until closed.

use std::sync::Arc;

use voltra_render::Renderer;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;

use super::App;

/// Runs `app` on `event_loop` until the window closes.
pub(super) fn run(mut app: App, event_loop: EventLoop<()>) {
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app).expect("event loop failed");
}

/// Builds the renderer for `window` now, blocking, and finishes startup.
pub(super) fn start_renderer(app: &mut App, window: Arc<Window>) {
    let size = window.inner_size();
    let renderer = Renderer::new(window, size.width, size.height);
    app.finish_resume(renderer);
}
