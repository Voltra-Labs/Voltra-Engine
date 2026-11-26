//! What of a machine the inspector edits — its layers' weights, masks and
//! blends, its transitions' fades, its states' blend trees, of every layer
//! — and the Save and Revert that write or drop it.

use voltra_assets::AssetPath;
use voltra_core::egui::{self, Id, Ui};
use voltra_scene::StateMachines;

use super::{layers, transitions, trees};

/// The machine's editable sections, edited in the store, so every animator
/// running the machine plays the edit at once, and written to the file by
/// **Save** (or dropped by **Revert**), as the timeline's clips are. Save
/// writes only what is edited here, into the file as written
/// ([`StateMachines::save_edits`]): what loading dropped stays.
///
/// `targets` are the paths under the animator's entity, which a layer's
/// mask is offered.
///
/// The machine is an asset, not the scene: its edits are not in the undo
/// history. Whether it is edited is the UI's to remember, in egui's memory
/// under the machine's path.
pub(super) fn show(
    ui: &mut Ui,
    machines: &mut StateMachines,
    path: &AssetPath,
    targets: &[String],
) {
    let handle = machines.load(path);
    let machine = machines.get(handle);
    let layer_states = machine.layers.iter().flat_map(|l| &l.states);
    let has_trees = machine
        .states
        .iter()
        .chain(layer_states)
        .any(|s| s.tree.is_some());
    let has_transitions =
        !machine.transitions.is_empty() || machine.layers.iter().any(|l| !l.transitions.is_empty());
    let has_layers = !machine.layers.is_empty();
    if !has_transitions && !has_trees && !has_layers {
        return;
    }
    let edited_id = Id::new(("animator_machine_edited", path.as_str()));
    let mut edited: bool = ui.ctx().data(|d| d.get_temp(edited_id)).unwrap_or(false);
    let machine = machines.get_mut(handle);
    if has_layers {
        edited |= layers::show(ui, machine, targets);
    }
    if has_transitions {
        edited |= transitions::show(ui, machine);
    }
    if has_trees {
        edited |= trees::show(ui, machine);
    }
    ui.horizontal(|ui| {
        if ui.add_enabled(edited, egui::Button::new("Save")).clicked() {
            // The edited parts only, into the file as written: a save of
            // the checked machine would delete what loading dropped.
            match machines.save_edits(path) {
                Ok(()) => {
                    edited = false;
                    log::info!("saved {}", path.as_str());
                }
                Err(e) => log::error!("could not save {}: {e}", path.as_str()),
            }
        }
        if ui
            .add_enabled(edited, egui::Button::new("Revert"))
            .on_hover_text("reads the file back, dropping the edits")
            .clicked()
        {
            if machines.reload(path) {
                edited = false;
            } else {
                log::warn!("{} could not be read back; the edits stay", path.as_str());
            }
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(edited_id, edited));
}
