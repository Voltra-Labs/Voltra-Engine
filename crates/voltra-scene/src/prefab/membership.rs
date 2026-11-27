//! Which instance an entity belongs to, when instances sit inside instances.
//!
//! An instance can hold two kinds of instance root under it: a **nested**
//! one — a prefab the instance's file holds, whose entities are parts of
//! the outer instance (their `PrefabPart`s name records of the *outer*
//! file) — and an **added** one, a prefab dropped under a part afterwards
//! or made there from a selection, whose parts name records of *its own*
//! file. A sync of the outer instance must leave an added one alone (it is
//! the instance's own, as an added entity is) and sync it with its own file
//! instead; taking its parts for the outer file's would despawn them all as
//! lost, or — a coin dropped into a coin names the coin's own records — let
//! two entities claim one record.
//!
//! **Which is which is recorded, not inferred**: an added root's link says
//! [`PrefabInstance::own`]. A spawn links its roots so and a nested link it
//! copies not; Create prefab's link is its own. The records a part names
//! cannot decide it — the case above, and a row made from a selection
//! holding a coin, whose record for the coin is the coin file's own id.

use std::collections::{HashMap, HashSet};

use voltra_ecs::{Entity, World};

use super::PrefabInstance;
use crate::hierarchy::{self, Parent};

/// Every entity's children, gathered once for a walk over many instances:
/// asking the world for one entity's children is a pass over every link.
#[derive(Debug, Default)]
pub(super) struct Children(HashMap<Entity, Vec<Entity>>);

impl Children {
    /// The children of every entity in `world`, each list in a stable order.
    pub(super) fn of(world: &World) -> Self {
        let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
        for (entity, link) in world.query::<Parent>() {
            if let Some(parent) = link.entity {
                children.entry(parent).or_default().push(entity);
            }
        }
        for list in children.values_mut() {
            list.sort_unstable_by_key(Entity::index);
        }
        Self(children)
    }

    fn get(&self, parent: Entity) -> &[Entity] {
        self.0.get(&parent).map_or(&[], Vec::as_slice)
    }
}

/// Whether `entity` is the root of an instance in its own right.
pub(super) fn is_own(world: &World, entity: Entity) -> bool {
    world.get::<PrefabInstance>(entity).is_some_and(|l| l.own)
}

/// The instance at `root`: `root` and every entity under it that is part of
/// it, parents before children — and the roots of the instances added to
/// it, which with what is under them are left out. Bounded: a loop of
/// parent links is walked once.
pub(super) fn members(
    world: &World,
    root: Entity,
    children: &Children,
) -> (Vec<Entity>, Vec<Entity>) {
    let mut inside = vec![root];
    let mut added = Vec::new();
    let mut seen = HashSet::from([root]);
    let mut at = 0;
    while at < inside.len() {
        let current = inside[at];
        at += 1;
        for &child in children.get(current) {
            if !seen.insert(child) || !world.is_alive(child) {
                continue;
            }
            if is_own(world, child) {
                added.push(child);
            } else {
                inside.push(child);
            }
        }
    }
    (inside, added)
}

/// The instance `entity` belongs to: the nearest of itself and its
/// ancestors carrying a [`PrefabInstance`], then outwards through every
/// instance it is a nested part of — up to one in its own right, or the
/// outermost. `None` for an entity in no instance. Walks only upwards.
pub fn instance_root(world: &World, entity: Entity) -> Option<Entity> {
    let mut found = None;
    for at in std::iter::once(entity).chain(hierarchy::ancestors(world, entity)) {
        if world.get::<PrefabInstance>(at).is_none() {
            continue;
        }
        found = Some(at);
        if is_own(world, at) {
            break;
        }
    }
    found
}

/// Whether `entity` is a part of the instance at `root` — not of one added
/// under it. Walks only upwards.
pub(super) fn belongs_to(world: &World, entity: Entity, root: Entity) -> bool {
    instance_root(world, entity) == Some(root)
}
