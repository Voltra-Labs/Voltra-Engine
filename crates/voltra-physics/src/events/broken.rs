//! A joint that broke under its load.

use voltra_ecs::Entity;

/// A joint loaded past its `break_force` or `break_torque` during a step.
///
/// By the time a game reads it, the [`Joint2D`] is gone from `entity` — the
/// shape Unity's breakable joints have (the component destroyed, then
/// `OnJointBreak2D`), where Box2D v3 only reports and leaves the destroying
/// to the caller: a joint left in place would hold again on the next step,
/// and report again. Raised once, in the step that broke it; a game that
/// wants the debris gone, or the joint back, does it from the event.
///
/// The load is measured as Box2D measures it: the impulse the solver needed
/// over the sub-step it acted in.
///
/// [`Joint2D`]: voltra_scene::Joint2D
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointBroken {
    /// The entity that carried the joint.
    pub entity: Entity,
    /// The first body it joined.
    pub a: Entity,
    /// The second, or `None` for the world.
    pub b: Option<Entity>,
    /// The force it held when it broke, in newtons.
    pub force: f32,
    /// The torque it held, in newton-metres, counter-clockwise on `a`.
    pub torque: f32,
}

impl JointBroken {
    /// Whether `body` is one the joint held: what a game asks when it cares
    /// about its own bridge, not every joint in the level.
    pub fn joined(&self, body: Entity) -> bool {
        self.a == body || self.b == Some(body)
    }
}
