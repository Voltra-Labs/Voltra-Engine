//! Which named sorting layer a drawable is on.

use voltra_core::egui::{self, Ui};
use voltra_scene::SortingLayers;

/// A dropdown of the project's layers, back to front. A name not in the list
/// — a scene from a project with other layers — is shown as it is and kept
/// until another is picked.
pub(super) fn show(
    ui: &mut Ui,
    salt: &str,
    layer: &mut String,
    layers: &SortingLayers,
) -> Option<&'static str> {
    let mut claim = None;
    ui.horizontal(|ui| {
        ui.label("sorting layer")
            .on_hover_text("drawn after every earlier layer, whatever the sort order");
        egui::ComboBox::from_id_salt(("sorting_layer", salt))
            .selected_text(layer.as_str())
            .show_ui(ui, |ui| {
                for name in layers.names() {
                    if ui.selectable_label(layer == name, name.as_str()).clicked() && layer != name
                    {
                        *layer = name.clone();
                        claim = Some("Set sorting layer");
                    }
                }
            });
    });
    claim
}
