//! Loading and caching the files a scene refers to.
//!
//! A scene names a texture by path; this crate turns that path into a
//! `voltra_render::Texture` on the GPU, once, no matter how many entities name
//! it. Sits below `voltra-scene` and above `voltra-render`, and reaches wgpu
//! only through `voltra_render::wgpu`.

pub mod error;
pub mod handle;
pub mod path;
pub mod placeholder;
pub mod root;
pub mod store;
pub mod textures;
pub mod watch;

pub use error::AssetError;
pub use handle::Handle;
pub use path::AssetPath;
pub use root::{default_root, ROOT_ENV};
pub use store::Assets;
pub use textures::Textures;
pub use watch::AssetWatcher;
