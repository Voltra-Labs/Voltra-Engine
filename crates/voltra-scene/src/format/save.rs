//! Turning a world into a scene file.

use std::collections::BTreeMap;
use std::path::Path;

use ron::value::RawValue;
use serde::{Deserialize, Serialize};
use voltra_ecs::World;

use super::atomic;
use super::error::SceneError;
use super::registry::ComponentRegistry;
use crate::scene_id::{SceneId, UnknownComponents};

/// The only scene format version this build writes or reads.
///
/// Written from the first release. Adding a version field later means guessing
/// what a file without one meant.
pub const VERSION: u32 = 1;

/// A scene as it appears on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneFile {
    pub version: u32,
    pub entities: Vec<EntityRecord>,
}

/// One entity: its stable identity and every component it carries.
///
/// `BTreeMap` rather than `HashMap` so component names come out alphabetically
/// and two saves of the same world are byte-identical. A `HashMap` would reorder
/// between runs and fill the diff with noise that is not a change.
///
/// Values are `Box<RawValue>`, not `ron::Value`: `Value` has no struct variant,
/// so a component would round-trip as a brace-and-quoted-key map with its
/// fields resorted alphabetically, and its deserializer rejects enums outright.
/// `RawValue` is `#[repr(transparent)]` over the original RON text and embeds
/// it verbatim, so a component's own field order and shape survive untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityRecord {
    pub id: SceneId,
    pub components: BTreeMap<String, Box<RawValue>>,
}

/// The formatting every save uses.
///
/// Defined once, on purpose. Two saves of the same world can only be identical
/// if there is exactly one configuration and nobody passes a variant of it.
/// `struct_names` is off: `SceneFile(` at the top of the file tells a reader
/// nothing they need.
fn pretty() -> ron::ser::PrettyConfig {
    ron::ser::PrettyConfig::new()
        .struct_names(false)
        .indentor("    ")
        .new_line("\n")
}

/// Every entity holding a [`SceneId`], in id order.
pub fn to_scene_file(world: &World, registry: &ComponentRegistry) -> Result<SceneFile, SceneError> {
    let mut entities: Vec<EntityRecord> = Vec::new();

    for (entity, id) in world.query::<SceneId>() {
        let mut components = BTreeMap::new();

        for name in registry.names() {
            if let Some(value) = registry.save_one(world, entity, name) {
                components.insert(name.to_owned(), value?);
            }
        }

        // Merged after the known ones so a component that has since become
        // registered wins over the stale copy kept from an older load.
        if let Some(unknown) = world.get::<UnknownComponents>(entity) {
            for (name, value) in &unknown.0 {
                components
                    .entry(name.clone())
                    .or_insert_with(|| value.clone());
            }
        }

        entities.push(EntityRecord {
            id: *id,
            components,
        });
    }

    // Ordering by id is ordering by creation, because the ids are UUIDv7. That
    // makes the file deterministic and puts new entities at the end rather than
    // in the middle of someone's diff.
    entities.sort_by_key(|record| record.id);

    Ok(SceneFile {
        version: VERSION,
        entities,
    })
}

/// Writes the world to `path`, or leaves the file exactly as it was.
///
/// There is no third outcome. The bytes go to a sibling temporary and are
/// renamed over `path`, so a crash, a full disk or a serialization failure
/// cannot leave a half-written scene where a whole one used to be. See
/// [`atomic::replace`].
pub fn save(world: &World, registry: &ComponentRegistry, path: &Path) -> Result<(), SceneError> {
    let file = to_scene_file(world, registry)?;
    let text = ron::ser::to_string_pretty(&file, pretty()).map_err(SceneError::Serialize)?;
    atomic::replace(path, text.as_bytes())?;
    log::info!(
        "saved {} entities to {}",
        file.entities.len(),
        path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Sprite, Transform};
    use voltra_render::glam::Vec2;

    fn world_with(count: usize) -> World {
        let mut world = World::new();
        for i in 0..count {
            let e = world.spawn();
            world.insert(e, SceneId::new());
            world.insert(e, Transform::from_translation(Vec2::new(i as f32, 0.0)));
            world.insert(e, Sprite::default());
        }
        world
    }

    #[test]
    fn an_empty_world_saves_an_empty_scene() {
        let file = to_scene_file(&World::new(), &ComponentRegistry::with_defaults())
            .expect("an empty world cannot fail to save");
        assert_eq!(file.version, VERSION);
        assert!(file.entities.is_empty());
    }

    #[test]
    fn an_entity_without_a_scene_id_is_skipped() {
        // Opting out of persistence is the absence of a SceneId, so no exclusion
        // list has to be kept anywhere.
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(e, Sprite::default());

        let file = to_scene_file(&world, &ComponentRegistry::with_defaults())
            .expect("saving cannot fail here");
        assert!(file.entities.is_empty(), "got {:?}", file.entities);
    }

    #[test]
    fn entities_are_ordered_by_id() {
        let world = world_with(8);
        let file = to_scene_file(&world, &ComponentRegistry::with_defaults())
            .expect("saving cannot fail here");

        let ids: Vec<_> = file.entities.iter().map(|e| e.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "entities must be written in SceneId order");
    }

    #[test]
    fn a_saved_entity_carries_its_registered_components() {
        let world = world_with(1);
        let file = to_scene_file(&world, &ComponentRegistry::with_defaults())
            .expect("saving cannot fail here");

        let keys: Vec<_> = file.entities[0].components.keys().cloned().collect();
        assert_eq!(keys, vec!["Sprite".to_owned(), "Transform".to_owned()]);
    }

    #[test]
    fn unknown_components_are_written_back() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, SceneId::new());
        world.insert(e, Sprite::default());

        let physics: Box<RawValue> = RawValue::from_ron("(mass: 5.0)")
            .expect("valid RON")
            .to_owned();
        let mut unknown = UnknownComponents::default();
        unknown.0.insert("Physics".to_owned(), physics.clone());
        world.insert(e, unknown);

        let file = to_scene_file(&world, &ComponentRegistry::with_defaults())
            .expect("saving cannot fail here");

        assert_eq!(
            file.entities[0].components.get("Physics"),
            Some(&physics),
            "an unrecognised component must survive a save untouched"
        );
    }

    #[test]
    fn saving_the_same_world_twice_is_identical() {
        // The property the whole format rests on: a diff shows changes, never
        // the serializer changing its mind about ordering or spacing.
        let world = world_with(4);
        let registry = ComponentRegistry::with_defaults();
        let dir = std::env::temp_dir().join("voltra-scene-save-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let a = dir.join("a.ron");
        let b = dir.join("b.ron");

        save(&world, &registry, &a).expect("saving cannot fail here");
        save(&world, &registry, &b).expect("saving cannot fail here");

        assert_eq!(
            std::fs::read_to_string(&a).expect("written"),
            std::fs::read_to_string(&b).expect("written"),
        );
    }
}
