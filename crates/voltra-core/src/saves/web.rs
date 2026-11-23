//! Save slots in a browser: the page's `localStorage`.
//!
//! What Godot's and Unity's web exports do too (theirs through IndexedDB,
//! behind a filesystem shim; a slot is text and small, so the synchronous
//! store does). Each slot is one key, the path the folder rules give it
//! (`/<game>/saves/<slot>.save.ron`), so two games on one origin keep apart
//! as two folders would. A browser that refuses storage — private mode,
//! storage turned off — is an error to the game, as a read-only disk is.

use std::io;
use std::path::Path;

use web_sys::Storage;

fn storage() -> io::Result<Storage> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "no localStorage on this page"))
}

/// A key is the path, `/`-separated whatever `Path` holds.
fn key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn refused(what: &str, error: wasm_bindgen::JsValue) -> io::Error {
    io::Error::other(format!("localStorage {what} refused: {error:?}"))
}

/// One write: `localStorage` replaces a value whole or not at all — a full
/// quota fails the call and leaves the old value.
pub(super) fn write(path: &Path, text: &str) -> io::Result<()> {
    storage()?
        .set_item(&key(path), text)
        .map_err(|e| refused("write", e))
}

/// The length of what `path` holds, in bytes, or `None` when nothing does.
pub(super) fn size(path: &Path) -> io::Result<Option<u64>> {
    let value = storage()?
        .get_item(&key(path))
        .map_err(|e| refused("read", e))?;
    Ok(value.map(|text| text.len() as u64))
}

pub(super) fn read(path: &Path) -> io::Result<String> {
    storage()?
        .get_item(&key(path))
        .map_err(|e| refused("read", e))?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, key(path)))
}

/// Removes `path`; whether there was anything to remove.
pub(super) fn remove(path: &Path) -> io::Result<bool> {
    let storage = storage()?;
    let key = key(path);
    let had = storage
        .get_item(&key)
        .map_err(|e| refused("read", e))?
        .is_some();
    storage
        .remove_item(&key)
        .map_err(|e| refused("remove", e))?;
    Ok(had)
}

/// The last part of every key directly under `dir`, as a folder's file
/// names; none when storage is unavailable.
pub(super) fn file_names(dir: &Path) -> Vec<String> {
    let Ok(storage) = storage() else {
        return Vec::new();
    };
    let prefix = format!("{}/", key(dir).trim_end_matches('/'));
    let count = storage.length().unwrap_or(0);
    (0..count)
        .filter_map(|index| storage.key(index).ok().flatten())
        .filter_map(|key| {
            let name = key.strip_prefix(&prefix)?;
            (!name.is_empty() && !name.contains('/')).then(|| name.to_owned())
        })
        .collect()
}
