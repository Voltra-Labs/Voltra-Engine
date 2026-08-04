//! Platform layer: owns the event loop, the OS window, and the frame tick.
//!
//! Everything GPU-side lives in `voltra-render`; this crate is the only place
//! allowed to depend on `winit`.

pub mod app;
pub mod input;
pub mod time;
pub mod window;

pub use app::App;
pub use input::{Input, MouseButton};
pub use time::{Clock, Timestep};
pub use window::WindowConfig;

// Re-exported so downstream crates can name keys without adding their own
// `winit` dependency — the same reason `voltra-render` re-exports `wgpu`.
pub use winit::keyboard::KeyCode;
