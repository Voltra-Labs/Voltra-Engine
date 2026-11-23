//! The player's reusable half: the build manifest, which `cargo xtask dist`
//! reads too, and the web pack's name. Everything that opens a window is in
//! the binary.

pub mod manifest;
pub mod pack;

pub use manifest::{Manifest, ManifestError, MANIFEST_FILE};
pub use pack::{PACK_FILE, PACK_ROOT};
