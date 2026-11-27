//! The mark of an entity a server's hit queries can rewind.

use serde::{Deserialize, Serialize};

/// Marks an entity whose collider a server remembers, pose by pose, for
/// lag compensation: a shot a client fired at what it saw is tested
/// against where this entity was at the moment the client was looking at,
/// not where it has got to since (`voltra-physics`' `rewind`).
///
/// A marker rather than every collider: remembering costs a copy of the
/// collider and its transform every server tick, and only what moves and
/// is shot at needs it — the players, not the walls. Unity Netcode's
/// `PhysicsWorldHistory` keeps every collider of its world; Source keeps
/// only players, and this keeps what the game marks. The entity needs a
/// `Collider` and a `Transform` too, and is usually [`Replicated`]: what
/// a client did not see it cannot have aimed at.
///
/// [`Replicated`]: crate::Replicated
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LagCompensated;

impl LagCompensated {
    /// The name the registry stores it under.
    pub const NAME: &'static str = "LagCompensated";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_and_writes_as_a_unit() {
        let text = ron::to_string(&LagCompensated).expect("writes");
        assert_eq!(text, "()");
        assert_eq!(
            ron::from_str::<LagCompensated>(&text).expect("reads"),
            LagCompensated
        );
    }
}
