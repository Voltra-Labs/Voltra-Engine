//! The machine's layers: how much each shows, which targets it may write,
//! and whether it replaces or adds.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_scene::animator::{Layer, LayerBlend};
use voltra_scene::StateMachine;

/// One block per layer — its weight, its blend and its mask, a path per row
/// with a button that takes it out, and a box offering the entity's named
/// targets to add. `targets` are the paths under the animator's entity.
/// Answers whether anything was edited.
pub(super) fn show(ui: &mut Ui, machine: &mut StateMachine, targets: &[String]) -> bool {
    let mut edited = false;
    egui::CollapsingHeader::new("layers")
        .id_salt("animator_layers")
        .show(ui, |ui| {
            ui.label(
                RichText::new("played over the machine's states, bottom up; an empty mask writes every target")
                    .weak()
                    .small(),
            );
            for (place, layer) in machine.layers.iter_mut().enumerate() {
                ui.push_id(("animator_layer", place), |ui| {
                    edited |= layer_ui(ui, layer, targets);
                });
            }
        });
    edited
}

fn layer_ui(ui: &mut Ui, layer: &mut Layer, targets: &[String]) -> bool {
    let mut edited = false;
    ui.label(RichText::new(&layer.name).strong());
    ui.horizontal(|ui| {
        ui.label("weight");
        edited |= ui
            .add(
                // Not clamped as shown: a weight typed outside the range in
                // the file is kept (it plays clamped) until it is dragged.
                DragValue::new(&mut layer.weight)
                    .speed(0.01)
                    .range(0.0..=1.0)
                    .clamp_existing_to_range(false),
            )
            .on_hover_text("how much of it shows; a game can set it while playing")
            .changed();
        egui::ComboBox::from_id_salt("blend")
            .selected_text(label(layer.blend))
            .show_ui(ui, |ui| {
                for blend in [LayerBlend::Override, LayerBlend::Additive] {
                    edited |= ui
                        .selectable_value(&mut layer.blend, blend, label(blend))
                        .changed();
                }
            })
            .response
            .on_hover_text(
                "override replaces what is under it by its weight; additive adds its clips' change from their first frame",
            );
    });
    let mut removed = None;
    for (i, path) in layer.mask.iter().enumerate() {
        ui.horizontal(|ui| {
            let shown = if path.is_empty() {
                "self"
            } else {
                path.as_str()
            };
            ui.label(format!("mask {shown}"))
                .on_hover_text("this target and everything under it");
            if ui.small_button("remove").clicked() {
                removed = Some(i);
            }
        });
    }
    if let Some(i) = removed {
        layer.mask.remove(i);
        edited = true;
    }
    let mut added = None;
    egui::ComboBox::from_id_salt("mask_add")
        .selected_text("add to mask")
        .show_ui(ui, |ui| {
            for target in targets.iter().filter(|t| !layer.mask.contains(t)) {
                let shown = if target.is_empty() {
                    "self"
                } else {
                    target.as_str()
                };
                if ui.selectable_label(false, shown).clicked() {
                    added = Some(target.clone());
                }
            }
        })
        .response
        .on_hover_text("the named entities under this one");
    if let Some(target) = added {
        layer.mask.push(target);
        edited = true;
    }
    edited
}

fn label(blend: LayerBlend) -> &'static str {
    match blend {
        LayerBlend::Override => "override",
        LayerBlend::Additive => "additive",
    }
}
