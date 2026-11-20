//! The player's reusable half: the build manifest, which `cargo xtask dist`
//! reads too. Everything that opens a window is in the binary.

pub mod manifest;

pub use manifest::{Manifest, ManifestError, MANIFEST_FILE};
