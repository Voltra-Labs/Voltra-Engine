//! Left panel: the scene as a tree, and which entities are selected — a
//! click selects one, Ctrl-click toggles one, Shift-click takes the rows
//! between.
//!
//! Rows are drag sources and drop targets both, which is how every engine's
//! hierarchy reparents: drop onto a row to become its child, drop below the
//! tree to become a root. The drop is applied *after* the tree is drawn — a
//! reparent while walking the links would change the list being walked.
//!
//! The search box over it filters the tree to what matches — by name, and
//! by component with `t:` (see [`crate::search`]) — keeping each match's
//! parents, dimmed, so it is shown where it lives.

use std::collections::HashSet;

use voltra_core::egui::{
    self, Align2, CursorIcon, Frame, Id, RichText, Sense, TextStyle, Ui, Vec2,
};
use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_scene::hierarchy::{self, HierarchyError};
use voltra_scene::SceneId;

use crate::drag;
use crate::editor::Editor;
use crate::search::hierarchy::{filter, label, HierarchyFilter};
use crate::search::Query;
use crate::selection::Click;

mod create;
mod reparent;

use reparent::{apply, Reparent};

/// How far one level of nesting is indented, in points.
///
/// Enough to read the shape of a deep tree in a 200-point panel, and not so
/// much that four levels push a name off the edge.
const INDENT: f32 = 14.0;

/// What a walk over the tree collects, acted on once it is over: a drop
/// would change the links being walked, and a Shift-click's range needs the
/// rows below the clicked one too.
#[derive(Default)]
struct Walk {
    /// Every row drawn, so a parent cycle stops rather than recursing.
    drawn: HashSet<Entity>,
    /// The rows in the order they are shown, top to bottom.
    order: Vec<Entity>,
    request: Option<Reparent>,
    click: Option<(Entity, Click)>,
    /// What a search left to show; `None` shows everything.
    filter: Option<HierarchyFilter>,
}

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::left("hierarchy")
        .default_size(200.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Hierarchy");
                create::show(editor, ui, frame);
            });
            ui.add(
                egui::TextEdit::singleline(&mut editor.hierarchy_search)
                    .hint_text("search: name, t:Sprite…")
                    .desired_width(f32::INFINITY),
            )
            .on_hover_text("words in the name, and t: a component every match has");
            ui.separator();

            // Predicate is identity, not appearance — the same rule `Clear`
            // and `save` use for what counts as "in the scene". Querying
            // `Sprite2D` would hide an identity-carrying entity that has no
            // `Sprite2D` (loadable from a hand-edited file today, a future
            // light or camera entity tomorrow) even though it is savable,
            // loadable and clearable; this list, and only this list, would
            // then disagree about what the scene contains.
            let roots = hierarchy::roots(frame.world);
            if roots.is_empty() && scene_is_empty(frame.world) {
                ui.label(RichText::new("nothing in the scene").italics().weak());
                return;
            }

            let mut walk = Walk::default();
            let query = Query::parse(&editor.hierarchy_search);
            if !query.is_empty() {
                let found = filter(frame.world, &editor.registry, &query);
                if !found.unknown.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "no component is called {}",
                            found.unknown.join(", ")
                        ))
                        .color(ui.visuals().error_fg_color),
                    );
                    return;
                }
                if found.matched.is_empty() {
                    ui.label(RichText::new("no entity matches").italics().weak());
                    return;
                }
                walk.filter = Some(found);
            }

            // The whole tree is one drop zone, so releasing over the empty
            // space below the last row unparents. A row that took the payload
            // first has already consumed it — `dnd_release_payload` takes
            // rather than peeks — so this only fires for a drop that hit no
            // row.
            let (_, dropped) = egui::ScrollArea::vertical()
                .show(ui, |ui| {
                    ui.dnd_drop_zone::<Entity, _>(Frame::default(), |ui| {
                        for root in roots {
                            row(editor, ui, frame, root, &mut walk);
                        }
                        // A parent cycle in a hand-edited file leaves entities
                        // that are neither roots nor reachable from one. They
                        // are in the scene and they save, so they are drawn —
                        // at the top level, which is the only honest place for
                        // an entity whose parent is itself.
                        for orphan in unreachable(frame.world, &walk.drawn) {
                            row(editor, ui, frame, orphan, &mut walk);
                        }
                        // Fills the panel, so the drop zone covers the empty
                        // space below the tree rather than only the rows — and
                        // says what releasing there does, because an empty
                        // area that is secretly a target is not a target.
                        let free = egui::vec2(ui.available_width(), ui.available_height());
                        if egui::DragAndDrop::has_payload_of_type::<Entity>(ui.ctx()) {
                            let (rect, _) = ui.allocate_exact_size(free, Sense::hover());
                            ui.painter().text(
                                rect.center_top() + Vec2::new(0.0, 8.0),
                                Align2::CENTER_TOP,
                                "drop here to unparent",
                                TextStyle::Small.resolve(ui.style()),
                                ui.visuals().weak_text_color(),
                            );
                        } else {
                            ui.allocate_space(free);
                        }
                    })
                })
                .inner;

            if let Some(child) = dropped {
                walk.request = Some(Reparent {
                    child: *child,
                    parent: None,
                });
            }

            if let Some((entity, click)) = walk.click {
                editor.selection.click(entity, click, &walk.order);
            }
            if let Some(reparent) = walk.request {
                apply(editor, frame, reparent);
            }
        });
}

/// One entity's row, then its children indented under it.
fn row(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>, entity: Entity, walk: &mut Walk) {
    // The guard against a cycle: an entity is drawn once, and a link that
    // points back up the tree stops here instead of recursing forever.
    if !walk.drawn.insert(entity) {
        return;
    }
    // Under a search, a row is drawn if it matches or holds a match; a
    // holder alone is dimmed, so the matches are what reads.
    let text = match &walk.filter {
        Some(found) if !found.shown.contains(&entity) => return,
        Some(found) if !found.matched.contains(&entity) => {
            RichText::new(label(frame.world, entity)).weak()
        }
        _ => RichText::new(label(frame.world, entity)),
    };
    walk.order.push(entity);

    let selected = editor.selection.contains(entity);
    // A row is a click target first and a drag source second, which is the
    // order Unity's hierarchy works in: a click selects, and only a press that
    // moves past egui's drag threshold picks the row up. `dnd_drag_source`
    // interacts with `Sense::drag()` alone, which swallows nearly every click —
    // the entity became almost impossible to select. `dnd_set_drag_payload`
    // publishes the payload on `drag_started` instead, so both gestures live on
    // one response and neither has to win a race with the other.
    let response = ui.selectable_label(selected, text).interact(Sense::drag());

    response.dnd_set_drag_payload(entity);
    if response.dragged() {
        // The only cursor feedback left once the row senses click too:
        // `dnd_set_drag_payload` deliberately withholds its `Grab` icon from a
        // clickable widget, which is why hovering no longer reads as "move me".
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        drag::ghost(ui, &label(frame.world, entity));
    }

    // What the drop would do, drawn while the pointer is still moving. A drag
    // with no target feedback is the complaint every hierarchy gets: the row
    // under the cursor has to say "this one becomes my parent" *before* the
    // release, and a target the rules refuse has to say so too rather than
    // swallowing the drop silently.
    if let Some(held) = response.dnd_hover_payload::<Entity>() {
        match hierarchy::can_parent(frame.world, *held, entity) {
            Ok(()) => drag::outline(ui, response.rect, ui.visuals().selection.bg_fill),
            // `Itself` is the row the drag started on, and marking your own row
            // as rejected reads as a broken panel rather than as a rule.
            Err(HierarchyError::Itself) => {}
            Err(_) => {
                ui.ctx().set_cursor_icon(CursorIcon::NoDrop);
                drag::outline(ui, response.rect, ui.visuals().error_fg_color);
            }
        }
    }

    if response.clicked() {
        let click = Click::from_modifiers(ui.input(|i| i.modifiers));
        walk.click = Some((entity, click));
    }
    if let Some(dragged) = response.dnd_release_payload::<Entity>() {
        walk.request = Some(Reparent {
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
            row(editor, ui, frame, child, walk);
        }
    });
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
