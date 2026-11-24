//! A page that connects to a native server through the browser's WebSocket
//! — the handshake, then a short message and one long enough to go in
//! fragments, each echoed back: the browser half of
//! `crates/voltra-player/web/test/net.mjs`.
//!
//! The server is named by the page's query, `?server=ws://127.0.0.1:PORT/`.
//! On success the page's title becomes `voltra-net: echoed`; anything else
//! is logged to the console.

#[cfg(target_arch = "wasm32")]
fn main() {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use voltra_net::{Address, BrowserWebSocket, ChannelId, Endpoint, NetConfig, NetEvent, PeerId};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    const PING: &[u8] = b"voltra ping";
    const LONG: usize = 3000;

    let say = |line: &str| web_sys::console::log_1(&line.into());
    let fail = |line: &str| web_sys::console::error_1(&line.into());
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
        fail("no ?server= in the page's address");
        return;
    };
    let transport = match BrowserWebSocket::connect(&url) {
        Ok(transport) => transport,
        Err(e) => {
            fail(&format!("could not connect: {e}"));
            return;
        }
    };
    say(&format!("voltra-net probe: connecting to {url}"));
    let client = Rc::new(RefCell::new(Endpoint::client(
        transport,
        Address::Server,
        NetConfig::default(),
    )));

    let long: Vec<u8> = (0..LONG).map(|i| (i % 251) as u8).collect();
    let document = window.document();
    let mut ticks = 0u64;
    let mut echoed = (false, false);
    let tick = Closure::<dyn FnMut()>::new(move || {
        ticks += 1;
        let now = Duration::from_millis(ticks * 20);
        let mut client = client.borrow_mut();
        client.receive(now);
        for event in client.events().to_vec() {
            match event {
                NetEvent::Connected(_) => {
                    say(&format!(
                        "voltra-net probe: joined as {:?}",
                        client.local_id()
                    ));
                    for bytes in [PING, &long[..]] {
                        if let Err(e) =
                            client.send(PeerId::SERVER, ChannelId::RELIABLE_ORDERED, bytes)
                        {
                            fail(&format!("send failed: {e}"));
                        }
                    }
                }
                NetEvent::Disconnected { reason, .. } => fail(&format!("disconnected: {reason}")),
            }
        }
        for message in client.take_messages() {
            echoed.0 |= message.bytes == PING;
            echoed.1 |= message.bytes == long;
        }
        client.flush(now);
        if echoed == (true, true) {
            echoed = (false, false);
            say("voltra-net probe: echoed");
            if let Some(document) = document.as_ref() {
                document.set_title("voltra-net: echoed");
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
