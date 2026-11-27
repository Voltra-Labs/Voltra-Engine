//! Making joints in the editor: the selection joined, and a pendulum to see
//! one swing. Each one undo entry.

use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{
    hierarchy, Collider2D, Joint2D, JointKind, Name, RigidBody2D, SceneId, Transform2D,
};

use crate::editor::Editor;
use crate::spawn;
use crate::undo::SceneView;

/// Whether the solver would take `entity` as a body: a rigid body, or a
/// collider, which is immovable.
fn is_body(world: &World, entity: Entity) -> bool {
    world.get::<RigidBody2D>(entity).is_some() || world.get::<Collider2D>(entity).is_some()
}

/// The joint `join_selection` would make, and whether it can: the primary
/// is a body, and at most one other selected entity is.
pub(crate) fn can_join(editor: &Editor, world: &World) -> bool {
    let Some(primary) = editor.selection.live_primary(world) else {
        return false;
    };
    let others = editor
        .selection
        .iter()
        .filter(|&e| e != primary && world.is_alive(e) && is_body(world, e))
        .count();
    is_body(world, primary) && others <= 1
}

/// Joins the selection with a hinge at the primary's centre: to the other
/// selected body if there is one, to the world if not. The joint goes on
/// the primary — or, if it already carries one, on a new entity of its own
/// at the hinge, naming both — and starts at the angle the two stand at, so
/// nothing moves until the physics does.
pub(crate) fn join_selection(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    if !can_join(editor, frame.world) {
        return;
    }
    let Some(primary) = editor.selection.live_primary(frame.world) else {
        return;
    };
    let other = editor
        .selection
        .iter()
        .find(|&e| e != primary && frame.world.is_alive(e) && is_body(frame.world, e));
    let at = hierarchy::world_transform_2d(frame.world, primary);
    let own = frame.world.get::<Joint2D>(primary).is_none();
    let joint = hinge(frame.world, primary, other, own);

    let touched: Vec<SceneId> = frame
        .world
        .get::<SceneId>(primary)
        .copied()
        .into_iter()
        .collect();
    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: editor.selection.ids(frame.world),
    };
    editor.history.begin("Join", view, touched);
    let mut made = Vec::new();
    if own {
        frame.world.insert(primary, joint);
    } else {
        let holder = frame.world.spawn();
        let id = SceneId::new();
        frame.world.insert(holder, id);
        frame.world.insert(holder, Name::new("Joint"));
        frame
            .world
            .insert(holder, Transform2D::from_translation(at.translation));
        frame.world.insert(holder, joint);
        editor.selection.set(Some(holder));
        made.push(id);
    }
    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: editor.selection.ids(frame.world),
    };
    editor.history.commit_including(view, made);
}

/// A hinge at `primary`'s centre, to `other` or the world, at the angle
/// the two stand at; naming `primary` as `a` unless the joint goes `on` it.
fn hinge(world: &World, primary: Entity, other: Option<Entity>, on: bool) -> Joint2D {
    let at = hierarchy::world_transform_2d(world, primary);
    let (b, anchor_b, angle_b) = match other {
        Some(other) => {
            let placed = hierarchy::world_transform_2d(world, other);
            (
                world.get::<SceneId>(other).copied(),
                // In `other`'s space as the solver holds it: unscaled.
                Joint2D::anchor_on(&placed, at.translation),
                placed.rotation,
            )
        }
        None => (None, at.translation, 0.0),
    };
    Joint2D {
        a: if on {
            None
        } else {
            world.get::<SceneId>(primary).copied()
        },
        b,
        anchor_a: Vec2::ZERO,
        anchor_b,
        kind: JointKind::Revolute {
            reference_angle: at.rotation - angle_b,
            limit: None,
            motor: None,
            spring: None,
        },
        ..Joint2D::default()
    }
}

/// Spawns a ball hung from the world on a rod, swinging from the side.
pub(crate) fn spawn_pendulum(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    spawn::record(editor, frame, "Spawn pendulum", |frame| {
        let pivot = Vec2::new(0.0, 1.5);
        let bob = spawn::body(
            frame,
            "Pendulum",
            pivot + Vec2::new(1.2, 0.0),
            Vec2::splat(0.3),
            0.0,
            RigidBody2D::new_dynamic(1.0),
            [0.4, 0.8, 1.0, 1.0],
        );
        frame.world.insert(
            bob,
            Joint2D {
                anchor_b: pivot,
                kind: JointKind::rod(1.2),
                ..Joint2D::default()
            },
        );
        bob
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hinge_to_a_scaled_body_holds_where_the_two_meet() {
        let mut world = World::new();
        let door = world.spawn();
        world.insert(door, Transform2D::from_translation(Vec2::new(1.0, 0.0)));
        world.insert(door, RigidBody2D::new_dynamic(1.0));
        let frame = world.spawn();
        let id = SceneId::new();
        world.insert(frame, id);
        world.insert(
            frame,
            Transform2D::from_translation(Vec2::new(0.0, 1.0)).with_scale(Vec2::splat(2.0)),
        );
        world.insert(frame, RigidBody2D::new_static());
        let joint = hinge(&world, door, Some(frame), true);
        assert_eq!(joint.b, Some(id));
        // The door's centre in the frame's space, unscaled: what the solver
        // turns back into the world at the door, not halfway to it.
        let placed = world.get::<Transform2D>(frame).expect("placed");
        let held = Joint2D::anchor_in_world(placed, joint.anchor_b);
        assert!((held - Vec2::new(1.0, 0.0)).length() < 1e-5, "{held}");
        assert!((joint.anchor_b - Vec2::new(1.0, -1.0)).length() < 1e-5);
    }
}
