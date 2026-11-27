//! Giving an instance back to its file: the selection made an instance of a
//! new prefab ([`link`]), and an edited instance written over the prefab it
//! came from ([`apply`]) — Unity's "Apply All", Godot's saving an edited
//! inherited scene.

mod stored;

use std::collections::{HashMap, HashSet};

use ron::value::RawValue;
use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use super::cycle::{nested_in, reaches};
use super::instances::sync_sources;
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

/// Writes the instance at `root` over its prefab, then brings every instance
/// of it — and of every prefab that follows it — in step. Answers what that
/// sync did.
///
/// What the file gets, entity by entity: everything the entity carries,
/// under its record's id — an entity the instance added becomes a record of
/// its own — with parents named by record. Not the links and parts, which
/// belong to instances, not to the prefab; and not the root's placement:
/// the file's root keeps the transform it had, as Unity's apply leaves the
/// root's position out. Records the instance deleted leave the file.
/// Afterwards the instance overrides nothing — it *is* the prefab.
///
/// **Instances the file holds stay instances.** A variant's root stays an
/// instance of its base and a nested prefab an instance of its file: their
/// links and parts are kept from the file, and what they override is
/// worked out again against their own files — so Apply on an instance of a
/// variant writes the variant, its edits becoming the variant's overrides,
/// and leaves the base alone (Unity asks which; Apply from inside the
/// variant, in prefab mode, writes the base).
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
    let handle = prefabs.load(&source);
    let resolved = prefabs.get(handle).file.clone().unwrap_or(SceneFile {
        version: VERSION,
        entities: Vec::new(),
    });
    let known: HashSet<SceneId> = resolved.entities.iter().map(|r| r.id).collect();
    // Which record each entity is: its part's, where the file has that
    // record; its own id otherwise — what the instance added, and what an
    // instance dropped into it brought, is a record of its own from now on.
    let record_of = |world: &World, entity: Entity| {
        world
            .get::<PrefabPart>(entity)
            .map(|p| p.record)
            .filter(|record| known.contains(record))
            .or_else(|| world.get::<SceneId>(entity).copied())
    };
    let file_root_transform = record_of(world, root)
        .and_then(|top| resolved.entities.iter().find(|r| r.id == top))
        .and_then(|r| r.components.get("Transform").cloned());

    // A reference between the instance's entities — a joint, a camera's
    // target — names their records in the file.
    let to_record: HashMap<SceneId, SceneId> = entities
        .iter()
        .filter_map(|&e| Some((*world.get::<SceneId>(e)?, record_of(world, e)?)))
        .collect();
    let mut records = Vec::with_capacity(entities.len());
    let mut kept = Vec::with_capacity(entities.len());
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
        kept.push(entity);
    }
    stored::restore(world, prefabs, &resolved, &kept, &mut records, root)?;
    let file = SceneFile {
        version: VERSION,
        entities: records,
    };
    let file = stored::refresh(registry, prefabs, &file)?;

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
        let own = !world
            .get::<PrefabPart>(entity)
            .is_some_and(|part| known.contains(&part.record));
        match (own, world.get::<SceneId>(entity).copied()) {
            (true, Some(id)) => {
                world.insert(entity, PrefabPart::new(id));
            }
            _ => {
                if let Some(part) = world.get_mut::<PrefabPart>(entity) {
                    part.overridden.clear();
                }
            }
        }
    }
    if let Some(link) = world.get_mut::<PrefabInstance>(root) {
        link.removed.clear();
    }
    let changed = prefabs.reload(&source);
    Ok(sync_sources(world, registry, prefabs, &changed))
}
