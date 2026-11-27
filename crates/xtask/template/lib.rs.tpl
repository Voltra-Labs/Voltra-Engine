//! {{title}}: the game the editor loads and plays, and the binary ships.
//!
//! A game is one value: its components named in `register`, its turns in
//! `update` (input edges, once a frame) and `fixed_update` (motion, once a
//! physics step). It acts on the scene it is given — `scenes/main.ron`,
//! edited in the editor — by asking the world for its components.

#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use voltra_core::{Game, Tick};
use voltra_render::glam::Vec2;
use voltra_scene::{ComponentRegistry, Transform2D};

/// The first scene, under the game's asset root.
pub const FIRST_SCENE: &str = "scenes/main.ron";

/// What the move actions steer, at `speed` world units a second.
///
/// State that must live through the editor reloading this code — a score,
/// a health bar — belongs on a component like this one: a scene keeps it,
/// and a reload carries the scene across.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Player {
    /// World units a second.
    pub speed: f32,
}

impl Player {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Player";
}

impl Default for Player {
    fn default() -> Self {
        Self { speed: 3.0 }
    }
}

/// The game. Made afresh for every play in the editor.
#[derive(Debug, Default)]
pub struct {{type}};

impl Game for {{type}} {
    fn register(&self, registry: &mut ComponentRegistry) {
        registry.register::<Player>(Player::NAME).with_template();
    }

    fn fixed_update(&mut self, tick: &mut Tick<'_>) {
        // From `assets/default.input.ron`: rebinding is an edit to that file.
        let direction = Vec2::new(
            tick.input.action_axis("move_left", "move_right"),
            tick.input.action_axis("move_down", "move_up"),
        )
        .clamp_length_max(1.0);
        let players: Vec<_> = tick
            .world
            .query::<Player>()
            .map(|(entity, player)| (entity, player.speed))
            .collect();
        for (entity, speed) in players {
            if let Some(transform) = tick.world.get_mut::<Transform2D>(entity) {
                transform.translation += direction * speed * tick.delta;
            }
        }
    }
}

voltra_core::export_game!({{type}});
