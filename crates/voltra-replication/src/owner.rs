//! Which peer an entity belongs to.

use voltra_net::PeerId;

/// The peer a replicated entity belongs to: the player it is, or that
/// controls it. Set by the server's game on the entity it spawned for a
/// peer; replicated with the entity, so the client it names knows which
/// entity is its own and predicts it (stage 116), and every other client
/// draws it between snapshots.
///
/// Not saved in a scene: who plays is a property of a session, not of a
/// level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetOwner(pub PeerId);
