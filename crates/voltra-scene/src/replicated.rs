//! The mark of an entity the server sends its clients.

use serde::{Deserialize, Serialize};

/// Marks an entity the server replicates: its replicated components (see
/// [`ComponentRegistry::replicate`](crate::ComponentRegistry::replicate))
/// go to every client it is relevant to, and it is spawned and despawned
/// there with it.
///
/// A component rather than "every entity with an id", as Unity's
/// `NetworkObject`, Godot's `MultiplayerSynchronizer` and Bevy replicon's
/// `Replicated` are: the level's walls, loaded from the same file on each
/// end, should cost nothing on the wire.
///
/// With interest management on (`ReplicationConfig::interest`), a client is
/// sent only the entities near what it watches; the two relevance fields
/// say how this one takes part. A child of another replicated entity
/// follows its replicated root and ignores both.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Replicated {
    /// How much this entity's updates matter next to the others' when a
    /// client's bandwidth budget cannot carry them all in one snapshot:
    /// each tick an entity waits, its priority is added to what it has
    /// banked, and the most banked go first. `1.0` by default; a player
    /// might be `4.0`, a distant prop `0.25`.
    pub priority: f32,
    /// Sent to every client wherever it is, as with no interest
    /// management: the match's state, a scoreboard, an objective every
    /// player is shown. Unreal's `bAlwaysRelevant`. Off by default.
    #[serde(skip_serializing_if = "is_false")]
    pub always_relevant: bool,
    /// Its own relevance radius in place of the session's: a beacon seen
    /// from across the map, a spark only next to it. `None` (the default)
    /// is the session's. Unreal's per-actor `NetCullDistanceSquared`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relevance_radius: Option<f32>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Replicated {
    /// The name the registry stores it under.
    pub const NAME: &'static str = "Replicated";

    /// Replicated at `priority`.
    pub fn with_priority(priority: f32) -> Self {
        Self {
            priority,
            ..Self::default()
        }
    }

    /// The same, sent to every client wherever it is.
    pub fn always_relevant(self) -> Self {
        Self {
            always_relevant: true,
            ..self
        }
    }

    /// The same, relevant within `radius` instead of the session's radius.
    pub fn with_relevance_radius(self, radius: f32) -> Self {
        Self {
            relevance_radius: Some(radius),
            ..self
        }
    }
}

impl Default for Replicated {
    fn default() -> Self {
        Self {
            priority: 1.0,
            always_relevant: false,
            relevance_radius: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_written_before_relevance_reads_and_writes_back_unchanged() {
        let read: Replicated = ron::from_str("(priority: 2.0)").expect("reads");
        assert_eq!(read, Replicated::with_priority(2.0));
        assert_eq!(ron::to_string(&read).expect("writes"), "(priority:2.0)");

        let both = Replicated::default()
            .always_relevant()
            .with_relevance_radius(40.0);
        let text = ron::to_string(&both).expect("writes");
        assert_eq!(ron::from_str::<Replicated>(&text).expect("reads"), both);
    }
}
