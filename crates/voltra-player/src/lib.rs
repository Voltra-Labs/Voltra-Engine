//! The player's reusable half: the build manifest, which `cargo xtask dist`
//! reads too, and where the web pack goes. Everything that opens a window is in
//! the binary.

pub mod manifest;

pub use manifest::{Manifest, ManifestError, MANIFEST_FILE};
// Where a web build's pack comes from and goes, for `cargo xtask pack`.
pub use voltra_assets::pack::{PACK_ASSETS, PACK_FILE, PACK_ROOT};
