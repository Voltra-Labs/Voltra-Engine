//! The platformer's own components, named in scene files and prefabs and
//! shown in the editor's inspector under these names.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;
use voltra_scene::ComponentRegistry;

/// The body the keys walk and jump.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Walker;

/// On the walker: the velocity it keeps between steps, which
/// `move_and_slide` answers and the next step starts from.
///
/// A component, and registered, so it lives through the editor reloading
/// the game's code mid-jump — state that must outlive an edit lives in the
/// world, not in the game's struct. Put there by the first step, so a level
/// file never holds one. The character's contact does not survive a
/// reload (it names entities by handle, which a rebuilt world renumbers):
/// the step after one finds the floor again, as after a level change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WalkerMotion {
    /// World units per second.
    pub velocity: Vec2,
}

/// Something to pick up, worth `value` on the counter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Coin {
    /// What it adds to the coin counter.
    pub value: u32,
}

impl Default for Coin {
    fn default() -> Self {
        Self { value: 1 }
    }
}

/// The flag at the end of a level: touching it loads `next`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Goal {
    /// A scene file under the asset root.
    pub next: String,
}

impl Default for Goal {
    fn default() -> Self {
        Self {
            next: "scenes/platformer-2.ron".to_owned(),
        }
    }
}

/// The text that shows the coins taken, and the count it shows.
///
/// The count lives here rather than in the game: a component is written
/// out and read back by everything that keeps a scene — a save, a play
/// session's snapshot, the editor reloading the game's code mid-play —
/// and the game's own struct is made afresh by the last of those.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoinCounter {
    /// What the coins taken were worth.
    pub taken: u32,
    /// What all the coins were worth when the run began; zero until then.
    pub total: u32,
}

/// The button that saves, as F5 does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct SaveButton;

/// The button that goes fullscreen and back, as F11 does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct FullscreenButton;

/// Names every component above, each but [`WalkerMotion`] with its default
/// as the template the editor's "Add component" starts from.
pub fn register(registry: &mut ComponentRegistry) {
    registry.register::<Walker>("Walker").with_template();
    // No template: the walker's step puts it there, not an author.
    registry.register::<WalkerMotion>("WalkerMotion");
    registry.register::<Coin>("Coin").with_template();
    registry.register::<Goal>("Goal").with_template();
    registry
        .register::<CoinCounter>("CoinCounter")
        .with_template();
    registry
        .register::<SaveButton>("SaveButton")
        .with_template();
    registry
        .register::<FullscreenButton>("FullscreenButton")
        .with_template();
}
