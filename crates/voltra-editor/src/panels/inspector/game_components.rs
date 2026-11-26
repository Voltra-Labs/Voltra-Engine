//! The game's own components, shown by the names it registered them under.
//!
//! The editor knows nothing of a game's types: what it has is the registry
//! the game filled, which reads and writes each as the RON a scene file
//! holds. So each is edited as that text, applied the moment it reads as
//! the type — Godot's inspector falls back to the same for a type with no
//! editor of its own — with the reason shown while it does not.

use std::sync::OnceLock;

use ron::value::RawValue;
use voltra_core::egui::{self, Color32, RichText, TextEdit, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::ComponentRegistry;

use super::active;

/// The engine's own names: what the rest of the inspector shows.
fn engine_names() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES.get_or_init(|| ComponentRegistry::with_defaults().names().collect())
}

/// The registered names that are the game's, not the engine's.
fn game_names(registry: &ComponentRegistry) -> Vec<&'static str> {
    registry
        .names()
        .filter(|name| !engine_names().contains(name))
        .collect()
}

/// Every game component on `entity`, editable, and a way to add the rest.
/// Draws nothing for a registry with no game in it.
pub(super) fn show(
    ui: &mut Ui,
    world: &mut World,
    entity: Entity,
    registry: &ComponentRegistry,
) -> Option<&'static str> {
    let names = game_names(registry);
    if names.is_empty() {
        return None;
    }
    let (held, missing): (Vec<_>, Vec<_>) = names
        .into_iter()
        .partition(|name| registry.save_one(world, entity, name).is_some());
    ui.separator();
    ui.label(RichText::new("Game").strong())
        .on_hover_text("the game's own components, as its scene files write them");

    let mut claim = None;
    for name in held {
        claim = claim.or(component(ui, world, entity, registry, name));
    }

    let addable: Vec<&str> = missing
        .into_iter()
        .filter(|name| registry.template(name).is_some())
        .collect();
    if !addable.is_empty() {
        let mut chosen = None;
        egui::ComboBox::from_id_salt(("add_game_component", entity))
            .selected_text("Add component")
            .show_ui(ui, |ui| {
                for name in addable {
                    if ui.selectable_label(false, name).clicked() {
                        chosen = Some(name);
                    }
                }
            });
        if let Some(name) = chosen {
            if add(world, entity, registry, name) {
                claim = Some("Add game component");
            }
        }
    }
    claim
}

/// One component: its name, a remove button, and its value as text.
fn component(
    ui: &mut Ui,
    world: &mut World,
    entity: Entity,
    registry: &ComponentRegistry,
    name: &str,
) -> Option<&'static str> {
    let current = match registry.save_one(world, entity, name)? {
        Ok(raw) => raw.get_ron().to_owned(),
        Err(error) => {
            ui.colored_label(Color32::LIGHT_RED, format!("{name}: {error}"));
            return None;
        }
    };
    let mut claim = None;
    ui.horizontal(|ui| {
        ui.label(name);
        if ui
            .small_button(crate::icons::REMOVE)
            .on_hover_text(format!("remove {name}"))
            .clicked()
            && registry.remove_one(world, entity, name)
        {
            claim = Some("Remove game component");
        }
    });
    if claim.is_some() {
        return claim;
    }

    // The text being typed lives in egui's memory while the field has the
    // focus; otherwise the field shows the component as it is now.
    let id = ui.id().with(("game_component", entity, name));
    let mut text = ui
        .data_mut(|data| data.get_temp::<String>(id))
        .unwrap_or_else(|| current.clone());
    let response = ui.add(
        TextEdit::multiline(&mut text)
            .code_editor()
            .desired_rows(1)
            .desired_width(f32::INFINITY),
    );
    if response.has_focus() {
        ui.data_mut(|data| data.insert_temp(id, text.clone()));
    } else {
        ui.data_mut(|data| data.remove::<String>(id));
    }
    if response.changed() && text != current {
        match apply(world, entity, registry, name, &text) {
            Ok(()) => {}
            Err(reason) => {
                ui.label(RichText::new(reason).small().color(Color32::LIGHT_RED));
            }
        }
    } else if response.has_focus() && text != current {
        // Still not readable from an earlier keystroke: say why again.
        if let Err(reason) = parse(registry, name, &text) {
            ui.label(RichText::new(reason).small().color(Color32::LIGHT_RED));
        }
    }
    claim.or(active(&response, "Edit game component"))
}

/// Reads `text` as `name`'s type, without touching the world.
fn parse(registry: &ComponentRegistry, name: &str, text: &str) -> Result<Box<RawValue>, String> {
    let raw = RawValue::from_ron(text).map_err(|e| e.to_string())?;
    match registry.normalize(name, raw) {
        Some(Ok(normal)) => Ok(normal),
        Some(Err(error)) => Err(error.to_string()),
        None => Err(format!("{name} is not registered")),
    }
}

/// Writes `text` as `entity`'s `name`, if it reads as the type.
fn apply(
    world: &mut World,
    entity: Entity,
    registry: &ComponentRegistry,
    name: &str,
    text: &str,
) -> Result<(), String> {
    let value = parse(registry, name, text)?;
    match registry.load_one(world, entity, name, &value) {
        Some(Ok(())) => Ok(()),
        Some(Err(error)) => Err(error.to_string()),
        None => Err(format!("{name} is not registered")),
    }
}

/// Puts `name`'s template on `entity`.
fn add(world: &mut World, entity: Entity, registry: &ComponentRegistry, name: &str) -> bool {
    match registry.template(name) {
        Some(Ok(template)) => matches!(
            registry.load_one(world, entity, name, &template),
            Some(Ok(()))
        ),
        Some(Err(error)) => {
            log::error!("cannot add {name}: {error}");
            false
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_core::egui::Context;

    #[derive(Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Coin {
        value: u32,
    }

    fn registry() -> ComponentRegistry {
        let mut registry = ComponentRegistry::with_defaults();
        registry.register::<Coin>("Coin").with_template();
        registry
    }

    #[test]
    fn only_the_games_names_are_the_games() {
        assert_eq!(game_names(&registry()), ["Coin"]);
        assert!(game_names(&ComponentRegistry::with_defaults()).is_empty());
    }

    #[test]
    fn text_that_reads_as_the_type_is_applied_and_text_that_does_not_is_not() {
        let registry = registry();
        let mut world = World::new();
        let coin = world.spawn();
        world.insert(coin, Coin { value: 1 });

        assert_eq!(
            apply(&mut world, coin, &registry, "Coin", "(value: 5)"),
            Ok(())
        );
        assert_eq!(world.get::<Coin>(coin), Some(&Coin { value: 5 }));

        let error = apply(&mut world, coin, &registry, "Coin", "(value: \"five\")")
            .expect_err("not a number");
        assert!(!error.is_empty());
        assert_eq!(world.get::<Coin>(coin), Some(&Coin { value: 5 }), "kept");
    }

    #[test]
    fn a_component_is_added_from_its_template() {
        let registry = registry();
        let mut world = World::new();
        let entity = world.spawn();

        assert!(add(&mut world, entity, &registry, "Coin"));
        assert_eq!(world.get::<Coin>(entity), Some(&Coin::default()));
    }

    #[test]
    fn drawing_the_section_changes_nothing_by_itself() {
        let registry = registry();
        let mut world = World::new();
        let coin = world.spawn();
        world.insert(coin, Coin { value: 3 });
        let mut claim = Some("unset");

        let ctx = Context::default();
        let _ = ctx.run_ui(Default::default(), |ui| {
            claim = show(ui, &mut world, coin, &registry);
        });

        assert_eq!(claim, None);
        assert_eq!(world.get::<Coin>(coin), Some(&Coin { value: 3 }));
    }
}
