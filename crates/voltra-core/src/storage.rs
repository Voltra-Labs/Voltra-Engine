//! Where a player's own files are kept, by platform: the one seam every
//! per-user file goes through — save slots ([`Saves`](crate::Saves)), the
//! controls a player rebound and the options a game keeps, all through
//! [`Saves`](crate::Saves)' folder — and the tool-written input map file
//! ([`ActionMap::save`](crate::ActionMap::save)).
//!
//! A file per path on a desktop, written whole through a temporary and a
//! rename; one `localStorage` key per path in a browser, which has no
//! files. The functions take the full path, so the folder rules
//! ([`saves::dir`](crate::saves::dir)) stay in one place above them.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{file_names, read, remove, size, write};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{file_names, read, remove, size, write};
