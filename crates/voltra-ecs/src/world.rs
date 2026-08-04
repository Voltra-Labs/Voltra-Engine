//! The container that ties entities to their components.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::entity::{Entities, Entity};
use crate::storage::SparseSet;

/// A storage the world can act on without knowing its component type.
///
/// `Any` alone is not enough: despawning has to clear an entity out of *every*
/// storage, and that needs a call that works before the type is recovered.
trait ErasedStorage: Any {
    fn remove_entity(&mut self, entity: Entity);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static> ErasedStorage for SparseSet<T> {
    fn remove_entity(&mut self, entity: Entity) {
        self.remove(entity);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Entities and their components.
#[derive(Default)]
pub struct World {
    entities: Entities,
    /// One sparse set per component type, keyed by that type's `TypeId`.
    storages: HashMap<TypeId, Box<dyn ErasedStorage>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self) -> Entity {
        self.entities.spawn()
    }

    /// Despawns an entity and drops all of its components.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        if !self.entities.is_alive(entity) {
            return false;
        }

        // Order matters: `Entities::despawn` bumps the generation, after which
        // every storage would treat this handle as stale and refuse to remove
        // anything, leaking a component per dead entity.
        for storage in self.storages.values_mut() {
            storage.remove_entity(entity);
        }

        self.entities.despawn(entity)
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.entities.is_alive(entity)
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    fn storage<T: 'static>(&self) -> Option<&SparseSet<T>> {
        self.storages
            .get(&TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref())
    }

    fn storage_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        self.storages
            .get_mut(&TypeId::of::<T>())
            .and_then(|s| s.as_any_mut().downcast_mut())
    }

    /// Adds or replaces a component, returning the value it replaced.
    ///
    /// Returns `None` for a dead or stale handle without storing anything —
    /// components on dead entities would never be reachable or reclaimed.
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) -> Option<T> {
        if !self.entities.is_alive(entity) {
            return None;
        }

        self.storages
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(SparseSet::<T>::new()))
            .as_any_mut()
            .downcast_mut::<SparseSet<T>>()
            .expect("storage keyed by TypeId::of::<T> always holds a SparseSet<T>")
            .insert(entity, component)
    }

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.storage::<T>()?.get(entity)
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.storage_mut::<T>()?.get_mut(entity)
    }

    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        self.storage_mut::<T>()?.remove(entity)
    }

    /// How many entities hold a `T`. Mostly useful for asserting on leaks.
    pub fn component_count<T: 'static>(&self) -> usize {
        self.storage::<T>().map_or(0, SparseSet::len)
    }

    /// Every entity holding a `T`, in dense order.
    pub fn query<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.storage::<T>().into_iter().flat_map(SparseSet::iter)
    }

    pub fn query_mut<T: 'static>(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        self.storage_mut::<T>()
            .into_iter()
            .flat_map(SparseSet::iter_mut)
    }

    /// Every entity holding both an `A` and a `B`.
    ///
    /// Walks `A`'s dense array and looks each entity up in `B`. Iterating
    /// whichever set is smaller would cut the lookups, but the two branches
    /// have different iterator types; worth doing once a profile asks for it.
    pub fn query2<A: 'static, B: 'static>(&self) -> impl Iterator<Item = (Entity, &A, &B)> {
        let b = self.storage::<B>();
        self.storage::<A>()
            .into_iter()
            .flat_map(SparseSet::iter)
            .filter_map(move |(entity, a)| Some((entity, a, b?.get(entity)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Clone, Copy)]
    struct Position(f32, f32);

    #[derive(Debug, PartialEq, Clone, Copy)]
    struct Velocity(f32, f32);

    #[derive(Debug, PartialEq)]
    struct Name(&'static str);

    #[test]
    fn spawned_entity_is_alive_and_counted() {
        let mut world = World::new();
        let e = world.spawn();

        assert!(world.is_alive(e));
        assert_eq!(world.entity_count(), 1);
    }

    #[test]
    fn components_round_trip() {
        let mut world = World::new();
        let e = world.spawn();

        assert_eq!(world.insert(e, Position(1.0, 2.0)), None);
        assert_eq!(world.get::<Position>(e), Some(&Position(1.0, 2.0)));
    }

    #[test]
    fn different_component_types_do_not_collide() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position(1.0, 2.0));
        world.insert(e, Velocity(3.0, 4.0));

        assert_eq!(world.get::<Position>(e), Some(&Position(1.0, 2.0)));
        assert_eq!(world.get::<Velocity>(e), Some(&Velocity(3.0, 4.0)));
    }

    #[test]
    fn get_mut_writes_through() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position(0.0, 0.0));

        world.get_mut::<Position>(e).unwrap().0 = 5.0;

        assert_eq!(world.get::<Position>(e), Some(&Position(5.0, 0.0)));
    }

    #[test]
    fn reading_an_absent_component_type_is_none() {
        let mut world = World::new();
        let e = world.spawn();

        assert_eq!(world.get::<Name>(e), None, "no storage exists yet");
    }

    #[test]
    fn remove_returns_the_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position(1.0, 1.0));

        assert_eq!(world.remove::<Position>(e), Some(Position(1.0, 1.0)));
        assert_eq!(world.get::<Position>(e), None);
    }

    #[test]
    fn inserting_on_a_dead_entity_is_refused() {
        let mut world = World::new();
        let e = world.spawn();
        world.despawn(e);

        assert_eq!(world.insert(e, Position(1.0, 1.0)), None);
        assert_eq!(
            world.get::<Position>(e),
            None,
            "a dead entity must not acquire components"
        );
    }

    #[test]
    fn despawn_drops_the_components() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position(1.0, 1.0));
        world.insert(e, Velocity(1.0, 1.0));

        world.despawn(e);

        // Left behind, these leak one component per dead entity forever.
        assert_eq!(world.component_count::<Position>(), 0);
        assert_eq!(world.component_count::<Velocity>(), 0);
    }

    #[test]
    fn a_recycled_entity_does_not_inherit_components() {
        let mut world = World::new();
        let old = world.spawn();
        world.insert(old, Position(9.0, 9.0));
        world.despawn(old);

        let new = world.spawn();

        assert_eq!(new.index(), old.index(), "the slot was reused");
        assert_eq!(
            world.get::<Position>(new),
            None,
            "but the previous tenant's data must not come with it"
        );
    }

    #[test]
    fn query_visits_only_entities_with_the_component() {
        let mut world = World::new();
        let a = world.spawn();
        let b = world.spawn();
        let _c = world.spawn();
        world.insert(a, Position(1.0, 0.0));
        world.insert(b, Position(2.0, 0.0));

        let mut seen: Vec<(Entity, f32)> =
            world.query::<Position>().map(|(e, p)| (e, p.0)).collect();
        seen.sort_by_key(|x| x.0);

        assert_eq!(seen, vec![(a, 1.0), (b, 2.0)]);
    }

    #[test]
    fn query_on_an_unknown_component_yields_nothing() {
        let world = World::new();
        assert_eq!(world.query::<Name>().count(), 0);
    }

    #[test]
    fn query_mut_writes_through() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position(1.0, 1.0));

        for (_, position) in world.query_mut::<Position>() {
            position.0 += 10.0;
        }

        assert_eq!(world.get::<Position>(e), Some(&Position(11.0, 1.0)));
    }

    #[test]
    fn query2_only_yields_entities_holding_both() {
        let mut world = World::new();
        let both = world.spawn();
        let only_position = world.spawn();
        let only_velocity = world.spawn();

        world.insert(both, Position(0.0, 0.0));
        world.insert(both, Velocity(1.0, 2.0));
        world.insert(only_position, Position(5.0, 5.0));
        world.insert(only_velocity, Velocity(9.0, 9.0));

        let found: Vec<Entity> = world
            .query2::<Position, Velocity>()
            .map(|(e, _, _)| e)
            .collect();

        assert_eq!(found, vec![both]);
    }

    #[test]
    fn query2_survives_a_despawn_of_an_unrelated_entity() {
        let mut world = World::new();
        let keep = world.spawn();
        let drop = world.spawn();
        for e in [keep, drop] {
            world.insert(e, Position(0.0, 0.0));
            world.insert(e, Velocity(1.0, 1.0));
        }

        world.despawn(drop);

        let found: Vec<Entity> = world
            .query2::<Position, Velocity>()
            .map(|(e, _, _)| e)
            .collect();
        assert_eq!(found, vec![keep]);
    }
}
