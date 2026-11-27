//! A prefab file read as what it spawns: every instance stored in it — a
//! variant's root, a prefab nested inside — brought in step with the file
//! it is an instance of, as an instance in a scene is.
//!
//! **A stored instance is stored as a scene stores one**: its records in
//! full, a [`PrefabInstance`] on its root and a [`PrefabPart`] on each,
//! naming the record of the inner file it is and what it overrides. So a
//! prefab's own overrides on an instance it holds — a variant's on its
//! base, an outer prefab's on a nested one — are the same overrides, in the
//! same format, an instance in a scene has on its prefab, and resolving
//! them is the same [`sync`]: in a scratch world, onto the file's records,
//! and back out. An instance of the result in a scene is a second layer of
//! overrides over the first.
//!
//! The file is read whole by everything that reads scenes: a reader that
//! does not resolve (an older build, a streamed region) gets the stored
//! instance as it was last saved, and an inner file that will not read
//! leaves it so.

use std::collections::{HashMap, HashSet};
use std::fmt;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use super::compare::parent_record;
use super::membership::{is_added, members};
use super::sync::sync_with;
use super::{Prefab, PrefabInstance};
use crate::format::{record_entity, ComponentRegistry, SceneError, SceneFile};
use crate::scene_id::SceneId;

/// How deep stored instances may go — a variant of a variant of …, a prefab
/// nested in a prefab nested in … — before the rest is left as its file
/// has it. Bounds the store's recursion on a hand-made chain.
pub const MAX_DEPTH: usize = 32;

/// How many records a prefab may hold once its stored instances are
/// resolved. Each nesting level can multiply what the level below adds, so
/// a few small files could otherwise ask for billions; past this, an
/// instance is left as its file has it.
pub const MAX_RECORDS: usize = 1 << 17;

/// How many records of one prefab file may carry a link — be the root of an
/// instance stored in it — before the store follows none of them. A level
/// with this many prefab instances is read as a scene instead
/// ([`super::Prefabs::load_scene`]), whose instances a sync brings in step.
pub const MAX_LINKS: usize = 1 << 14;

/// How many different prefabs one file may hold instances of before the
/// store follows none of them: each is a file to read.
pub const MAX_SOURCES: usize = 256;

/// Why an instance stored in a prefab was left as its file has it rather
/// than brought in step with its own file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveProblem {
    /// Following the instances leads back to a prefab already being read.
    Cycle {
        /// The prefabs on the way, the one being read first, ending with
        /// the one that closes the loop.
        chain: Vec<AssetPath>,
    },
    /// The instance of `source` is deeper than [`MAX_DEPTH`].
    TooDeep {
        /// The prefab the instance would follow.
        source: AssetPath,
    },
    /// Following `source` would take the prefab past [`MAX_RECORDS`].
    TooLarge {
        /// The prefab the instance would follow.
        source: AssetPath,
    },
    /// The file holds more stored instances than [`MAX_LINKS`], or of more
    /// prefabs than [`MAX_SOURCES`]: none are followed.
    TooManyLinks {
        /// Records carrying a link.
        links: usize,
        /// Different prefabs they name.
        sources: usize,
    },
    /// The instance of `source` would not come into step with it.
    Failed {
        /// The prefab the instance would follow.
        source: AssetPath,
        /// What went wrong.
        error: String,
    },
}

impl fmt::Display for ResolveProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cycle { chain } => {
                let names: Vec<&str> = chain.iter().map(AssetPath::as_str).collect();
                write!(f, "holds an instance of itself: {}", names.join(" › "))
            }
            Self::TooDeep { source } => write!(
                f,
                "its instance of {} is nested deeper than {MAX_DEPTH} prefabs",
                source.as_str()
            ),
            Self::TooLarge { source } => write!(
                f,
                "following {} would hold more than {MAX_RECORDS} entities",
                source.as_str()
            ),
            Self::TooManyLinks { links, sources } => write!(
                f,
                "holds {links} instances of {sources} prefabs, past the {MAX_LINKS} / \
                 {MAX_SOURCES} the store follows"
            ),
            Self::Failed { source, error } => {
                write!(
                    f,
                    "its instance of {} would not follow it: {error}",
                    source.as_str()
                )
            }
        }
    }
}

/// The instances stored in `file` that are not inside another stored in
/// it — what resolving starts from, as a sync starts from a scene's
/// outermost — by record id, with the prefab each is an instance of. Linear:
/// each record's answer is remembered for the records under it, and a loop
/// of parent links in a hand-edited file ends.
pub(super) fn stored_instances(file: &SceneFile) -> Vec<(SceneId, AssetPath)> {
    let parent: HashMap<SceneId, SceneId> = file
        .entities
        .iter()
        .filter_map(|r| Some((r.id, parent_record(r)?)))
        .collect();
    let mut unreadable = 0usize;
    let mut linked: HashMap<SceneId, AssetPath> = HashMap::new();
    for record in &file.entities {
        let Some(raw) = record.components.get(PrefabInstance::NAME) else {
            continue;
        };
        match raw.into_rust::<PrefabInstance>() {
            Ok(link) => {
                linked.entry(record.id).or_insert(link.source);
            }
            Err(_) => unreadable += 1,
        }
    }
    if unreadable > 0 {
        log::warn!("{unreadable} stored prefab link(s) could not be read");
    }
    // Whether a record or one above it is linked, remembered as it is found.
    let mut linked_here_or_above: HashMap<SceneId, bool> = HashMap::new();
    let mut above = |start: SceneId| -> bool {
        let mut path = Vec::new();
        let mut on_path = HashSet::new();
        let mut at = Some(start);
        let answer = loop {
            let Some(id) = at else {
                break false;
            };
            if let Some(&known) = linked_here_or_above.get(&id) {
                break known;
            }
            if linked.contains_key(&id) {
                break true;
            }
            if !on_path.insert(id) {
                break false;
            }
            path.push(id);
            at = parent.get(&id).copied();
        };
        for id in path {
            linked_here_or_above.insert(id, answer);
        }
        answer
    };
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for record in &file.entities {
        let Some(source) = linked.get(&record.id) else {
            continue;
        };
        let inside = parent.get(&record.id).is_some_and(|&p| above(p));
        if !inside && seen.insert(record.id) {
            found.push((record.id, source.clone()));
        }
    }
    found
}

/// `stored` with every instance in it whose prefab `sources` holds brought
/// in step with that prefab. Answers the file, or `None` when nothing
/// changed, and what was left behind. An instance whose prefab is not in
/// `sources` — one the store refused to follow — is left as it is.
pub(super) fn resolve(
    stored: &SceneFile,
    registry: &ComponentRegistry,
    sources: &HashMap<AssetPath, &Prefab>,
) -> Result<(Option<SceneFile>, Vec<ResolveProblem>), SceneError> {
    let instances = stored_instances(stored);
    let mut problems = Vec::new();
    if instances
        .iter()
        .all(|(_, source)| !sources.contains_key(source))
    {
        return Ok((None, problems));
    }

    let mut world = World::new();
    let entities = spawn_stored(&mut world, registry, stored)?;
    let by_id: HashMap<SceneId, Entity> = stored
        .entities
        .iter()
        .zip(&entities)
        .rev()
        .map(|(record, &entity)| (record.id, entity))
        .collect();

    let mut total = stored.entities.len();
    let mut changed = false;
    let mut too_large = false;
    // Outermost first, then what was added to each, inwards.
    let mut queue: Vec<Entity> = instances.iter().rev().map(|(id, _)| by_id[id]).collect();
    let mut done = HashSet::new();
    while let Some(root) = queue.pop() {
        if !world.is_alive(root) || !done.insert(root) {
            continue;
        }
        let Some(source) = world.get::<PrefabInstance>(root).map(|l| l.source.clone()) else {
            continue;
        };
        let Some(prefab) = sources.get(&source) else {
            continue;
        };
        total += prefab.len();
        if total > MAX_RECORDS {
            if !too_large {
                problems.push(ResolveProblem::TooLarge { source });
                too_large = true;
            }
            continue;
        }
        let file_ids: HashSet<SceneId> = prefab
            .file
            .as_ref()
            .map(|f| f.entities.iter().map(|r| r.id).collect())
            .unwrap_or_default();
        let (_, added) = members(&world, root, &mut |world, child| {
            is_added(world, child, &file_ids, &mut |source, record| {
                sources
                    .get(source)
                    .and_then(|p| p.file.as_ref())
                    .is_none_or(|f| f.entities.iter().any(|r| r.id == record))
            })
        });
        let added: HashSet<Entity> = added.into_iter().collect();
        match sync_with(&mut world, registry, prefab, root, Some(&added)) {
            Ok((report, inner)) => {
                changed |= !report.is_empty();
                queue.extend(inner);
            }
            Err(error) => problems.push(ResolveProblem::Failed {
                source,
                error: error.to_string(),
            }),
        }
    }
    if !changed {
        return Ok((None, problems));
    }
    Ok((Some(written(&world, registry, &entities)?), problems))
}

/// Spawns `file` into `world` under its own ids, its parents resolved.
/// Answers the entity of each record, in the file's order.
pub(super) fn spawn_stored(
    world: &mut World,
    registry: &ComponentRegistry,
    file: &SceneFile,
) -> Result<Vec<Entity>, SceneError> {
    let mut entities = Vec::with_capacity(file.entities.len());
    for record in &file.entities {
        let entity = world.spawn();
        entities.push(entity);
        world.insert(entity, record.id);
        let unknown =
            crate::format::record::insert_components(world, registry, entity, &record.components)?;
        if !unknown.0.is_empty() {
            world.insert(entity, unknown);
        }
    }
    crate::hierarchy::resolve_parents(world);
    Ok(entities)
}

/// The world written back as a file: the entities of `first`, still alive,
/// in their order, then what was added, by id — so a file's roots keep the
/// order they were saved in, the first root staying first.
pub(super) fn written(
    world: &World,
    registry: &ComponentRegistry,
    first: &[Entity],
) -> Result<SceneFile, SceneError> {
    let listed: HashSet<Entity> = first.iter().copied().collect();
    let mut added: Vec<(SceneId, Entity)> = world
        .query::<SceneId>()
        .filter(|(entity, _)| !listed.contains(entity))
        .map(|(entity, id)| (*id, entity))
        .collect();
    added.sort();
    let mut entities = Vec::with_capacity(first.len() + added.len());
    for entity in first
        .iter()
        .copied()
        .filter(|&e| world.is_alive(e))
        .chain(added.into_iter().map(|(_, e)| e))
    {
        if let Some(record) = record_entity(world, registry, entity) {
            entities.push(record?);
        }
    }
    Ok(SceneFile {
        version: crate::format::VERSION,
        entities,
    })
}
