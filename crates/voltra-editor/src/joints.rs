//! Making joints in the editor: the selection joined, and a pendulum to see
//! one swing. Each one undo entry.

use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{hierarchy, Collider, Joint2D, JointKind, Name, RigidBody, SceneId, Transform};

use crate::editor::Editor;
use crate::spawn;
use crate::undo::SceneView;

/// Whether the solver would take `entity` as a body: a rigid body, or a
/// collider, which is immovable.
fn is_body(world: &World, entity: Entity) -> bool {
    world.get::<RigidBody>(entity).is_some() || world.get::<Collider>(entity).is_some()
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
    let at = hierarchy::world_transform(frame.world, primary);
    let (b, anchor_b, angle_b) = match other {
        Some(other) => {
            let placed = hierarchy::world_transform(frame.world, other);
            let local = placed.matrix().inverse().transform_point2(at.translation);
            (
                frame.world.get::<SceneId>(other).copied(),
                local,
                placed.rotation,
            )
        }
        None => (None, at.translation, 0.0),
    };
    let own = frame.world.get::<Joint2D>(primary).is_none();
    let joint = Joint2D {
        a: if own {
            None
        } else {
            frame.world.get::<SceneId>(primary).copied()
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
    };

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
            .insert(holder, Transform::from_translation(at.translation));
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
            RigidBody::new_dynamic(1.0),
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
