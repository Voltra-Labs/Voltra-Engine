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
use super::membership::{instance_root, members, Children};
use super::write::WriteError;
use super::{placement, Placement, PrefabInstance, PrefabPart, Prefabs, SyncReport};
use crate::format::{record_entity, ComponentRegistry, SceneError, SceneFile, VERSION};
use crate::hierarchy::{self, Parent};
use crate::scene_id::SceneId;

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
/// the file's root keeps the transform it had, in either dimension
/// ([`Placement`]), as Unity's apply leaves the root's position out. Records the instance deleted leave the file.
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
    // The instance's own parts — not those of an instance added to it, which
    // name records of their own file, even ones this file has too (a coin
    // dropped into a coin).
    let belongs: HashSet<Entity> = members(world, root, &Children::of(world))
        .0
        .into_iter()
        .collect();
    // Which record each entity is: its part's, where the file has that
    // record; its own id for what the instance added. A part of an
    // instance dropped into this one becomes the record derived from that
    // instance's root and the record it names in its own file — the same
    // every time, so a scene that has the Apply undone (the dropped instance
    // back as it was, the file still written) is one a sync matches to the
    // file rather than given a second copy.
    let mut ids: HashMap<Entity, SceneId> = HashMap::with_capacity(entities.len());
    for &entity in &entities {
        let Some(&own) = world.get::<SceneId>(entity) else {
            continue;
        };
        let part = world.get::<PrefabPart>(entity).map(|p| p.record);
        let id = if belongs.contains(&entity) {
            part.filter(|record| known.contains(record)).unwrap_or(own)
        } else {
            let holder = instance_root(world, entity).and_then(|h| world.get::<SceneId>(h));
            match (part, holder) {
                (Some(record), Some(&holder)) => SceneId::derived(holder, record),
                _ => own,
            }
        };
        ids.insert(entity, id);
    }
    let record_of = |_: &World, entity: Entity| ids.get(&entity).copied();
    // The file's own root placement, in whichever dimensions it has one.
    let file_root =
        record_of(world, root).and_then(|top| resolved.entities.iter().find(|r| r.id == top));
    let file_placement: Vec<(&'static str, Box<RawValue>)> = Placement::NAMES
        .iter()
        .filter_map(|&name| Some((name, file_root?.components.get(name)?.clone())))
        .collect();

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
            record.components.remove(Parent::NAME);
            // Per transform the root carries, the file's where it has one and
            // the root's own at the origin where it has none. A root with no
            // transform keeps the file's; one whose dimension changed does
            // not get the old one beside its new one.
            let own = placement::at_origin(world, entity)?;
            let kept = |name: &str| {
                file_placement
                    .iter()
                    .find(|(kept, _)| *kept == name)
                    .map(|(_, value)| value.clone())
            };
            if own.is_empty() {
                for (name, value) in &file_placement {
                    record.components.insert((*name).to_owned(), value.clone());
                }
            }
            for (name, zeroed) in own {
                let value = kept(name).unwrap_or(zeroed);
                record.components.insert(name.to_owned(), value);
            }
        } else if let Some(parent) = hierarchy::parent_of(world, entity) {
            if let Some(parent_record) = record_of(world, parent) {
                let link = Parent {
                    id: parent_record,
                    entity: None,
                };
                let value = RawValue::from_rust(&link).map_err(SceneError::Serialize)?;
                record.components.insert(Parent::NAME.to_owned(), value);
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
        // An instance added to this one is a nested part of it now.
        if !belongs.contains(&entity) {
            if let Some(link) = world.get_mut::<PrefabInstance>(entity) {
                link.own = false;
            }
        }
        let names_its_record = belongs.contains(&entity)
            && world
                .get::<PrefabPart>(entity)
                .is_some_and(|part| known.contains(&part.record));
        match (names_its_record, ids.get(&entity)) {
            (false, Some(&id)) => {
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
