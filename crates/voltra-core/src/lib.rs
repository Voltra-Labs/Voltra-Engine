//! Platform layer: owns the event loop, the OS window, and the frame tick.
//!
//! Everything GPU-side lives in `voltra-render`; this crate is the only place
//! allowed to depend on `winit`.

pub mod app;
pub mod input;
pub mod time;
pub mod ui;
pub mod window;

pub use app::{App, Tick, UiFrame};
pub use input::{Input, MouseButton};
pub use time::{Clock, Timestep};
pub use ui::EguiLayer;
pub use window::WindowConfig;

// Re-exported so downstream crates can name keys without adding their own
// `winit` dependency — the same reason `voltra-render` re-exports `wgpu`.
pub use winit::keyboard::KeyCode;

// A game reads `Tick::contacts` and `Tick::events`, asks the world a question
// with `query`, and has to be able to name what comes back. The types belong to
// `voltra-physics`; re-exporting them here means a game that only asks "am I
// standing on something" needs one dependency, not two.
pub use voltra_physics::{query, CollisionEvent, Contact, QueryFilter, RayHit, Touch};

// The editor builds its panels with these, and must use the exact versions the
// layer above was compiled against.
pub use egui;
