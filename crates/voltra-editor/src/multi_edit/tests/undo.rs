//! The undo entry a multi-edit makes.

use voltra_ecs::{Entity, World};
use voltra_scene::format::{entity_with_id, ComponentRegistry};
use voltra_scene::{Collider2D, SceneId, Sprite2D};

use super::{begin, three, BLUE, GREEN, RED};
use crate::gizmo::Gizmo;
use crate::selection::Selection;
use crate::undo::{History, SceneView, UndoContext, UndoHost};

/// An [`UndoHost`] with no `App` and no GPU behind it.
struct Host(World);

impl UndoHost for Host {
    fn world(&mut self) -> &mut World {
        &mut self.0
    }
    fn reset_physics(&mut self) {}
    fn resolve_scene_assets(&mut self) {}
}

#[test]
fn a_drag_across_three_entities_is_one_undo_step() {
    let registry = ComponentRegistry::with_defaults();
    let (world, selected) = three([RED, GREEN, BLUE]);
    let mut host = Host(world);
    let mut selection = Selection::default();
    selected.iter().for_each(|&e| selection.add(e));
    let ids: Vec<SceneId> = selection.ids(&host.0);
    let mut history = History::default();

    // Three frames of one drag on the primary's sort order.
    for order in 1..=3 {
        let view = SceneView {
            world: &host.0,
            registry: &registry,
            selected: ids.clone(),
        };
        history.watch(view, ids.iter().copied());
        let edit = begin(&host.0, &registry, &selected);
        host.0
            .get_mut::<Sprite2D>(selected[2])
            .expect("s")
            .sort_order = order;
        edit.finish(&mut host.0, &registry);
        history.claim("Set sort order");
        let view = SceneView {
            world: &host.0,
            registry: &registry,
            selected: ids.clone(),
        };
        history.end_frame(view);
    }
    // Let go: a frame with no claim closes it.
    let view = SceneView {
        world: &host.0,
        registry: &registry,
        selected: ids.clone(),
    };
    history.watch(view, ids.iter().copied());
    let view = SceneView {
        world: &host.0,
        registry: &registry,
        selected: ids.clone(),
    };
    history.end_frame(view);

    assert_eq!(history.len(), 1, "one step for the drag and the three");
    for &id in &ids {
        let entity = entity_with_id(&host.0, id).expect("there");
        assert_eq!(host.0.get::<Sprite2D>(entity).expect("s").sort_order, 3);
    }

    let mut gizmo = Gizmo::default();
    let ctx = UndoContext {
        registry: &registry,
        selected: &mut selection,
        gizmo: &mut gizmo,
    };
    assert!(history.undo(ctx, &mut host));
    for (&id, color) in ids.iter().zip([RED, GREEN, BLUE]) {
        let entity = entity_with_id(&host.0, id).expect("there");
        let sprite = host.0.get::<Sprite2D>(entity).expect("s");
        assert_eq!((sprite.sort_order, sprite.color), (0, color));
    }
}

/// One frame of the editor: watched, edited by `edit` on the primary,
/// written across, recorded.
fn frame(
    history: &mut History,
    host: &mut Host,
    registry: &ComponentRegistry,
    selected: &[Entity],
    ids: &[SceneId],
    edit: impl FnOnce(&mut World, Entity),
) {
    let view = SceneView {
        world: &host.0,
        registry,
        selected: ids.to_vec(),
    };
    history.watch(view, ids.iter().copied());
    let multi = begin(&host.0, registry, selected);
    edit(&mut host.0, selected[2]);
    multi.finish(&mut host.0, registry);
    let view = SceneView {
        world: &host.0,
        registry,
        selected: ids.to_vec(),
    };
    history.end_frame(view);
}

#[test]
fn a_component_added_to_all_is_undone_from_all_and_redone_on_all() {
    let registry = ComponentRegistry::with_defaults();
    let (world, selected) = three([RED, GREEN, BLUE]);
    let mut host = Host(world);
    let mut selection = Selection::default();
    selected.iter().for_each(|&e| selection.add(e));
    let ids = selection.ids(&host.0);
    let mut history = History::default();
    let circle = Collider2D::Circle { radius: 1.5 };
    frame(
        &mut history,
        &mut host,
        &registry,
        &selected,
        &ids,
        |w, p| {
            w.insert(p, circle);
        },
    );
    assert_eq!(history.len(), 1);

    let mut gizmo = Gizmo::default();
    let held = |host: &Host| {
        ids.iter()
            .map(|&id| {
                let e = entity_with_id(&host.0, id).expect("there");
                host.0.get::<Collider2D>(e).copied()
            })
            .collect::<Vec<_>>()
    };
    let ctx = UndoContext {
        registry: &registry,
        selected: &mut selection,
        gizmo: &mut gizmo,
    };
    assert!(history.undo(ctx, &mut host));
    assert_eq!(held(&host), [None, None, None]);
    let ctx = UndoContext {
        registry: &registry,
        selected: &mut selection,
        gizmo: &mut gizmo,
    };
    assert!(history.redo(ctx, &mut host));
    assert_eq!(held(&host), [Some(circle); 3]);
}
