//! Platform layer: owns the event loop, the OS window, and the frame tick.
//!
//! Everything GPU-side lives in `voltra-render`; this crate is the only place
//! allowed to depend on `winit`.

pub mod app;
pub mod input;
pub mod logging;
pub mod saves;
pub mod time;
pub mod ui;
pub mod window;

pub use app::headless::{Headless, HeadlessError};
pub use app::{App, SceneLoad, Tick, UiFrame, DEFAULT_INPUT_MAP, DEFAULT_SORTING_LAYERS};
pub use input::{
    ActionMap, ActionState, Binding, Direction, Input, MouseButton, PadAxis, PadButton,
};
pub use saves::{SaveError, Saves};
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
// Navigation, the same way: a game asks for a path and needs no second
// dependency to hold the answer.
pub use voltra_nav as nav;
pub use voltra_physics::{
    character, move_and_slide, query, CollisionEvent, Contact, QueryFilter, RayHit, ShapeHit,
    Slide, Touch,
};

// A game plays a sound through `Tick::audio` and has to be able to name what
// it hands over and what comes back. Re-exported for the reason the physics
// types above are: one dependency for a game that wants a coin to make a
// noise, not two.
pub use voltra_audio::{Audio, BusId, Buses, Clip, PlayParams, VoiceId};
// A game spawns prefabs through `Tick::spawn` and gets back an `Instance`.
pub use voltra_scene::{Instance, Prefab};

// The editor builds its panels with these, and must use the exact versions the
// layer above was compiled against.
pub use egui;
