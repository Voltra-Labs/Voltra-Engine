//! Platform layer: owns the event loop, the OS window, and the frame tick.
//!
//! Everything GPU-side lives in `voltra-render`; this crate is the only place
//! allowed to depend on `winit`.

pub mod app;
pub mod window;

pub use app::App;
pub use window::WindowConfig;
