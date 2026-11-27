//! The name field: what the hierarchy calls an entity.

use voltra_core::egui::{TextEdit, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::Name;

/// The name field over `entities` — the primary first, then the rest of
/// the selection — and the claim that keeps a whole rename in one undo
/// entry.
///
/// The component is inserted on the first keystroke rather than being there
/// from the start: an entity is not required to have a name, and typing one is
/// what says it wants one. With several selected, a name typed is every one's,
/// as in Unity; names that differ show as an empty field reading `—`, and one
/// without a name counts as an empty one.
pub(super) fn show(ui: &mut Ui, world: &mut World, entities: &[Entity]) -> Option<&'static str> {
    let &primary = entities.first()?;
    let mut name = name_of(world, primary);
    let mixed = entities[1..].iter().any(|&e| name_of(world, e) != name);
    if mixed {
        name.clear();
    }

    let response = ui.add(
        TextEdit::singleline(&mut name)
            .hint_text(if mixed { "—" } else { "name" })
            .desired_width(f32::INFINITY),
    );
    let response = if mixed {
        response
            .on_hover_text("the selected entities have different names; one typed is all of theirs")
    } else {
        response
    };

    if response.changed() {
        for &entity in entities {
            world.insert(entity, Name::new(name.clone()));
        }
    }

    // Focused, not only changed: a rename is one interaction from the first
    // keystroke to the moment the field is left, the same rule a held
    // `DragValue` follows.
    (response.has_focus() || response.changed()).then_some("Rename")
}

/// `entity`'s name, empty without one.
fn name_of(world: &World, entity: Entity) -> String {
    world
        .get::<Name>(entity)
        .map(|name| name.0.clone())
        .unwrap_or_default()
}

/// What to call `entity` in a sentence.
pub(super) fn of(world: &World, entity: Entity) -> String {
    match world.get::<Name>(entity) {
        Some(name) if !name.as_str().is_empty() => name.as_str().to_owned(),
        _ => format!("Entity {}", entity.index()),
    }
}
