//! What the bridge refuses so as not to throw away a person's work.

use crate::editor::Editor;

/// Refuses to replace the open document while a prefab is open over a
/// scene: the scene set aside has changes of its own, and returning first
/// is how they are kept or let go on purpose.
pub(super) fn not_in_prefab_mode(editor: &Editor) -> Result<(), String> {
    match editor.prefab_mode.returns_to() {
        Some(scene) => Err(format!(
            "{} is open as a prefab over {scene}: prefab_close first",
            editor.document.name()
        )),
        None => Ok(()),
    }
}

/// Refuses to let unsaved changes go unless the caller said to: the person
/// at the editor may be in the middle of them, and an AI must not throw
/// their work away by accident.
pub(super) fn keep_unsaved(editor: &Editor, discard_changes: bool) -> Result<(), String> {
    if editor.document.is_dirty(&editor.history) && !discard_changes {
        return Err(format!(
            "{} has unsaved changes: save it first, or pass discard_changes to drop them",
            editor.document.name()
        ));
    }
    Ok(())
}
