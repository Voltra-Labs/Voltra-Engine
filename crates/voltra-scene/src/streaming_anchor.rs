//! What keeps a streamed world's regions loaded around it.

use serde::{Deserialize, Serialize};

/// Marks an entity the world's regions are loaded around: every region
/// within `radius` of where it stands is kept in, wherever every other
/// anchor is. Unreal's streaming source, which a player controller is by
/// default.
///
/// As many as a game needs: a split-screen pair, a server keeping the
/// world around every player it hosts, a cutscene's camera parked
/// somewhere the player is not. A world with none streams around its
/// cameras instead, so a single-player game need add nothing.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamingAnchor {
    /// How far around it regions are loaded, in world units; `None` takes
    /// the world file's `load_radius`.
    pub radius: Option<f32>,
}

impl StreamingAnchor {
    /// The name the registry stores it under.
    pub const NAME: &'static str = "StreamingAnchor";

    /// An anchor keeping regions within `radius` loaded.
    pub fn with_radius(radius: f32) -> Self {
        Self {
            radius: Some(radius),
        }
    }
}
