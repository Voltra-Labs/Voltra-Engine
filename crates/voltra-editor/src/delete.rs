//! Deleting the selection — every selected entity and everything under
//! each — as one undo entry, from the inspector's button or the Delete key.

use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, prefab, ComponentRegistry, Prefabs, SceneId};

use crate::editor::Editor;
use crate::selection::Selection;
use crate::undo::{History, SceneView};

/// Deletes the editor's selection.
pub(crate) fn selection(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    delete(
        frame.world,
        &editor.registry,
        frame.prefabs,
        &mut editor.history,
        &mut editor.selection,
    );
}

/// Deletes `selection` from `world`, recorded in `history`, and empties it.
/// Nothing selected, or nothing of it alive, records nothing.
fn delete(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    history: &mut History,
    selection: &mut Selection,
) {
    selection.retain_alive(world);
    let roots: Vec<Entity> = selection.iter().collect();
    if roots.is_empty() {
        return;
    }
    // Everything under each goes too, so everything under each has to be in
    // the entry — an undo that revived a parent alone would leave its
    // children deleted for good. A child selected with its parent is named
    // twice; the history keys by id, so once is what it records.
    let mut going: Vec<SceneId> = roots
        .iter()
        .flat_map(|&root| std::iter::once(root).chain(hierarchy::descendants(world, root)))
        .filter_map(|e| world.get::<SceneId>(e).copied())
        .collect();
    // A part deleted from its instance is remembered on the instance's root,
    // so the next sync does not spawn it back: that root is in the entry too.
    going.extend(
        roots
            .iter()
            .filter_map(|&root| prefab::instance_root(world, prefabs, root).filter(|&r| r != root))
            .filter_map(|r| world.get::<SceneId>(r).copied()),
    );
    // Explicit rather than watched: the watcher's `after` is read at the end
    // of the frame from entities that no longer exist, which is right, but
    // the selection has to be recorded as it was *before* the despawn
    // cleared it.
    let view = SceneView {
        world,
        registry,
        selected: selection.ids(world),
    };
    history.begin("Delete", view, going);

    for root in roots {
        // A child selected with its parent went with it already.
        if world.is_alive(root) {
            prefab::note_removed(world, prefabs, root);
            hierarchy::despawn_recursive(world, root);
        }
    }
    selection.clear();

    let view = SceneView {
        world,
        registry,
        selected: Vec::new(),
    };
    history.commit(view);
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::Vec2;
    use voltra_scene::format::entity_with_id;
    use voltra_scene::Transform;

    use super::*;
    use crate::gizmo::Gizmo;
    use crate::undo::{UndoContext, UndoHost};

    struct Host(World);

    impl UndoHost for Host {
        fn world(&mut self) -> &mut World {
            &mut self.0
        }
        fn reset_physics(&mut self) {}
        fn resolve_scene_assets(&mut self) {}
    }

    fn spawn(world: &mut World, x: f32) -> Entity {
        let e = world.spawn();
        world.insert(e, SceneId::new());
        world.insert(e, Transform::from_translation(Vec2::new(x, 0.0)));
        e
    }

    #[test]
    fn the_whole_selection_goes_as_one_entry_and_comes_back_selected() {
        let registry = ComponentRegistry::with_defaults();
        let mut history = History::default();
        let mut host = Host(World::new());
        let a = spawn(&mut host.0, 1.0);
        let b = spawn(&mut host.0, 2.0);
        let child = spawn(&mut host.0, 3.0);
        let keep = spawn(&mut host.0, 4.0);
        hierarchy::set_parent(&mut host.0, child, b).expect("no cycle");
        let ids: Vec<SceneId> = [a, b, child, keep]
            .map(|e| *host.0.get::<SceneId>(e).expect("an id"))
            .into();
        let mut selection = Selection::default();
        selection.add(a);
        selection.add(b);

        delete(
            &mut host.0,
            &registry,
            &mut Prefabs::new(std::env::temp_dir()),
            &mut history,
            &mut selection,
        );
        assert!(selection.is_empty());
        assert_eq!(history.len(), 1, "one entry for both");
        for &id in &ids[..3] {
            assert!(entity_with_id(&host.0, id).is_none(), "gone, child too");
        }
        assert!(entity_with_id(&host.0, ids[3]).is_some());

        let mut gizmo = Gizmo::default();
        let ctx = UndoContext {
            registry: &registry,
            selected: &mut selection,
            gizmo: &mut gizmo,
        };
        assert!(history.undo(ctx, &mut host));
        for &id in &ids[..3] {
            assert!(entity_with_id(&host.0, id).is_some(), "back");
        }
        let back: Vec<SceneId> = selection.ids(&host.0);
        assert_eq!(back, [ids[0], ids[1]], "selected as they were, in order");
    }

    #[test]
    fn nothing_selected_records_nothing() {
        let registry = ComponentRegistry::with_defaults();
        let mut history = History::default();
        let mut world = World::new();
        spawn(&mut world, 0.0);
        delete(
            &mut world,
            &registry,
            &mut Prefabs::new(std::env::temp_dir()),
            &mut history,
            &mut Selection::default(),
        );
        assert!(history.is_empty());
    }

    #[test]
    fn a_part_deleted_from_its_instance_is_remembered_in_the_same_entry() {
        use voltra_assets::AssetPath;
        use voltra_scene::{PrefabInstance, PrefabPart};
        let registry = ComponentRegistry::with_defaults();
        let mut history = History::default();
        let mut host = Host(World::new());
        let root = spawn(&mut host.0, 0.0);
        let part = spawn(&mut host.0, 1.0);
        hierarchy::set_parent(&mut host.0, part, root).expect("no cycle");
        host.0.insert(
            root,
            PrefabInstance::new(AssetPath::new("coin.ron").expect("valid")),
        );
        let record = SceneId::new();
        host.0.insert(part, PrefabPart::new(record));
        let root_id = *host.0.get::<SceneId>(root).expect("an id");

        let mut selection = Selection::default();
        selection.add(part);
        delete(
            &mut host.0,
            &registry,
            &mut Prefabs::new(std::env::temp_dir()),
            &mut history,
            &mut selection,
        );
        let removed = |world: &World| {
            entity_with_id(world, root_id)
                .and_then(|r| world.get::<PrefabInstance>(r).map(|l| l.removed.clone()))
                .expect("the root stays")
        };
        assert_eq!(removed(&host.0), [record]);

        let mut gizmo = Gizmo::default();
        let ctx = UndoContext {
            registry: &registry,
            selected: &mut selection,
            gizmo: &mut gizmo,
        };
        assert!(history.undo(ctx, &mut host));
        assert!(removed(&host.0).is_empty(), "undone with the delete");
    }
}
