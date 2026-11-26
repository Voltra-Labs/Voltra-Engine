//! The machine's blend trees: where each clip alone plays, which clip, and
//! the parameters a tree reads.

use voltra_assets::AssetPath;
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::animator::{BlendTree, Child1D, Child2D, ParamValue};
use voltra_scene::StateMachine;

use crate::panels::inspector::path_field::{self, PathEdit};

/// Every state's tree: its parameters, a row per clip — its threshold or
/// point, and the clip, dropped from the browser or typed — and a field
/// that adds one. Answers whether anything was edited.
pub(super) fn show(ui: &mut Ui, machine: &mut StateMachine) -> bool {
    let floats: Vec<String> = machine
        .parameters
        .iter()
        .filter(|p| matches!(p.value, ParamValue::Float(_)))
        .map(|p| p.name.clone())
        .collect();
    let mut edited = false;
    egui::CollapsingHeader::new("blend trees")
        .id_salt("animator_trees")
        .show(ui, |ui| {
            ui.label(
                RichText::new("where each clip alone plays; Clear takes a clip out")
                    .weak()
                    .small(),
            );
            for (index, state) in machine.states.iter_mut().enumerate() {
                let Some(tree) = &mut state.tree else {
                    continue;
                };
                ui.push_id(("animator_tree", index), |ui| {
                    ui.label(RichText::new(&state.name).strong());
                    edited |= tree_ui(ui, tree, &floats);
                });
            }
        });
    edited
}

/// One tree's rows. Answers whether it was edited.
fn tree_ui(ui: &mut Ui, tree: &mut BlendTree, floats: &[String]) -> bool {
    let mut edited = false;
    let mut removed = None;
    match tree {
        BlendTree::Blend1D {
            parameter,
            children,
        } => {
            ui.horizontal(|ui| {
                ui.label("by");
                edited |= parameter_ui(ui, "parameter", parameter, floats);
            });
            egui::Grid::new("children").num_columns(2).show(ui, |ui| {
                for (i, child) in children.iter_mut().enumerate() {
                    edited |= ui
                        .add(DragValue::new(&mut child.threshold).speed(0.05))
                        .on_hover_text("where along the parameter it alone plays")
                        .changed();
                    match clip_cell(ui, i, &mut child.clip) {
                        Some(Change::Removed) => removed = Some(i),
                        Some(Change::Set) => edited = true,
                        None => {}
                    }
                    ui.end_row();
                }
            });
            if let Some(clip) = added(ui) {
                let threshold = children
                    .iter()
                    .map(|c| c.threshold)
                    .filter(|t| t.is_finite())
                    .reduce(f32::max)
                    .map_or(0.0, |last| last + 1.0);
                children.push(Child1D { threshold, clip });
                edited = true;
            }
            if let Some(i) = removed {
                children.remove(i);
                edited = true;
            }
        }
        BlendTree::Blend2D { x, y, children } => {
            ui.horizontal(|ui| {
                ui.label("by x");
                edited |= parameter_ui(ui, "x", x, floats);
                ui.label("y");
                edited |= parameter_ui(ui, "y", y, floats);
            });
            egui::Grid::new("children").num_columns(2).show(ui, |ui| {
                for (i, child) in children.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        for value in &mut child.position {
                            edited |= ui.add(DragValue::new(value).speed(0.05)).changed();
                        }
                    })
                    .response
                    .on_hover_text("the point (x, y) where it alone plays");
                    match clip_cell(ui, i, &mut child.clip) {
                        Some(Change::Removed) => removed = Some(i),
                        Some(Change::Set) => edited = true,
                        None => {}
                    }
                    ui.end_row();
                }
            });
            if let Some(clip) = added(ui) {
                children.push(Child2D {
                    position: [0.0, 0.0],
                    clip,
                });
                edited = true;
            }
            if let Some(i) = removed {
                children.remove(i);
                edited = true;
            }
        }
    }
    edited
}

/// What a clip's field did.
enum Change {
    /// It names another clip now.
    Set,
    /// Cleared: the clip leaves the tree.
    Removed,
}

/// Child `i`'s clip: typed or dropped, only a `.anim.ron` taken.
fn clip_cell(ui: &mut Ui, i: usize, clip: &mut AssetPath) -> Option<Change> {
    let edit = ui
        .push_id(("clip", i), |ui| {
            path_field::show(ui, "tree_clip", "path/to/clip.anim.ron", clip.as_str())
        })
        .inner?;
    match edit {
        PathEdit::Cleared => Some(Change::Removed),
        PathEdit::Set(path) => {
            let path = animation(path)?;
            *clip = path;
            Some(Change::Set)
        }
    }
}

/// The field that adds a clip to the tree, emptied once it has.
fn added(ui: &mut Ui) -> Option<AssetPath> {
    ui.horizontal(|ui| {
        ui.label("add");
        let edit = ui.push_id("tree_add", |ui| {
            let edit = path_field::show(ui, "tree_clip", "drop a clip to add it", "");
            if matches!(edit, Some(PathEdit::Set(_))) {
                // Emptied for the next: the box keeps what it was given.
                let buffer = ui.id().with("tree_clip");
                ui.ctx().data_mut(|d| d.remove::<String>(buffer));
            }
            edit
        });
        match edit.inner? {
            PathEdit::Set(path) => animation(path),
            PathEdit::Cleared => None,
        }
    })
    .inner
}

/// `path`, when it names a clip.
fn animation(path: AssetPath) -> Option<AssetPath> {
    if voltra_assets::watch::is_animation(path.as_str()) {
        Some(path)
    } else {
        log::warn!("{} is not an animation clip (*.anim.ron)", path.as_str());
        None
    }
}

/// A float parameter for `name` to read, picked from the machine's floats —
/// or kept as written when it names none of them, so an author's typo is
/// shown, not lost. Answers whether it changed.
fn parameter_ui(ui: &mut Ui, salt: &str, name: &mut String, floats: &[String]) -> bool {
    let mut picked = None;
    let shown = if name.is_empty() {
        "none"
    } else {
        name.as_str()
    };
    egui::ComboBox::from_id_salt(("tree_parameter", salt))
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for float in floats {
                if ui.selectable_label(float == name, float).clicked() && float != name {
                    picked = Some(float.clone());
                }
            }
        });
    match picked {
        Some(float) => {
            *name = float;
            true
        }
        None => false,
    }
}
