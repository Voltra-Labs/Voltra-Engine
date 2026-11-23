//! The platformer's own components, named in scene files and prefabs and
//! shown in the editor's inspector under these names.

use serde::{Deserialize, Serialize};
use voltra_scene::ComponentRegistry;

/// The body the keys walk and jump.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Walker;

/// Something to pick up, worth `value` on the counter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Coin {
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

/// The text that shows the coins taken.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CoinCounter;

/// The button that saves, as F5 does.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct SaveButton;

/// Names every component above, each with its default as the template the
/// editor's "Add component" starts from.
pub fn register(registry: &mut ComponentRegistry) {
    registry.register::<Walker>("Walker").with_template();
    registry.register::<Coin>("Coin").with_template();
    registry.register::<Goal>("Goal").with_template();
    registry
        .register::<CoinCounter>("CoinCounter")
        .with_template();
    registry
        .register::<SaveButton>("SaveButton")
        .with_template();
}
