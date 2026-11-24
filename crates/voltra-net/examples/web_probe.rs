//! A page that opens a WebSocket to a native server and checks a datagram
//! comes back: the browser half of `crates/voltra-player/web/test/net.mjs`.
//!
//! The server is named by the page's query, `?server=ws://127.0.0.1:PORT/`.
//! On success the page's title becomes `voltra-net: echoed`; anything else
//! is logged to the console.

#[cfg(target_arch = "wasm32")]
fn main() {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use voltra_net::{Address, BrowserWebSocket, Transport};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    const PING: &[u8] = b"voltra ping";

    let say = |line: &str| web_sys::console::log_1(&line.into());
    let Some(window) = web_sys::window() else {
        return;
    };
    let search = window.location().search().unwrap_or_default();
    let Some(url) = search
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| pair.strip_prefix("server="))
        .map(str::to_owned)
    else {
        web_sys::console::error_1(&"no ?server= in the page's address".into());
        return;
    };
    let transport = match BrowserWebSocket::connect(&url) {
        Ok(transport) => transport,
        Err(e) => {
            web_sys::console::error_1(&format!("could not connect: {e}").into());
            return;
        }
    };
    say(&format!("voltra-net probe: connecting to {url}"));

    let transport = Rc::new(RefCell::new(transport));
    let document = window.document();
    let mut ticks = 0u32;
    let tick = Closure::<dyn FnMut()>::new(move || {
        ticks += 1;
        let mut transport = transport.borrow_mut();
        // A datagram a tick, as a game would: dropped until the socket opens.
        if let Err(e) = transport.update(Duration::from_millis(u64::from(ticks) * 20)) {
            web_sys::console::error_1(&format!("update failed: {e}").into());
        }
        if transport.is_open() {
            if let Err(e) = transport.send(Address::Server, PING) {
                web_sys::console::error_1(&format!("send failed: {e}").into());
            }
        }
        while let Ok(Some(datagram)) = transport.recv() {
            if datagram.payload == PING && datagram.from == Address::Server {
                say("voltra-net probe: echoed");
                if let Some(document) = document.as_ref() {
                    document.set_title("voltra-net: echoed");
                }
            }
        }
    });
    if let Err(e) = window
        .set_interval_with_callback_and_timeout_and_arguments_0(tick.as_ref().unchecked_ref(), 20)
    {
        web_sys::console::error_1(&e);
    }
    // The page's lifetime: the interval calls it until the tab closes.
    tick.forget();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
