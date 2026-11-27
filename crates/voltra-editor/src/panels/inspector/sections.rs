//! The component sections under the name, in the order they are drawn,
//! split by what they are about.

mod physics;
mod visual;

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;

use crate::editor::Editor;

/// Every section `entity` has; what to call the edit one of them is in the
/// middle of, if any.
pub(super) fn show(
    editor: &mut Editor,
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    entity: Entity,
) -> Option<&'static str> {
    let claim = visual::show(editor, ui, frame, entity);
    claim.or(physics::show(editor, ui, frame, entity))
}
