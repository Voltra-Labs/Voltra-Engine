//! What an arena is made of: players, the coin they race for, the round.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

/// A player: how fast it moves and the box it keeps inside. Replicated,
/// so a client's prediction moves it exactly as the server does.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Player {
    /// World units a second.
    pub speed: f32,
    /// Half the arena, from the middle: the player stays inside.
    pub bounds: Vec2,
}

impl Player {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Player";
}

impl Default for Player {
    fn default() -> Self {
        Self {
            speed: 5.0,
            bounds: Vec2::new(8.0, 4.0),
        }
    }
}

/// A player's coins this round. Replicated: every end draws the scores.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score(pub u32);

impl Score {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Score";
}

/// The coin. Not replicated as a type — its `Transform2D` is — but marks the
/// entity on every end, spawned from the same prefab.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coin;

impl Coin {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Coin";
}

/// The round: coins to win, who won, and how long until the next round.
/// On the scene's replicated `Round` entity, so every client knows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Round {
    /// Coins that win a round.
    pub target: u32,
    /// The winner's peer number, once someone has won.
    pub winner: Option<u32>,
    /// Seconds the win is shown before the scores reset.
    pub pause: f32,
    /// Seconds left of that pause.
    pub left: f32,
    /// Rounds finished.
    pub finished: u32,
}

impl Round {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Round";
}

impl Default for Round {
    fn default() -> Self {
        Self {
            target: 5,
            winner: None,
            pause: 3.0,
            left: 0.0,
            finished: 0,
        }
    }
}
