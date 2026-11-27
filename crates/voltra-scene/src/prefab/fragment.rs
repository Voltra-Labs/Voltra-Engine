//! An entity and everything under it, cut out of a world as a prefab file.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::{PrefabInstance, Prefabs};
use crate::format::{record_entity, ComponentRegistry, SceneError, SceneFile, VERSION};
use crate::hierarchy::{descendants, is_ancestor};
use crate::transform::Transform;

/// `root` and its descendants as a file [`super::spawn`] can stamp out again.
///
/// What changes on the way out — on the root, and one thing on every entity:
///
/// - **Its parent link is dropped.** The parent stays in the scene, and a
///   link to it would name an entity the prefab does not hold. A spawn would
///   cut it anyway; a file that does not carry it is the honest one.
/// - **Its own [`PrefabInstance`] is dropped.** Whatever it was an instance
///   of, it is now the prefab, and every spawn links its roots to the file
///   they came from. A child's link is kept: that child is a nested instance.
/// - **An entity's [`super::PrefabPart`] is kept only inside an instance
///   the file holds**: an instance in the selection, not around it, is
///   stored whole — link and parts — so it stays an instance of its own
///   file and follows it. Any other part names a record of whatever prefab
///   the selection came from, not of this one, and is dropped.
/// - **Its translation is zeroed; its rotation and scale are kept.** A spawn
///   places roots *around* `at`, so a prefab authored where it happened to
///   stand in the scene would land that far from every cursor it is dropped
///   on. Unity keeps the position and every author then resets it by hand;
///   Godot's saved branch is local to its own root, which is this.
///
/// Root first, then the descendants: [`super::Instance::root`] is the first
/// root, and a file that leads with it reads the way the hierarchy does. An
/// entity without a [`crate::SceneId`] is transient and left out, as a save
/// leaves it out.
///
/// `prefabs` tells which instance an entity belongs to: one dropped under
/// a part of another is its own (see [`super::instance_root`]).
pub fn fragment(
    world: &World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    root: Entity,
) -> Result<SceneFile, SceneError> {
    let mut entities = Vec::new();

    for entity in std::iter::once(root).chain(descendants(world, root)) {
        let Some(record) = record_entity(world, registry, entity) else {
            continue;
        };
        let mut record = record?;
        // The file is a prefab of its own: which record of some other one
        // an entity was is no part of it — unless that other one is an
        // instance inside the file.
        let stored_inside = super::instance_root(world, prefabs, entity)
            .is_some_and(|at| at != root && is_ancestor(world, root, at));
        if !stored_inside {
            record.components.remove(super::PrefabPart::NAME);
        }
        if entity == root {
            record.components.remove("Parent");
            record.components.remove(PrefabInstance::NAME);
            if let Some(transform) = world.get::<Transform>(entity) {
                let at_origin = Transform {
                    translation: Vec2::ZERO,
                    ..*transform
                };
                let value =
                    ron::value::RawValue::from_rust(&at_origin).map_err(SceneError::Serialize)?;
                record.components.insert("Transform".to_owned(), value);
            }
        }
        entities.push(record);
    }

    Ok(SceneFile {
        version: VERSION,
        entities,
    })
}
