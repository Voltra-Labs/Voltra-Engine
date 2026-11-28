//! Platform layer: owns the event loop, the OS window, and the frame tick.
//!
//! Everything GPU-side lives in `voltra-render`; this crate is the only place
//! allowed to depend on `winit`.

#![warn(missing_docs)]

pub mod app;
pub mod game;
pub mod input;
pub mod lobby;
pub mod logging;
pub mod options;
mod prefab_parts;
pub mod saves;
pub mod start;
mod storage;
pub mod time;
pub mod ui;
pub mod window;

pub use app::draw::render_settings::{
    parse_render_settings, RenderSettings, DEFAULT_RENDER_SETTINGS,
};
pub use app::headless::{Headless, HeadlessError, Screenshot, ScreenshotError};
pub use app::{
    parse_bus_layout, parse_sorting_layers, App, LoadStats, Net, SceneChangeStatus, SceneLoad,
    StreamingStats, Tick, UiFrame, DEFAULT_BUS_LAYOUT, DEFAULT_INPUT_MAP, DEFAULT_LOAD_BUDGET,
    DEFAULT_SORTING_LAYERS, NET_COUNTERS,
};
pub use game::Game;
pub use input::{
    ActionMap, ActionState, Binding, Direction, Input, MouseButton, PadAxis, PadButton,
};
pub use saves::{SaveError, Saves};
pub use start::when_assets_ready;
pub use time::{Clock, Timestep};
pub use ui::EguiLayer;
pub use window::{Fullscreen, GameWindow, Monitor, VideoMode, WindowConfig};

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
// The network a game hosts or joins through `Tick::net`: its transports,
// channels, peers and events, named without a second dependency.
pub use voltra_net as net;
// What the server sends its clients, and the prediction a game steps.
pub use voltra_physics::{
    character, move_and_slide, query, rewind, CollisionEvent, Contact, JointBroken, QueryFilter,
    RayHit, Rewound, ShapeHit, Slide, Touch,
};
pub use voltra_replication as replication;

// A game plays a sound through `Tick::audio` and has to be able to name what
// it hands over and what comes back. Re-exported for the reason the physics
// types above are: one dependency for a game that wants a coin to make a
// noise, not two.
pub use voltra_audio::{
    effect, Audio, BusId, BusSettings, Buses, Clip, EffectDesc, PlayParams, VoiceId,
};
// A game spawns prefabs through `Tick::spawn` and gets back an `Instance`.
pub use voltra_scene::{Instance, Prefab};

// The editor builds its panels with these, and must use the exact versions the
// layer above was compiled against.
pub use egui;
