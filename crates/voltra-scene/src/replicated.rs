//! The mark of an entity the server sends its clients.

use serde::{Deserialize, Serialize};

/// Marks an entity the server replicates: its replicated components (see
/// [`ComponentRegistry::replicate`](crate::ComponentRegistry::replicate))
/// go to every client, and it is spawned and despawned there with it.
///
/// A component rather than "every entity with an id", as Unity's
/// `NetworkObject`, Godot's `MultiplayerSynchronizer` and Bevy replicon's
/// `Replicated` are: the level's walls, loaded from the same file on each
/// end, should cost nothing on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Replicated {
    /// How much this entity's updates matter next to the others' when a
    /// client's bandwidth budget cannot carry them all in one snapshot:
    /// each tick an entity waits, its priority is added to what it has
    /// banked, and the most banked go first. `1.0` by default; a player
    /// might be `4.0`, a distant prop `0.25`.
    pub priority: f32,
}

impl Replicated {
    /// The name the registry stores it under.
    pub const NAME: &'static str = "Replicated";

    /// Replicated at `priority`.
    pub fn with_priority(priority: f32) -> Self {
        Self { priority }
    }
}

impl Default for Replicated {
    fn default() -> Self {
        Self { priority: 1.0 }
    }
}
