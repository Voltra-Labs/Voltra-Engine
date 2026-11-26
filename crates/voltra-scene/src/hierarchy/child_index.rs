//! Every parent's children, gathered in one pass over the world.
//!
//! [`children_of`](super::children_of) scans every [`Parent`] link to answer
//! for one entity, which is right for a single question and quadratic for a
//! walk: a hierarchy panel drawing a parent of 100k children spent 5.6 s a
//! frame asking each row for its children (stage 119's hardware pass). A
//! walk builds this once and asks it instead — Godot and Unity keep the child
//! list on the node for the same reason; here links live on the child, so the
//! list is derived when a walk needs it.

use std::collections::{HashMap, HashSet};

use voltra_ecs::{Entity, World};

use super::parent::Parent;

/// The children of every entity some link names, each list in the stable
/// order [`children_of`](super::children_of) gives.
#[derive(Debug, Default)]
pub struct ChildIndex {
    children: HashMap<Entity, Vec<Entity>>,
}

impl ChildIndex {
    /// Reads every [`Parent`] link in `world` once.
    ///
    /// A link naming a despawned entity is kept under that entity, as
    /// `children_of` would find it: nothing walks from a dead parent.
    pub fn new(world: &World) -> Self {
        let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
        for (child, link) in world.query::<Parent>() {
            if let Some(parent) = link.entity {
                children.entry(parent).or_default().push(child);
            }
        }
        for list in children.values_mut() {
            list.sort_unstable_by_key(Entity::index);
        }
        Self { children }
    }

    /// `parent`'s children; empty for an entity with none.
    pub fn children(&self, parent: Entity) -> &[Entity] {
        self.children.get(&parent).map_or(&[], Vec::as_slice)
    }

    /// `entity`'s children, their children, and so on, parents before
    /// children. Cycle-safe: an entity is reported once, and `entity` itself
    /// never.
    pub fn descendants(&self, entity: Entity) -> Vec<Entity> {
        let mut found = Vec::new();
        let mut seen = HashSet::from([entity]);
        let mut frontier = vec![entity];
        while let Some(current) = frontier.pop() {
            for &child in self.children(current) {
                if seen.insert(child) {
                    found.push(child);
                    frontier.push(child);
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hierarchy::{children_of, set_parent};
    use crate::{SceneId, Transform};

    fn spawn(world: &mut World) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Transform::default());
        entity
    }

    #[test]
    fn children_match_children_of_in_order() {
        let mut world = World::new();
        let parent = spawn(&mut world);
        let kids: Vec<Entity> = (0..5).map(|_| spawn(&mut world)).collect();
        for &kid in kids.iter().rev() {
            set_parent(&mut world, kid, parent).expect("parents");
        }
        let index = ChildIndex::new(&world);
        assert_eq!(
            index.children(parent),
            children_of(&world, parent).as_slice()
        );
        assert_eq!(index.children(parent), kids.as_slice());
        assert!(index.children(kids[0]).is_empty());
    }

    #[test]
    fn descendants_list_parents_before_children_and_stop_at_a_cycle() {
        let mut world = World::new();
        let root = spawn(&mut world);
        let child = spawn(&mut world);
        let grandchild = spawn(&mut world);
        set_parent(&mut world, child, root).expect("parents");
        set_parent(&mut world, grandchild, child).expect("parents");
        // A loop a hand-edited file could hold: `set_parent` refuses one.
        world.insert(root, Parent::resolved(SceneId::new(), grandchild));

        let found = ChildIndex::new(&world).descendants(root);
        assert_eq!(found, vec![child, grandchild]);
    }

    #[test]
    fn a_wide_parent_is_walked_in_one_pass() {
        let mut world = World::new();
        let root = spawn(&mut world);
        for _ in 0..50_000 {
            let kid = spawn(&mut world);
            world.insert(kid, Parent::resolved(SceneId::new(), root));
        }
        assert_eq!(ChildIndex::new(&world).descendants(root).len(), 50_000);
    }
}
