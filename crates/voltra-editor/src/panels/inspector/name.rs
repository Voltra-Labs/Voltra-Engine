//! The name field: what the hierarchy calls an entity.

use voltra_core::egui::{TextEdit, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::Name;

/// The name field, and the claim that keeps a whole rename in one undo entry.
///
/// The component is inserted on the first keystroke rather than being there
/// from the start: an entity is not required to have a name, and typing one is
/// what says it wants one.
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let mut name = world
        .get::<Name>(entity)
        .map(|name| name.0.clone())
        .unwrap_or_default();

    let response = ui.add(
        TextEdit::singleline(&mut name)
            .hint_text("name")
            .desired_width(f32::INFINITY),
    );

    if response.changed() {
        world.insert(entity, Name::new(name));
    }

    // Focused, not only changed: a rename is one interaction from the first
    // keystroke to the moment the field is left, the same rule a held
    // `DragValue` follows.
    (response.has_focus() || response.changed()).then_some("Rename")
}

/// What to call `entity` in a sentence.
pub(super) fn of(world: &World, entity: Entity) -> String {
    match world.get::<Name>(entity) {
        Some(name) if !name.as_str().is_empty() => name.as_str().to_owned(),
        _ => format!("Entity {}", entity.index()),
    }
}
