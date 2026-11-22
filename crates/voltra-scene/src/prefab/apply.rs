//! Giving an instance back to its file: the selection made an instance of a
//! new prefab ([`link`]), and an edited instance written over the prefab it
//! came from ([`apply`]) — Unity's "Apply All", Godot's saving an edited
//! inherited scene.

use ron::value::RawValue;
use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use super::cycle::{nested_in, reaches};
use super::instances::sync_source;
use super::write::WriteError;
use super::{PrefabInstance, PrefabPart, Prefabs, SyncReport};
use crate::format::{record_entity, ComponentRegistry, SceneError, SceneFile, VERSION};
use crate::hierarchy::{self, Parent};
use crate::scene_id::SceneId;
use crate::transform::Transform;

/// Makes `root` and everything under it an instance of `source`, a file
/// [`super::write`] has just cut from them: the root linked, and each entity
/// told it is the record of its own id — which is the id the file was
/// written with.
pub fn link(world: &mut World, root: Entity, source: AssetPath) {
    for entity in std::iter::once(root).chain(hierarchy::descendants(world, root)) {
        if let Some(&id) = world.get::<SceneId>(entity) {
            world.insert(entity, PrefabPart::new(id));
        }
    }
    world.insert(root, PrefabInstance::new(source));
}

/// Writes the instance at `root` over its prefab, then brings every other
/// instance of it in step. Answers what that sync did.
///
/// What the file gets, entity by entity: everything the entity carries,
/// under its record's id — an entity the instance added becomes a record of
/// its own — with parents named by record. Not the links and parts, which
/// belong to instances, not to the prefab; and not the root's placement:
/// the file's root keeps the transform it had, as Unity's apply leaves the
/// root's position out. Records the instance deleted leave the file.
/// Afterwards the instance overrides nothing — it *is* the prefab.
///
/// Refused, with nothing written, for an instance that holds an instance of
/// its own prefab (the file would contain itself).
pub fn apply(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    root: Entity,
) -> Result<SyncReport, WriteError> {
    let Some(source) = world.get::<PrefabInstance>(root).map(|l| l.source.clone()) else {
        return Ok(SyncReport::default());
    };
    let entities: Vec<Entity> = std::iter::once(root)
        .chain(hierarchy::descendants(world, root))
        .collect();
    // What the instance added is a record of its own from now on.
    for &entity in &entities {
        if world.get::<PrefabPart>(entity).is_none() {
            if let Some(&id) = world.get::<SceneId>(entity) {
                world.insert(entity, PrefabPart::new(id));
            }
        }
    }
    let record_of =
        |world: &World, entity: Entity| world.get::<PrefabPart>(entity).map(|p| p.record);
    let file_root_transform = {
        let handle = prefabs.load(&source);
        let record = record_of(world, root);
        prefabs
            .get(handle)
            .file
            .as_ref()
            .and_then(|file| file.entities.iter().find(|r| Some(r.id) == record))
            .and_then(|r| r.components.get("Transform").cloned())
    };

    // A reference between the instance's entities — a joint, a camera's
    // target — names their records in the file.
    let to_record: std::collections::HashMap<SceneId, SceneId> = entities
        .iter()
        .filter_map(|&e| Some((*world.get::<SceneId>(e)?, record_of(world, e)?)))
        .collect();
    let mut records = Vec::with_capacity(entities.len());
    for &entity in &entities {
        let Some(record) = record_entity(world, registry, entity) else {
            continue;
        };
        let mut record = record?;
        let Some(id) = record_of(world, entity) else {
            continue;
        };
        record.id = id;
        record.components.remove(PrefabPart::NAME);
        for (name, raw) in record.components.iter_mut() {
            if let Some(mapped) =
                registry.rewrite_refs(name, raw, &|id| to_record.get(&id).copied())?
            {
                *raw = mapped;
            }
        }
        if entity == root {
            record.components.remove(PrefabInstance::NAME);
            record.components.remove("Parent");
            let placement = match &file_root_transform {
                Some(kept) => kept.clone(),
                None => {
                    let at_origin = world.get::<Transform>(entity).map(|t| Transform {
                        translation: voltra_render::glam::Vec2::ZERO,
                        ..*t
                    });
                    match at_origin {
                        Some(t) => RawValue::from_rust(&t).map_err(SceneError::Serialize)?,
                        None => continue,
                    }
                }
            };
            record.components.insert("Transform".to_owned(), placement);
        } else if let Some(parent) = hierarchy::parent_of(world, entity) {
            if let Some(parent_record) = record_of(world, parent) {
                let link = Parent {
                    id: parent_record,
                    entity: None,
                };
                let value = RawValue::from_rust(&link).map_err(SceneError::Serialize)?;
                record.components.insert("Parent".to_owned(), value);
            }
        }
        records.push(record);
    }
    let file = SceneFile {
        version: VERSION,
        entities: records,
    };

    if let Some(through) = reaches(prefabs, nested_in(&file), &source) {
        return Err(WriteError::Cycle {
            target: source,
            through,
        });
    }
    let path = prefabs.root().join(source.as_str());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(SceneError::Io)?;
    }
    crate::format::write(&file, &path)?;

    // The instance is the prefab now.
    for &entity in &entities {
        if let Some(part) = world.get_mut::<PrefabPart>(entity) {
            part.overridden.clear();
        }
    }
    if let Some(link) = world.get_mut::<PrefabInstance>(root) {
        link.removed.clear();
    }
    prefabs.reload(&source);
    Ok(sync_source(world, registry, prefabs, &source))
}
