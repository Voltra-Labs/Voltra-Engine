//! Selecting in the viewport: a click picks what is under it (Ctrl toggles
//! it, Shift adds it), a box drawn on empty ground takes in what it
//! encloses, and every selected entity is outlined.

use voltra_core::egui::Response;
use voltra_core::UiFrame;

use crate::editor::Editor;
use crate::picking::clicked_entity;
use crate::selection::Click;
use crate::view;

/// The click and the box, after the gizmo has had first refusal:
/// `consumed` is whether it took the pointer.
pub(super) fn update(editor: &mut Editor, scene: &Response, frame: &UiFrame<'_>, consumed: bool) {
    if let Some(taken) = editor.marquee.update(scene, frame, consumed) {
        editor.selection.take(&taken.entities, taken.extend);
    }
    // A click and a drag are mutually exclusive in egui, so a box let go of
    // is never also a click.
    if consumed || !scene.clicked() {
        return;
    }
    let click = Click::from_modifiers(scene.ctx.input(|i| i.modifiers));
    match clicked_entity(scene, frame) {
        // No order in the viewport: a Shift-click adds.
        Some(entity) => editor.selection.click(entity, click, &[]),
        // Empty ground clears, unless a key says the click was to add.
        None if click == Click::Replace => editor.selection.clear(),
        None => {}
    }
}

/// The box being drawn and the outline of every selected entity.
pub(super) fn draw(editor: &Editor, frame: &mut UiFrame<'_>) {
    let outlines = view::selection::outlines(frame.world, &editor.selection, frame.sheets());
    let lines = frame.lines();
    view::selection::draw(&outlines, lines);
    editor.marquee.draw(lines);
}
