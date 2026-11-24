//! An options screen every game can open: the display, the volumes and the
//! controls, from one prefab and a few calls.
//!
//! `assets/prefabs/options.prefab.ron` is the screen, built from stage 80's
//! controls — buttons, a toggle, sliders, a scrolling stack — and found by
//! the names of its parts, so a game restyles it by editing the file.
//! [`OptionsScreen`] opens it from a tick, fills in one slider per bus and
//! one row per action from the prefab's template rows, carries out what the
//! player changes as they change it (`tick.window`, the buses, the action
//! map), and keeps it when closed: the controls through
//! [`Tick::save_controls`](crate::Tick::save_controls), the rest as
//! [`Settings`] — both in the player's folder, `localStorage` on the web.
//! [`Settings::apply_saved`] puts the display and volumes back when the
//! game starts; the controls the loop lays over the map on its own.
//!
//! The shape of Unity's and Godot's sample options menus — a scene the game
//! instances, with a script behind it — rather than an engine-drawn dialog:
//! what every game copies should be a file it owns.

mod screen;
mod settings;

pub use screen::{OptionsConfig, OptionsError, OptionsScreen, Outcome};
pub use settings::{Settings, SettingsError, OPTIONS_FILE, VERSION};

/// The options screen's prefab, under the asset root.
pub const OPTIONS_PREFAB: &str = "prefabs/options.prefab.ron";
