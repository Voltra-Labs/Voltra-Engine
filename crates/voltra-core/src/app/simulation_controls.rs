//! What a caller steers and reads of the simulation: the run switch, single
//! steps, and the last step's contacts, events and broken joints.

use voltra_physics::{CollisionEvent, Contact, PhysicsWorld};

use super::App;

impl App {
    /// Whether each frame runs the physics steps it owes.
    pub fn set_simulating(&mut self, simulating: bool) {
        self.simulation.set_running(simulating);
    }

    /// Whether the physics steps run each frame.
    pub fn is_simulating(&self) -> bool {
        self.simulation.is_running()
    }

    /// Runs `count` fixed steps on the next frame regardless of the switch.
    ///
    /// What a paused editor's Step button asks for. Additive, so two presses in
    /// one frame run two steps rather than one.
    pub fn request_steps(&mut self, count: u32) {
        self.simulation.request_steps(count);
    }

    /// The last physics step's contacts.
    pub fn contacts(&self) -> &[Contact] {
        self.physics_world.contacts()
    }

    /// What began and ended touching in the last physics step.
    pub fn events(&self) -> &[CollisionEvent] {
        self.physics_world.events()
    }

    /// The joints the last physics step broke.
    pub fn broken_joints(&self) -> &[voltra_physics::JointBroken] {
        self.physics_world.broken_joints()
    }

    /// The simulation, to tune or to reset after loading a scene.
    pub fn physics_mut(&mut self) -> &mut PhysicsWorld {
        &mut self.physics_world
    }
}
