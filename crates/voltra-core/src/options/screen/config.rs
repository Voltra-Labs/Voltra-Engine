//! What a game tells the options screen, what can stop it opening, and
//! what a frame of it answers.

use std::fmt;

use voltra_scene::SceneError;

use crate::options::OPTIONS_PREFAB;

/// What the screen shows, and of what.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionsConfig {
    /// The prefab, under the asset root: [`OPTIONS_PREFAB`] unless a game
    /// has its own.
    pub prefab: String,
    /// The actions the controls list offers to rebind, each with what the
    /// row calls it. Empty: every action of the map but the `ui_` ones,
    /// named after themselves.
    pub actions: Vec<(String, String)>,
    /// The buses given a volume slider, in order. `None`: every bus.
    pub buses: Option<Vec<String>>,
}

impl Default for OptionsConfig {
    fn default() -> Self {
        Self {
            prefab: OPTIONS_PREFAB.to_owned(),
            actions: Vec::new(),
            buses: None,
        }
    }
}

impl OptionsConfig {
    /// The default screen, listing `actions` — `(action, shown as)` — to
    /// rebind.
    pub fn with_actions(actions: &[(&str, &str)]) -> Self {
        Self {
            actions: actions
                .iter()
                .map(|(action, title)| ((*action).to_owned(), (*title).to_owned()))
                .collect(),
            ..Self::default()
        }
    }
}

/// Why the screen could not open.
#[derive(Debug)]
pub enum OptionsError {
    /// The prefab path is not an asset path.
    BadPath(String),
    /// The prefab would not spawn.
    Spawn(SceneError),
    /// The prefab spawned nothing: its file is missing or unreadable (the
    /// store has said why).
    Empty(String),
}

impl fmt::Display for OptionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadPath(path) => write!(f, "{path:?} is not an asset path"),
            Self::Spawn(e) => write!(f, "the options screen would not spawn: {e}"),
            Self::Empty(path) => write!(f, "{path} holds no options screen"),
        }
    }
}

impl std::error::Error for OptionsError {}

/// Whether the screen is still open after a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Still showing.
    Open,
    /// Closed — by Back or the cancel action — saved and taken away.
    Closed,
}
