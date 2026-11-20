//! An entity that survives a scene change.

/// Marks a root entity — and everything parented under it — to be kept when
/// the scene is replaced: the player character carried into the next level,
/// a HUD, the music.
///
/// Unity's `DontDestroyOnLoad`, and like it a property of **roots**: a
/// `Persistent` child of a root that is not persistent goes with its parent,
/// since keeping a child whose parent was despawned would leave it hanging
/// off nothing. A marker rather than a flag on some other component, because
/// anything can need it — a sprite, a camera, an empty entity holding state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Persistent;
