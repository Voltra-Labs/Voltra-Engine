//! Starting in a browser: fetch the pack, mount it, then start.
//!
//! The page serves [`PACK_FILE`] beside itself; it is fetched whole and
//! mounted at [`PACK_ROOT`], where a native build's executable would sit, so
//! every read through `voltra_assets::fs` answers from it and
//! `voltra_assets::default_root` is its asset folder.
//!
//! What goes wrong is said in the console and in the page's
//! `#voltra-status` element, when it has one: a page that stays blank is the
//! web's "window that never opens".

use js_sys::Uint8Array;
use voltra_assets::pack::{PACK_FILE, PACK_ROOT};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

/// The page element a status line goes to, if the page has one.
const STATUS_ELEMENT: &str = "voltra-status";

/// Fetches and mounts the pack, then runs `game`. Returns at once; `game`
/// runs when the pack has arrived. Its error, or the fetch's, is shown.
pub fn when_assets_ready(game: impl FnOnce() -> Result<(), String> + 'static) {
    wasm_bindgen_futures::spawn_local(async move {
        let started = async {
            mount_pack().await?;
            status("");
            game()
        };
        if let Err(error) = started.await {
            log::error!("{error}");
            status(&error);
        }
    });
}

async fn mount_pack() -> Result<(), String> {
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
    let state = if text.is_empty() { "hidden" } else { "shown" };
    // A page without the attribute styled just shows the text.
    let _ = element.set_attribute("data-state", state);
}
