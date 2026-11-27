//! The component sections under the name, in the order they are drawn,
//! split by what they are about.
//!
//! With several selected, a section is drawn only when none of the
//! components it edits is one only some of them have
//! ([`MultiEdit::editable`]), with a line under its rule naming the fields
//! that differ ([`super::mixed::marker`]).

mod behaviour;
mod physics;
mod visual;

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;

use crate::editor::Editor;
use crate::multi_edit::MultiEdit;

/// Every section `entity` has; what to call the edit one of them is in the
/// middle of, if any.
pub(super) fn show(
    editor: &mut Editor,
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    entity: Entity,
    multi: &mut MultiEdit,
) -> Option<&'static str> {
    let claim = visual::show(editor, ui, frame, entity, multi);
    let claim = claim.or(behaviour::show(editor, ui, frame, entity, multi));
    claim.or(physics::show(editor, ui, frame, entity, multi))
}

/// `component`, if the section for `name` may be drawn: not a component
/// only some of the selection have.
fn gate<T>(component: Option<T>, multi: &MultiEdit, name: &str) -> Option<T> {
    component.filter(|_| multi.editable(&[name]))
}
