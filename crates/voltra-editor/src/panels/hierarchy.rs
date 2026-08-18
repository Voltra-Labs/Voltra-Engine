//! Left panel: the scene as a tree, and which entity is selected.
//!
//! Rows are drag sources and drop targets both, which is how every engine's
//! hierarchy reparents: drop onto a row to become its child, drop below the
//! tree to become a root. The drop is applied *after* the tree is drawn — a
//! reparent while walking the links would change the list being walked.

use std::collections::HashSet;

use voltra_core::egui::{self, Frame, Id, RichText, Ui};
use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_scene::hierarchy;
use voltra_scene::{Name, SceneId};

use crate::editor::Editor;
use crate::undo::SceneView;

/// How far one level of nesting is indented, in points.
///
/// Enough to read the shape of a deep tree in a 200-point panel, and not so
/// much that four levels push a name off the edge.
const INDENT: f32 = 14.0;

/// A drop the tree asked for, applied once the walk is over.
struct Reparent {
    child: Entity,
    /// `None` drops the entity at the top level.
    parent: Option<Entity>,
}

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::left("hierarchy")
        .default_size(200.0)
        .show(ui, |ui| {
            ui.heading("Hierarchy");
            ui.separator();

            // Predicate is identity, not appearance — the same rule `Clear`
            // and `save` use for what counts as "in the scene". Querying
            // `Sprite` would hide an identity-carrying entity that has no
            // `Sprite` (loadable from a hand-edited file today, a future
            // light or camera entity tomorrow) even though it is savable,
            // loadable and clearable; this list, and only this list, would
            // then disagree about what the scene contains.
            let roots = hierarchy::roots(frame.world);
            if roots.is_empty() && scene_is_empty(frame.world) {
                ui.label(RichText::new("nothing in the scene").italics().weak());
                return;
            }

            let mut request = None;
            let mut drawn = HashSet::new();

            // The whole tree is one drop zone, so releasing over the empty
            // space below the last row unparents. A row that took the payload
            // first has already consumed it — `dnd_release_payload` takes
            // rather than peeks — so this only fires for a drop that hit no
            // row.
            let (_, dropped) = egui::ScrollArea::vertical()
                .show(ui, |ui| {
                    ui.dnd_drop_zone::<Entity, _>(Frame::default(), |ui| {
                        for root in roots {
                            row(editor, ui, frame, root, &mut drawn, &mut request);
                        }
                        // A parent cycle in a hand-edited file leaves entities
                        // that are neither roots nor reachable from one. They
                        // are in the scene and they save, so they are drawn —
                        // at the top level, which is the only honest place for
                        // an entity whose parent is itself.
                        for orphan in unreachable(frame.world, &drawn) {
                            row(editor, ui, frame, orphan, &mut drawn, &mut request);
                        }
                        // Fills the panel, so the drop zone covers the empty
                        // space below the tree rather than only the rows.
                        ui.allocate_space(egui::vec2(ui.available_width(), ui.available_height()));
                    })
                })
                .inner;

            if let Some(child) = dropped {
                request = Some(Reparent {
                    child: *child,
                    parent: None,
                });
            }

            if let Some(reparent) = request {
                apply(editor, frame, reparent);
            }
        });
}

/// One entity's row, then its children indented under it.
fn row(
    editor: &mut Editor,
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    entity: Entity,
    drawn: &mut HashSet<Entity>,
    request: &mut Option<Reparent>,
) {
    // The guard against a cycle: an entity is drawn once, and a link that
    // points back up the tree stops here instead of recursing forever.
    if !drawn.insert(entity) {
        return;
    }

    let selected = editor.selected == Some(entity);
    let response = ui
        .dnd_drag_source(Id::new(("hierarchy-row", entity)), entity, |ui| {
            // The row's own click is merged into the drag source's response
            // below, which is what `dnd_drag_source` returns.
            let _ = ui.selectable_label(selected, label(frame.world, entity));
        })
        .response;

    if response.clicked() {
        editor.selected = Some(entity);
    }
    if let Some(dragged) = response.dnd_release_payload::<Entity>() {
        *request = Some(Reparent {
            child: *dragged,
            parent: Some(entity),
        });
    }

    let children = hierarchy::children_of(frame.world, entity);
    if children.is_empty() {
        return;
    }
    // `indent` rather than a manual offset: it also draws the vertical line
    // that makes a deep tree readable. The spacing is set before the call, not
    // inside it — `indent` reads the spacing of the `Ui` it is called on.
    ui.spacing_mut().indent = INDENT;
    ui.indent(Id::new(("hierarchy-children", entity)), |ui| {
        for child in children {
            row(editor, ui, frame, child, drawn, request);
        }
    });
}

/// Performs a drop, as one undo entry.
///
/// The entity keeps its **world** placement across the move, which is what
/// Unity, Unreal and Godot all do when a row is dragged in the hierarchy:
/// dropping a sprite onto another one is a statement about what it belongs to,
/// not a request to teleport it into the parent's frame. The primitive in
/// `voltra-scene` does the opposite and keeps the local values — that is the
/// one that cannot lose data — so the placement is captured and put back here.
fn apply(editor: &mut Editor, frame: &mut UiFrame<'_>, reparent: Reparent) {
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
        selected: Some(id),
    };
    editor.history.begin("Reparent", view, [id]);

    let placement = hierarchy::world_transform(frame.world, child);
    let outcome = match parent {
        Some(parent) => hierarchy::set_parent(frame.world, child, parent),
        None => {
            hierarchy::clear_parent(frame.world, child);
            Ok(())
        }
    };

    match outcome {
        Ok(()) => {
            hierarchy::set_world_transform(frame.world, child, &placement);
            // The dragged row, not whatever was selected before: the drag says
            // which entity the user is thinking about.
            editor.selected = Some(child);
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
        selected: Some(id),
    };
    editor.history.commit(view);
}

/// What a row calls this entity.
fn label(world: &World, entity: Entity) -> String {
    match world.get::<Name>(entity) {
        Some(name) if !name.as_str().is_empty() => name.as_str().to_owned(),
        // Unnamed, or named the empty string mid-rename. The index is what the
        // inspector shows too, so the two panels agree.
        _ => format!("Entity {}", entity.index()),
    }
}

/// Scene entities the walk from the roots never reached.
fn unreachable(world: &World, drawn: &HashSet<Entity>) -> Vec<Entity> {
    let mut left: Vec<Entity> = world
        .query::<SceneId>()
        .map(|(entity, _)| entity)
        .filter(|entity| !drawn.contains(entity))
        .collect();
    left.sort_unstable_by_key(Entity::index);
    left
}

/// Whether the scene holds nothing at all.
///
/// Distinct from "no roots": a scene whose every entity is in a parent cycle
/// has no roots and is not empty, and saying "nothing in the scene" over a file
/// that has just been opened is the worst possible answer.
fn scene_is_empty(world: &World) -> bool {
    world.query::<SceneId>().next().is_none()
}
