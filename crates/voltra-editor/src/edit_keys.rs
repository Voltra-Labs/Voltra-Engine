//! The keys that act on the selection: Delete, Ctrl+D, and the clipboard's
//! Ctrl+C, Ctrl+X and Ctrl+V.
//!
//! Heard only while no text field has the keyboard, as the undo keys are:
//! inside one they edit its text. The clipboard's arrive as egui events
//! rather than keys — egui turns Ctrl+C into `Copy`, Ctrl+X into `Cut`, and
//! Ctrl+V into `Paste` with the clipboard's text — so they are read as such.

use voltra_core::egui::{Context, Event, Key, KeyboardShortcut, Modifiers};
use voltra_core::UiFrame;

use crate::editor::Editor;

/// Acts on this frame's selection keys, if any were pressed.
pub(crate) fn poll(editor: &mut Editor, ctx: &Context, frame: &mut UiFrame<'_>) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let duplicate = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
    let (delete, duplicate, events) = ctx.input_mut(|input| {
        (
            input.consume_key(Modifiers::NONE, Key::Delete),
            input.consume_shortcut(&duplicate),
            input.events.clone(),
        )
    });
    for event in events {
        match event {
            Event::Copy => crate::clipboard::copy_selection(editor, ctx, frame),
            Event::Cut => {
                crate::clipboard::copy_selection(editor, ctx, frame);
                crate::delete::selection(editor, frame);
            }
            Event::Paste(text) => crate::clipboard::paste_text(editor, frame, &text),
            _ => {}
        }
    }
    if duplicate {
        crate::clipboard::duplicate(editor, frame);
    }
    if delete {
        crate::delete::selection(editor, frame);
    }
}
