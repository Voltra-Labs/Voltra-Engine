//! The undo and redo keys, and when the editor is allowed to hear them.

use voltra_core::egui::{Context, Key, KeyboardShortcut, Modifiers};

/// What this frame's keys asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoAction {
    Undo,
    Redo,
}

/// Reads and consumes an undo or redo key, if one was pressed.
///
/// Scoped the way the viewport's `W` key is: ignored while egui wants the
/// keyboard, so `Ctrl+Z` inside the texture field is egui's own text undo and
/// not the scene's.
///
/// Redo is matched first, and that order is load-bearing. egui matches modifiers
/// *logically* — extra Shift and Alt are ignored — so `Ctrl+Z`'s pattern also
/// accepts `Ctrl+Shift+Z`, and testing undo first would make the redo key undo.
pub fn poll(ctx: &Context) -> Option<UndoAction> {
    if ctx.egui_wants_keyboard_input() {
        return None;
    }

    let redo_z = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    let redo_y = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
    let undo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);

    ctx.input_mut(|input| {
        if input.consume_shortcut(&redo_z) || input.consume_shortcut(&redo_y) {
            Some(UndoAction::Redo)
        } else if input.consume_shortcut(&undo) {
            Some(UndoAction::Undo)
        } else {
            None
        }
    })
}
