//! A copy of a prefab spawned into a world: fresh ids, links remapped,
//! roots placed and linked to their file.

use std::collections::{HashMap, HashSet};

use voltra_ecs::{Entity, World};

use super::{Prefab, PrefabInstance, PrefabPart};
use crate::format::{ComponentRegistry, EntityRecord, SceneError};
use crate::hierarchy::Parent;
use crate::scene_id::SceneId;
use crate::transform::Transform;

/// The name a parent link is stored under in a scene file.
const PARENT: &str = "Parent";

/// The entities one spawn made.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Instance {
    /// Every entity, in the prefab's own order.
    pub entities: Vec<Entity>,
    /// The ones with no parent inside the instance — the ones `at` placed.
    pub roots: Vec<Entity>,
}

impl Instance {
    /// The first root, which is *the* root of the common one-root prefab.
    pub fn root(&self) -> Option<Entity> {
        self.roots.first().copied()
    }
}

/// Spawns a copy of `prefab` into `world`, its roots placed by `at`.
///
/// What a copy means, rule by rule:
///
/// - **Every entity gets a fresh [`SceneId`].** Two instances of one prefab
///   sharing ids would be two entities a save writes under one name, and a
///   load that put one of them back.
/// - **Parent links inside the prefab are remapped** to the fresh ids and
///   resolved on the spot, without the whole-world pass a load needs: every
///   entity a link can name is one this call just made.
/// - **Joints between entities of the prefab are remapped** the same way; a
///   joint to a body outside it keeps naming that body.
/// - **A link to a parent outside the prefab is cut**, and its entity is a
///   root. The id it names belongs to whatever scene the fragment was cut
///   from, and following it would hang every instance off one entity there.
/// - **Roots are placed by `at`**, composed in front of their own transform,
///   so a prefab authored around the origin lands around `at` and keeps the
///   offsets it was authored with. Children are left alone: they are local to
///   their parent, and moving the root moved them. A root with no `Transform`
///   is not positional and is not given one.
/// - **Every root is linked to the prefab's source** with a
///   [`PrefabInstance`], replacing whatever link the record itself carried:
///   a root is an instance of the file it was just spawned from, in its own
///   right ([`PrefabInstance::own`]). A child's link is kept — that child is
///   a nested instance, a part of this one.
/// - **Every entity is told which record it is** with a [`PrefabPart`],
///   overriding nothing, which is what [`sync`] keeps it in step by.
///
/// All-or-nothing, like a load: a component that will not read despawns every
/// entity this call spawned before the error is returned.
pub fn spawn(
    world: &mut World,
    registry: &ComponentRegistry,
    prefab: &Prefab,
    at: &Transform,
) -> Result<Instance, SceneError> {
    spawn_copy(world, registry, prefab, Some(at), Ids::Fresh)
}

/// Spawns a copy of a whole scene file into `world`: an additive load, or the
/// new level of a scene change.
///
/// [`spawn`]'s rules — fresh ids, parents remapped, all-or-nothing — but
/// nothing is placed and nothing is linked: a level is not an instance of
/// itself, and the prefab links its own records carry (a coin placed in the
/// level) are kept as authored.
pub fn spawn_scene(
    world: &mut World,
    registry: &ComponentRegistry,
    scene: &Prefab,
) -> Result<Instance, SceneError> {
    spawn_copy(world, registry, scene, None, Ids::Fresh)
}

/// [`spawn_scene`], every entity keeping the id its record has: a level
/// loaded as *the* level of a networked session, whose two ends match
/// their copies of each level entity by id (stage 114's first complete
/// snapshot adopts the client's own by it). With fresh ids, a client's
/// level copy is never the server's: it is swept, and the server's comes
/// back bare or by prefab, without what the level gave it — a sprite.
///
/// A record whose id `held` answers true — an entity that stays in the
/// world through the change, a [`Persistent`](crate::Persistent) one the
/// level also holds — is left out, with what hangs under it in the file:
/// two entities never answer to one id. Ids the old level holds for the
/// moment before it is swept are the caller's to answer false for.
pub fn spawn_scene_as_authored(
    world: &mut World,
    registry: &ComponentRegistry,
    scene: &Prefab,
    held: &dyn Fn(SceneId) -> bool,
) -> Result<Instance, SceneError> {
    spawn_copy(world, registry, scene, None, Ids::Authored(held))
}

/// Which ids a copy's entities get.
#[derive(Clone, Copy)]
enum Ids<'a> {
    /// A fresh one each: a copy of a copy.
    Fresh,
    /// Their records' own: the level as authored, but for the ones this
    /// answers true for, left out.
    Authored(&'a dyn Fn(SceneId) -> bool),
}

/// The records of `records` to leave out: those `held` answers true for,
/// and every one under them in the file.
fn left_out(records: &[EntityRecord], held: &dyn Fn(SceneId) -> bool) -> HashSet<SceneId> {
    let mut out: HashSet<SceneId> = records
        .iter()
        .map(|record| record.id)
        .filter(|&id| held(id))
        .collect();
    if out.is_empty() {
        return out;
    }
    let parents: Vec<(SceneId, Option<SceneId>)> = records
        .iter()
        .map(|record| {
            let parent = record
                .components
                .get(PARENT)
                .and_then(|link| link.into_rust::<Parent>().ok())
                .map(|link| link.id);
            (record.id, parent)
        })
        .collect();
    // To a fixed point: a file need not list a parent before its children.
    loop {
        let before = out.len();
        for &(id, parent) in &parents {
            if parent.is_some_and(|parent| out.contains(&parent)) {
                out.insert(id);
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// The copy both share. `at` is `Some` for a prefab: roots placed by it and
/// linked to the source.
fn spawn_copy(
    world: &mut World,
    registry: &ComponentRegistry,
    prefab: &Prefab,
    at: Option<&Transform>,
    ids: Ids,
) -> Result<Instance, SceneError> {
    let Some(file) = prefab.file.as_ref() else {
        return Ok(Instance::default());
    };

    let skipped = match ids {
        Ids::Fresh => HashSet::new(),
        Ids::Authored(held) => left_out(&file.entities, held),
    };
    let records: Vec<&EntityRecord> = file
        .entities
        .iter()
        .filter(|record| !skipped.contains(&record.id))
        .collect();

    // One fresh id per record, and a map from the old ids for the links. The
    // map keeps the first record under a duplicated id, which is what a link
    // to it resolved to in the file as well.
    let fresh: Vec<SceneId> = records
        .iter()
        .map(|record| match ids {
            Ids::Fresh => SceneId::new(),
            Ids::Authored(_) => record.id,
        })
        .collect();
    let mut renamed: HashMap<SceneId, SceneId> = HashMap::with_capacity(fresh.len());
    for (record, id) in records.iter().zip(&fresh) {
        renamed.entry(record.id).or_insert(*id);
    }

    let mut entities = Vec::with_capacity(records.len());
    for (record, id) in records.iter().zip(&fresh) {
        let entity = world.spawn();
        entities.push(entity);
        world.insert(entity, *id);

        match crate::format::record::insert_components(world, registry, entity, &record.components)
        {
            Ok(unknown) if !unknown.0.is_empty() => {
                world.insert(entity, unknown);
            }
            Ok(_) => {}
            Err(error) => {
                for entity in entities {
                    world.despawn(entity);
                }
                return Err(error);
            }
        }
        // Which record it is, for keeping it in step with the file. A
        // prefab's copy only: a scene loaded additively keeps the parts its
        // own instances carry.
        if at.is_some() {
            world.insert(entity, PrefabPart::new(record.id));
            // An instance the file holds is a nested part of this one, here,
            // whatever it was where the file was cut: the roots are linked
            // as instances of their own below.
            if let Some(link) = world.get_mut::<PrefabInstance>(entity) {
                link.own = false;
            }
        }
    }

    let by_id: HashMap<SceneId, Entity> = fresh
        .iter()
        .copied()
        .zip(entities.iter().copied())
        .collect();
    let mut roots = Vec::new();
    for &entity in &entities {
        let Some(link) = world.get::<Parent>(entity).map(|link| link.id) else {
            roots.push(entity);
            continue;
        };
        match renamed.get(&link) {
            Some(&parent) => {
                world.insert(entity, Parent::resolved(parent, by_id[&parent]));
            }
            None => {
                log::debug!(
                    "prefab entity names a parent outside the prefab; spawning it as a root"
                );
                world.remove::<Parent>(entity);
                roots.push(entity);
            }
        }
    }

    // A reference between two entities of the copy — a joint, a camera's
    // target — names the copy's two.
    for &entity in &entities {
        registry.remap_refs(world, entity, &|id| renamed.get(&id).copied());
    }

    if let Some(at) = at {
        for &root in &roots {
            if let Some(transform) = world.get_mut::<Transform>(root) {
                *transform = place(at, transform);
            }
            if let Some(source) = &prefab.source {
                world.insert(root, PrefabInstance::new(source.clone()));
            }
        }
    }

    Ok(Instance { entities, roots })
}

/// `local` as seen through `at`.
///
/// A placement that only moves is applied as an addition rather than through
/// a matrix: it is the common case — a bullet at the muzzle, a coin on a tile
/// — and the decomposition [`Transform::from_matrix`] does would put float
/// noise into an authored rotation and scale that nothing asked to change.
fn place(at: &Transform, local: &Transform) -> Transform {
    if at.rotation == 0.0 && at.scale == voltra_render::glam::Vec2::ONE {
        return Transform {
            translation: local.translation + at.translation,
            ..*local
        };
    }
    Transform::from_matrix(at.matrix() * local.matrix())
}
