//! Pulling a body with the pointer while the scene plays: a mouse joint.
//!
//! Box2D's testbed shape: a press on a dynamic body puts a mouse joint on
//! it, held at the point pressed and pulled towards the pointer with a force
//! a thousand times its mass, until the button is let go. The pull goes
//! through the solver, so the body swings, collides and drags what it is
//! joined to — where a gizmo drag, the editor's other play-mode handle,
//! teleports it. Unity and Godot have no such drag in their editors; their
//! play-mode handles are gizmos, which this editor also keeps.
//!
//! The joint lives on an entity of its own with no `SceneId`: transient,
//! never saved, never in a snapshot — and so not despawned by Stop's
//! restore either, which is why Stop lets go of it first.

use voltra_core::egui::{PointerButton, Response};
use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_physics::{query, QueryFilter};
use voltra_render::glam::Vec2;
use voltra_scene::{BodyType, Joint2D, JointKind, RigidBody, SceneId, Transform};

use crate::picking::world_at;

/// Newtons per kilogram the pull may use: Box2D's testbed's.
const FORCE_PER_KILOGRAM: f32 = 1000.0;

/// How stiff the pull is, and how damped: the testbed's, so a body follows
/// the pointer closely without ringing.
const HERTZ: f32 = 5.0;
const DAMPING_RATIO: f32 = 0.7;

/// Half the side of the box a press looks for a body in, in world units:
/// a point, for the query that wants a box.
const REACH: f32 = 1e-3;

/// The body being pulled, if one is.
#[derive(Debug, Default)]
pub(crate) struct Grab {
    /// The entity carrying the mouse joint.
    joint: Option<Entity>,
}

impl Grab {
    /// One frame of it: a primary drag that starts on a dynamic body while
    /// `playing` grabs it, the pointer moves the target, and letting go
    /// releases it. Returns whether it has the pointer, so the selection
    /// does not also take the drag as a box. `consumed`: whether the gizmo
    /// took the press first.
    pub(crate) fn update(
        &mut self,
        scene: &Response,
        frame: &mut UiFrame<'_>,
        consumed: bool,
        playing: bool,
    ) -> bool {
        if let Some(joint) = self.joint {
            let held = scene.dragged_by(PointerButton::Primary) && holds(frame.world, joint);
            if !held {
                self.release(frame.world);
                return false;
            }
            if let Some(pointer) = scene.interact_pointer_pos() {
                let target = world_at(scene, pointer, frame);
                if let Some(joint) = frame.world.get_mut::<Joint2D>(joint) {
                    joint.anchor_b = target;
                }
            }
            return true;
        }
        if consumed || !playing || !scene.drag_started_by(PointerButton::Primary) {
            return false;
        }
        // From where the press was, not where the drag threshold was crossed.
        let Some(pressed) = scene
            .ctx
            .input(|i| i.pointer.press_origin())
            .or(scene.interact_pointer_pos())
        else {
            return false;
        };
        let at = world_at(scene, pressed, frame);
        self.joint = grab(frame.world, at);
        self.joint.is_some()
    }

    /// Lets go of the body, if one is held: the joint's entity despawned.
    pub(crate) fn release(&mut self, world: &mut World) {
        if let Some(joint) = self.joint.take() {
            world.despawn(joint);
        }
    }

    /// Whether a body is held.
    #[cfg(test)]
    pub(crate) fn is_holding(&self) -> bool {
        self.joint.is_some()
    }
}

/// Whether the joint is still there and so is the body it pulls: a game may
/// despawn either, and a joint may break.
fn holds(world: &World, joint: Entity) -> bool {
    world
        .get::<Joint2D>(joint)
        .and_then(|joint| joint.a)
        .is_some_and(|id| voltra_scene::format::entity_with_id(world, id).is_some())
}

/// Puts a mouse joint on the dynamic body under `at`, held where `at` is on
/// it; the entity carrying it, or `None` with nothing there to pull.
fn grab(world: &mut World, at: Vec2) -> Option<Entity> {
    let reach = Vec2::splat(REACH);
    let body = query::overlap_aabb(world, at - reach, at + reach, QueryFilter::default())
        .into_iter()
        .find(|&entity| {
            world
                .get::<RigidBody>(entity)
                .is_some_and(|body| body.body_type == BodyType::Dynamic && body.inverse_mass > 0.0)
                && world.get::<SceneId>(entity).is_some()
        })?;
    let id = *world.get::<SceneId>(body)?;
    let mass = 1.0 / world.get::<RigidBody>(body)?.inverse_mass;
    let on_body = Joint2D::anchor_on(world.get::<Transform>(body)?, at);
    let joint = world.spawn();
    world.insert(
        joint,
        Joint2D {
            a: Some(id),
            anchor_a: on_body,
            anchor_b: at,
            kind: JointKind::Mouse {
                max_force: FORCE_PER_KILOGRAM * mass,
                hertz: HERTZ,
                damping_ratio: DAMPING_RATIO,
            },
            ..Joint2D::default()
        },
    );
    Some(joint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_physics::PhysicsWorld;
    use voltra_scene::Collider;

    fn crate_at(world: &mut World, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Transform::from_translation(at));
        world.insert(
            entity,
            Collider::Box {
                half_extents: Vec2::splat(0.5),
            },
        );
        world.insert(entity, RigidBody::new_dynamic(2.0));
        entity
    }

    #[test]
    fn a_press_on_a_body_pulls_it_where_the_pointer_goes() {
        let mut world = World::new();
        let body = crate_at(&mut world, Vec2::ZERO);
        let joint = grab(&mut world, Vec2::new(0.25, 0.0)).expect("a body is there");
        let held = world.get::<Joint2D>(joint).expect("the joint");
        assert_eq!(held.anchor_a, Vec2::new(0.25, 0.0), "held where pressed");
        assert!(world.get::<SceneId>(joint).is_none(), "transient");
        world.get_mut::<Joint2D>(joint).expect("joint").anchor_b = Vec2::new(3.25, 1.0);
        let mut physics = PhysicsWorld::new();
        for _ in 0..120 {
            physics.step_once(&mut world, Vec2::new(0.0, -10.0));
        }
        // Where it was pressed is where the pointer is; the rest of it
        // hangs from there as it likes.
        let grip = Joint2D::anchor_in_world(
            world.get::<Transform>(body).expect("body"),
            Vec2::new(0.25, 0.0),
        );
        assert!(
            (grip - Vec2::new(3.25, 1.0)).length() < 0.05,
            "pulled along: {grip}"
        );
    }

    #[test]
    fn a_scaled_body_is_gripped_where_it_was_pressed() {
        let mut world = World::new();
        let body = crate_at(&mut world, Vec2::ZERO);
        world.get_mut::<Transform>(body).expect("placed").scale = Vec2::splat(2.0);
        let at = Vec2::new(0.4, 0.0);
        let joint = grab(&mut world, at).expect("a body is there");
        let held = world.get::<Joint2D>(joint).expect("the joint");
        // The solver turns an anchor with the body and never scales it: an
        // anchor at half the offset would be gripped at half the distance.
        assert!((held.anchor_a - at).length() < 1e-6, "{}", held.anchor_a);
        world.get_mut::<Joint2D>(joint).expect("joint").anchor_b = Vec2::new(2.4, 1.0);
        let mut physics = PhysicsWorld::new();
        for _ in 0..120 {
            physics.step_once(&mut world, Vec2::new(0.0, -10.0));
        }
        let grip = Joint2D::anchor_in_world(world.get::<Transform>(body).expect("body"), at);
        assert!(
            (grip - Vec2::new(2.4, 1.0)).length() < 0.05,
            "the pressed point follows the pointer: {grip}"
        );
    }

    #[test]
    fn empty_ground_and_a_static_body_grab_nothing() {
        let mut world = World::new();
        let floor = crate_at(&mut world, Vec2::ZERO);
        world.insert(floor, RigidBody::new_static());
        assert!(grab(&mut world, Vec2::ZERO).is_none());
        assert!(grab(&mut world, Vec2::new(5.0, 0.0)).is_none());
    }

    #[test]
    fn a_despawned_body_is_let_go() {
        let mut world = World::new();
        let body = crate_at(&mut world, Vec2::ZERO);
        let joint = grab(&mut world, Vec2::ZERO).expect("grabbed");
        assert!(holds(&world, joint));
        world.despawn(body);
        assert!(!holds(&world, joint), "nothing left to pull");
        let mut grab = Grab { joint: Some(joint) };
        grab.release(&mut world);
        assert!(!world.is_alive(joint) && !grab.is_holding());
    }
}
