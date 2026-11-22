//! Comparing an instance part with its record: which components a sync
//! tracks, what they are in the file, and where the part differs.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use ron::value::RawValue;
use voltra_ecs::{Entity, World};

use super::{PrefabInstance, PrefabPart};
use crate::format::{ComponentRegistry, EntityRecord, SceneError};
use crate::hierarchy::Parent;
use crate::scene_id::SceneId;

/// The components a sync rebuilds rather than copies.
pub(super) const STRUCTURE: [&str; 3] = ["Parent", PrefabInstance::NAME, PrefabPart::NAME];

/// Whether a sync keeps `name` in step on an entity: not structure, and not
/// the instance root's placement.
pub(super) fn tracked(name: &str, is_root: bool) -> bool {
    !STRUCTURE.contains(&name) && !(is_root && name == "Transform")
}

/// A record's components as their types write them, structure left out and
/// components this build does not know skipped.
pub(super) fn canonical(
    registry: &ComponentRegistry,
    record: &EntityRecord,
) -> Result<BTreeMap<String, Box<RawValue>>, SceneError> {
    let mut out = BTreeMap::new();
    for (name, raw) in &record.components {
        if STRUCTURE.contains(&name.as_str()) {
            continue;
        }
        // A component this build does not know is skipped: it cannot be
        // compared, and a sync never touches what it cannot read.
        if let Some(value) = registry.normalize(name, raw) {
            out.insert(name.clone(), value?);
        }
    }
    Ok(out)
}

/// The components of `entity` that differ from `want` (a record's
/// [`canonical`] values) — changed, missing or extra — among those a sync
/// tracks. What an instance part overrides.
pub(super) fn differences(
    world: &World,
    registry: &ComponentRegistry,
    entity: Entity,
    is_root: bool,
    want: &BTreeMap<String, Box<RawValue>>,
) -> Result<BTreeSet<String>, SceneError> {
    let mut differ = BTreeSet::new();
    for name in registry.names() {
        if !tracked(name, is_root) {
            continue;
        }
        let have = registry.save_one(world, entity, name).transpose()?;
        let same = match (want.get(name), &have) {
            (Some(want), Some(have)) => want.get_ron() == have.get_ron(),
            (None, None) => true,
            _ => false,
        };
        if !same {
            differ.insert(name.to_owned());
        }
    }
    Ok(differ)
}

/// The parent record a record names, if it carries one.
pub(super) fn parent_record(record: &EntityRecord) -> Option<SceneId> {
    let raw = record.components.get("Parent")?;
    raw.into_rust::<Parent>().ok().map(|parent| parent.id)
}

/// `top` and every record under it in `records`, in file order.
pub(super) fn subtree(records: &[EntityRecord], top: SceneId) -> Vec<&EntityRecord> {
    let mut inside: HashSet<SceneId> = HashSet::from([top]);
    // Passes until nothing joins: a child can come before its parent in a
    // file. Bounded by the record count, so a cycle ends.
    for _ in 0..=records.len() {
        let before = inside.len();
        for record in records {
            if parent_record(record).is_some_and(|parent| inside.contains(&parent)) {
                inside.insert(record.id);
            }
        }
        if inside.len() == before {
            break;
        }
    }
    records.iter().filter(|r| inside.contains(&r.id)).collect()
}
