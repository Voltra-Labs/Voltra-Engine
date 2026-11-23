//! Starting the loop in a browser, where nothing may block.
//!
//! The device arrives through promises, so the renderer is built in a
//! future (`spawn_local`) and handed back to the loop through a slot the
//! loop owns, with a wake-up through the event loop's proxy: the loop then
//! finishes `resumed` in `user_event`, on its own turn, exactly as natively
//! it does inline. Bevy and every winit-on-the-web sample take this shape;
//! the slot rather than the renderer in the event keeps the proxy's event
//! type `()` on both targets.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use voltra_render::Renderer;
use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy};
use winit::platform::web::EventLoopExtWebSys;
use winit::window::Window;

use super::App;

/// The renderer on its way, and how to say it has arrived.
#[derive(Default)]
pub(super) struct PendingRenderer {
    slot: Rc<RefCell<Option<Renderer>>>,
    proxy: Option<EventLoopProxy<()>>,
}

/// Hands `app` to the browser's event loop and returns at once: the page's
/// animation frames drive it from here.
pub(super) fn run(mut app: App, event_loop: EventLoop<()>) {
    // A frame is asked for at the end of every frame, and the browser paces
    // those to the display; polling between them would spin for nothing.
    event_loop.set_control_flow(ControlFlow::Wait);
    app.pending_renderer.proxy = Some(event_loop.create_proxy());
    event_loop.spawn_app(app);
}

/// Starts building the renderer for `window`; [`App::finish_resume`] runs
/// when it is done, from `user_event`.
pub(super) fn start_renderer(app: &mut App, window: Arc<Window>) {
    let Some(proxy) = app.pending_renderer.proxy.clone() else {
        log::error!("no event loop proxy: App::run was bypassed, so nothing can draw");
        return;
    };
    let slot = Rc::clone(&app.pending_renderer.slot);
    wasm_bindgen_futures::spawn_local(async move {
        // The canvas may not be laid out yet; the size is corrected once
        // the renderer is in place (`take_renderer`'s caller resizes).
        let size = window.inner_size();
        let renderer = Renderer::new_async(window, size.width, size.height).await;
        *slot.borrow_mut() = Some(renderer);
        if proxy.send_event(()).is_err() {
            log::error!("the event loop closed before the renderer was ready");
        }
    });
}

impl App {
    /// The renderer, if it arrived since the last call.
    pub(super) fn take_renderer(&mut self) -> Option<Renderer> {
        self.pending_renderer.slot.borrow_mut().take()
    }
}
