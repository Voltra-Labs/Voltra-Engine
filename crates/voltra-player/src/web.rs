//! The player in a browser: the game arrives as a pack, not a folder.
//!
//! There is no command line and no folder beside an executable. The page
//! serves [`PACK_FILE`] beside itself; it is fetched whole, mounted at
//! [`PACK_ROOT`] — where a native build's executable would sit — and the
//! launch is resolved from the manifest at its top exactly as a native
//! build resolves it from the one beside it. Every asset read after that
//! answers from the pack (`voltra_assets::fs`).
//!
//! What goes wrong is said in the console and in the page's
//! `#voltra-status` element, when it has one: a page that stays blank is the
//! web's "window that never opens".

use std::path::Path;

use js_sys::Uint8Array;
use voltra_player::{PACK_FILE, PACK_ROOT};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::args::Args;
use crate::{launch, start};

/// The page element a status line goes to, if the page has one.
const STATUS_ELEMENT: &str = "voltra-status";

pub fn main() {
    voltra_core::logging::init(false);
    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = run().await {
            log::error!("{error}");
            status(&error);
        }
    });
}

async fn run() -> Result<(), String> {
    status(&format!("Loading {PACK_FILE}…"));
    let bytes = fetch(PACK_FILE).await?;
    let files = voltra_assets::pack::decode(&bytes).map_err(|e| format!("{PACK_FILE}: {e}"))?;
    log::info!(
        "{PACK_FILE}: {} files, {} KiB",
        files.len(),
        bytes.len().div_ceil(1024)
    );
    drop(bytes);
    voltra_assets::fs::mount(PACK_ROOT, files);

    // No command line: the manifest decides everything, as it does for a
    // native build double-clicked.
    let launch =
        launch::resolve(&Args::default(), Some(Path::new(PACK_ROOT))).map_err(|e| e.to_string())?;
    let app = start::app(launch).ok_or("the start scene could not be loaded; see the console")?;
    status("");
    // Returns at once: the browser drives the loop from here.
    app.run();
    Ok(())
}

/// The bytes at `url`, relative to the page.
async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window: not running in a page")?;
    let failed = |what: &str, e: JsValue| format!("{url}: {what} failed: {e:?}");
    let response = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|e| failed("fetch", e))?;
    let response: web_sys::Response = response.dyn_into().map_err(|e| failed("fetch", e))?;
    if !response.ok() {
        return Err(format!(
            "{url}: the server answered {} {}",
            response.status(),
            response.status_text()
        ));
    }
    let buffer = response.array_buffer().map_err(|e| failed("read", e))?;
    let buffer = JsFuture::from(buffer)
        .await
        .map_err(|e| failed("read", e))?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

/// Shows `text` in the page's status element; empty hides it.
fn status(text: &str) {
    let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(STATUS_ELEMENT))
    else {
        return;
    };
    element.set_text_content(Some(text));
    let _ = element.set_attribute(
        "data-state",
        if text.is_empty() { "hidden" } else { "shown" },
    );
}
