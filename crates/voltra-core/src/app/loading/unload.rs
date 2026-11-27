//! A load's entities taken out of the world over frames.
//!
//! What went in a unit at a time comes out the same way: a region of a few
//! thousand entities despawned in one frame is the stutter streaming exists
//! to avoid. The list is worked out by the unload's first unit — what the
//! load brought, and everything since parented under it (a bullet stuck in
//! a wall), found in one pass over the world's parent links — and
//! despawned children first, so a frame that stops half way leaves no
//! child pointing at a parent that is gone.
//!
//! A [`Persistent`] entity is kept, as a scene change keeps one: lifted out
//! of the region, where it stands, with everything under it. What it was
//! parented to goes.

use std::collections::{HashMap, HashSet};

use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, Parent, Persistent};

/// How many entities one unit despawns: a despawn is a few hundred
/// nanoseconds, too little to time one by one.
const CHUNK: usize = 64;

#[derive(Debug, Default)]
pub(super) struct Unload {
    /// What the load brought, until the first unit plans from it.
    brought: Option<Vec<Entity>>,
    /// Children before parents.
    going: Vec<Entity>,
    next: usize,
}

impl Unload {
    /// The unload of `entities` and what hangs under them, planned by its
    /// first [`step`](Self::step).
    pub(super) fn new(entities: Vec<Entity>) -> Self {
        Self {
            brought: Some(entities),
            going: Vec::new(),
            next: 0,
        }
    }

    /// Despawns the next batch — or, the first time, works out what goes.
    /// Returns whether everything is gone.
    pub(super) fn step(&mut self, world: &mut World) -> bool {
        if let Some(entities) = self.brought.take() {
            self.going = plan(world, &entities);
            return self.is_done();
        }
        let end = (self.next + CHUNK).min(self.going.len());
        for &entity in &self.going[self.next..end] {
            world.despawn(entity);
        }
        self.next = end;
        self.is_done()
    }

    pub(super) fn is_done(&self) -> bool {
        self.brought.is_none() && self.next >= self.going.len()
    }

    /// How many are still to go, once planned.
    #[cfg(test)]
    pub(super) fn remaining(&self) -> usize {
        self.going.len() - self.next
    }
}

/// `entities` and what hangs under them, children before parents, with the
/// persistent ones among them lifted out.
fn plan(world: &mut World, entities: &[Entity]) -> Vec<Entity> {
    let set: HashSet<Entity> = entities
        .iter()
        .copied()
        .filter(|entity| world.is_alive(*entity))
        .collect();
    if set.is_empty() {
        return Vec::new();
    }
    // One pass over the links for the whole unload, rather than one
    // per entity as `despawn_recursive` would make.
    let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
    for (child, link) in world.query::<Parent>() {
        if let Some(parent) = link.entity {
            children.entry(parent).or_default().push(child);
        }
    }

    let mut kept = Vec::new();
    let mut order = Vec::with_capacity(set.len());
    let mut seen = HashSet::with_capacity(set.len());
    let mut frontier: Vec<Entity> = entities
        .iter()
        .copied()
        .filter(|entity| {
            set.contains(entity)
                && hierarchy::parent_of(world, *entity).is_none_or(|p| !set.contains(&p))
        })
        .collect();
    // Parents first, through the links; then whatever a loop of links
    // kept the walk from reaching.
    let mut leftovers = entities.iter().copied();
    loop {
        while let Some(entity) = frontier.pop() {
            if !seen.insert(entity) {
                continue;
            }
            if world.get::<Persistent>(entity).is_some() {
                kept.push(entity);
                continue;
            }
            order.push(entity);
            if let Some(under) = children.get(&entity) {
                frontier.extend(under.iter().copied());
            }
        }
        match leftovers.find(|e| set.contains(e) && !seen.contains(e)) {
            Some(entity) => frontier.push(entity),
            None => break,
        }
    }

    for entity in kept {
        let parent_going = hierarchy::parent_of(world, entity).is_some_and(|p| seen.contains(&p));
        if parent_going {
            let placed = hierarchy::world_transform(world, entity);
            hierarchy::clear_parent(world, entity);
            world.insert(entity, placed);
        }
    }
    order.reverse();
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;
    use voltra_scene::{SceneId, Transform2D};

    fn spawn(world: &mut World, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Transform2D::from_translation(at));
        entity
    }

    fn drain(unload: &mut Unload, world: &mut World) {
        while !unload.step(world) {}
    }

    #[test]
    fn everything_brought_and_parented_since_goes_and_nothing_else() {
        let mut world = World::new();
        let wall = spawn(&mut world, Vec2::ZERO);
        let torch = spawn(&mut world, Vec2::ZERO);
        hierarchy::set_parent(&mut world, torch, wall).expect("parents");
        // Parented in by the game after the load.
        let arrow = spawn(&mut world, Vec2::ZERO);
        hierarchy::set_parent(&mut world, arrow, torch).expect("parents");
        let elsewhere = spawn(&mut world, Vec2::ZERO);

        let mut unload = Unload::new(vec![wall, torch]);
        unload.step(&mut world);
        assert_eq!(unload.remaining(), 3);
        drain(&mut unload, &mut world);

        assert!(!world.is_alive(wall) && !world.is_alive(torch) && !world.is_alive(arrow));
        assert!(world.is_alive(elsewhere));
    }

    #[test]
    fn children_go_before_their_parents() {
        let mut world = World::new();
        let mut chain = vec![spawn(&mut world, Vec2::ZERO)];
        for _ in 0..(CHUNK * 3) {
            let child = spawn(&mut world, Vec2::ZERO);
            let parent = *chain.last().expect("one at least");
            hierarchy::set_parent(&mut world, child, parent).expect("parents");
            chain.push(child);
        }

        let mut unload = Unload::new(chain.clone());
        while !unload.step(&mut world) {
            // Whatever is left is a whole chain from the top.
            let alive: Vec<bool> = chain.iter().map(|e| world.is_alive(*e)).collect();
            let first_dead = alive.iter().position(|a| !a).unwrap_or(alive.len());
            assert!(alive[first_dead..].iter().all(|a| !a));
        }
        assert!(chain.iter().all(|e| !world.is_alive(*e)));
    }

    #[test]
    fn a_persistent_entity_is_lifted_out_where_it_stands() {
        let mut world = World::new();
        let platform = spawn(&mut world, Vec2::new(10.0, 0.0));
        let key = spawn(&mut world, Vec2::new(1.0, 0.0));
        hierarchy::set_parent(&mut world, key, platform).expect("parents");
        world.insert(key, Persistent);

        let mut unload = Unload::new(vec![platform, key]);
        drain(&mut unload, &mut world);

        assert!(!world.is_alive(platform));
        assert!(world.is_alive(key));
        assert!(hierarchy::parent_of(&world, key).is_none());
        let at = world.get::<Transform2D>(key).expect("kept").translation;
        assert_eq!(at, Vec2::new(11.0, 0.0));
    }

    #[test]
    fn entities_already_gone_are_skipped() {
        let mut world = World::new();
        let a = spawn(&mut world, Vec2::ZERO);
        let b = spawn(&mut world, Vec2::ZERO);
        world.despawn(a);
        let mut unload = Unload::new(vec![a, b]);
        unload.step(&mut world);
        assert_eq!(unload.remaining(), 1);
        drain(&mut unload, &mut world);
        assert!(!world.is_alive(b));
    }
}
