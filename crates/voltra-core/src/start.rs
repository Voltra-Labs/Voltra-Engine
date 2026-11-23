//! Starting a game once its files can be read.
//!
//! Natively the asset root is a folder on disk and a game builds its `App`
//! the moment `main` runs. A browser has no disk: the game's files arrive as
//! one pack ([`voltra_assets::pack`]) fetched beside the page, and nothing
//! may read a scene, a texture or an input map until it is mounted. A game
//! that wraps its start in [`when_assets_ready`] runs the same on both.
//!
//! A game's `main`, whole:
//!
//! ```no_run
//! voltra_core::logging::init(false);
//! voltra_core::when_assets_ready(|| {
//!     let app = voltra_core::App::new(Default::default());
//!     // …load scenes, install ticks…
//!     app.run();
//!     Ok(())
//! });
//! ```

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::when_assets_ready;
#[cfg(target_arch = "wasm32")]
pub use web::when_assets_ready;
