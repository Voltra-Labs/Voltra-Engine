//! A light-layer mask, folded away by default: thirty-two toggles are a lot
//! of panel for a setting most sprites and lights never change.

use voltra_core::egui::Ui;

use super::collision::bits;

/// The `mask` toggles under a collapsible `label`, salted by `id`. Returns
/// the history label when a toggle changed — one click, the whole edit.
pub(super) fn show(
    ui: &mut Ui,
    id: &str,
    label: &str,
    hint: &str,
    mask: &mut u32,
) -> Option<&'static str> {
    let mut changed = false;
    ui.collapsing(label, |ui| {
        ui.label(hint);
        changed = bits(ui, id, mask);
    });
    changed.then_some("Set light layers")
}
