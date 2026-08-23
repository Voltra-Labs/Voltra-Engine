//! Loading and caching the files a scene refers to.
//!
//! A scene names a texture by path; this crate turns that path into a
//! `voltra_render::Texture` on the GPU, once, no matter how many entities name
//! it. Sits below `voltra-scene` and above `voltra-render`, and reaches wgpu
//! only through `voltra_render::wgpu`.

pub mod atlas;
pub mod atlases;
pub mod browse;
pub mod clips;
pub mod error;
pub mod handle;
pub mod path;
pub mod placeholder;
pub mod root;
pub mod size;
pub mod store;
pub mod textures;
pub mod watch;

pub use atlas::{Atlas, AtlasError, AtlasFile, Frame, Grid};
pub use atlases::Atlases;
pub use browse::{Entry, EntryKind};
pub use clips::Clips;
pub use error::AssetError;
pub use handle::Handle;
pub use path::AssetPath;
pub use root::{default_root, ROOT_ENV};
pub use size::TextureSizes;
pub use store::Assets;
pub use textures::Textures;
pub use watch::AssetWatcher;
