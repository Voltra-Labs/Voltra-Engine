//! What the physics steps reported that the per-frame tick has not been
//! handed yet.

use voltra_physics::{CollisionEvent, JointBroken, PhysicsWorld};

/// Every step's events since the per-frame tick's last turn.
///
/// A frame can owe two steps, and `PhysicsWorld` only keeps the last
/// step's: without this the per-frame tick would miss the pickup taken, or
/// the bridge broken, on the first of them. Emptied once the tick has had
/// it.
#[derive(Debug, Default)]
pub(super) struct PendingPhysics {
    /// What began and ended touching.
    pub(super) collisions: Vec<CollisionEvent>,
    /// The joints that broke.
    pub(super) broken: Vec<JointBroken>,
}

impl PendingPhysics {
    /// Adds what the step `physics` just ran reported.
    pub(super) fn gather(&mut self, physics: &PhysicsWorld) {
        self.collisions.extend_from_slice(physics.events());
        self.broken.extend_from_slice(physics.broken_joints());
    }

    /// Forgets everything: handed out, or naming a world that is gone.
    pub(super) fn clear(&mut self) {
        self.collisions.clear();
        self.broken.clear();
    }
}
