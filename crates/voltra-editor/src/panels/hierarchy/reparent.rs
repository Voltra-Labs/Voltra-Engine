//! A row dropped onto another, or below the tree: the reparent the
//! hierarchy's drag asks for, applied once the walk is over.

use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::hierarchy;
use voltra_scene::SceneId;

use crate::editor::Editor;
use crate::undo::SceneView;

/// A drop the tree asked for, applied once the walk is over.
pub(super) struct Reparent {
    pub(super) child: Entity,
    /// `None` drops the entity at the top level.
    pub(super) parent: Option<Entity>,
}

/// Performs a drop, as one undo entry.
///
/// The entity keeps its **world** placement across the move, which is what
/// Unity, Unreal and Godot all do when a row is dragged in the hierarchy:
/// dropping a sprite onto another one is a statement about what it belongs to,
/// not a request to teleport it into the parent's frame. The primitive in
/// `voltra-scene` does the opposite and keeps the local values — that is the
/// one that cannot lose data — so the placement is captured and put back here.
pub(super) fn apply(editor: &mut Editor, frame: &mut UiFrame<'_>, reparent: Reparent) {
    let Reparent { child, parent } = reparent;
    if !frame.world.is_alive(child) {
        return;
    }
    let Some(id) = frame.world.get::<SceneId>(child).copied() else {
        return;
    };

    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: vec![id],
    };
    editor.history.begin("Reparent", view, [id]);

    let placement = hierarchy::world_transform_2d(frame.world, child);
    let outcome = match parent {
        Some(parent) => hierarchy::set_parent(frame.world, child, parent),
        None => {
            hierarchy::clear_parent(frame.world, child);
            Ok(())
        }
    };

    match outcome {
        Ok(()) => {
            hierarchy::set_world_transform_2d(frame.world, child, &placement);
            // The dragged row, not whatever was selected before: the drag says
            // which entity the user is thinking about.
            editor.selection.set(Some(child));
        }
        // Refused, and every reason is a rule rather than a failure — a cycle,
        // an entity without an identity, a physics body. A drop that silently
        // does nothing reads as a broken panel, so it is reported; the commit
        // below records no change, so the history stays clean.
        Err(error) => log::warn!("reparent refused: {error}"),
    }

    let view = SceneView {
        world: frame.world,
        registry: &editor.registry,
        selected: vec![id],
    };
    editor.history.commit(view);
}
