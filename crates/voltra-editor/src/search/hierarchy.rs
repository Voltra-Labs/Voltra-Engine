//! A search over the scene: which entities match, and which rows the tree
//! keeps to show where they are.

use std::collections::HashSet;

use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, ComponentRegistry, Name, SceneId};

use super::Query;

/// What a search found in the scene.
#[derive(Debug, Default)]
pub(crate) struct HierarchyFilter {
    /// The entities the query matches.
    pub(crate) matched: HashSet<Entity>,
    /// Those and every ancestor of one: the rows the tree draws, so a match
    /// three levels down is shown where it is rather than torn out of its
    /// place. Godot's tree filter keeps the path the same way; Unity's
    /// flattens it, and loses where each match lives.
    pub(crate) shown: HashSet<Entity>,
    /// `t:` terms no registered component is called, for the panel to say
    /// so rather than show an empty tree with no reason.
    pub(crate) unknown: Vec<String>,
}

/// What a row calls an entity, and what a search matches words against:
/// its name, or its index when it has none (or an empty one, mid-rename) —
/// what the inspector shows too, so the panels agree.
pub(crate) fn label(world: &World, entity: Entity) -> String {
    match world.get::<Name>(entity) {
        Some(name) if !name.as_str().is_empty() => name.as_str().to_owned(),
        _ => format!("Entity {}", entity.index()),
    }
}

/// Every scene entity `query` matches: its label holds every word, and it
/// has a component of every `t:` type — all of them, so `t:Sprite2D
/// t:Collider2D` narrows rather than widens. A type is the registered name,
/// case aside; one no component is called matches nothing and is reported.
pub(crate) fn filter(
    world: &World,
    registry: &ComponentRegistry,
    query: &Query,
) -> HierarchyFilter {
    let mut found = HierarchyFilter::default();
    let mut components = Vec::new();
    for typed in query.types() {
        match registry
            .names()
            .find(|name| name.eq_ignore_ascii_case(typed))
        {
            Some(name) => components.push(name),
            None => found.unknown.push(typed.clone()),
        }
    }
    if !found.unknown.is_empty() {
        return found;
    }

    for (entity, _) in world.query::<SceneId>() {
        let has_all = components
            .iter()
            .all(|name| registry.has(world, entity, name) == Some(true));
        if has_all && query.name_matches(&label(world, entity)) {
            found.matched.insert(entity);
        }
    }
    for &entity in &found.matched {
        let mut at = Some(entity);
        // `insert` answering false is the walk meeting a row already shown —
        // from another match, or a parent cycle in a hand-edited file — and
        // everything above it is shown already.
        while let Some(row) = at.filter(|&row| found.shown.insert(row)) {
            at = hierarchy::parent_of(world, row);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{Collider2D, Sprite2D, Transform2D};

    fn named(world: &mut World, name: &str) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Name::new(name));
        world.insert(entity, Transform2D::default());
        entity
    }

    /// level ▸ coins ▸ gold coin (a sprite), and a box beside them.
    fn scene() -> (World, [Entity; 4]) {
        let mut world = World::new();
        let level = named(&mut world, "Level");
        let coins = named(&mut world, "Coins");
        let gold = named(&mut world, "Gold coin");
        let boxed = named(&mut world, "Box");
        world.insert(gold, Sprite2D::default());
        world.insert(boxed, Sprite2D::default());
        world.insert(boxed, Collider2D::default());
        hierarchy::set_parent(&mut world, coins, level).expect("a parent");
        hierarchy::set_parent(&mut world, gold, coins).expect("a parent");
        (world, [level, coins, gold, boxed])
    }

    #[test]
    fn a_match_keeps_the_rows_above_it_and_nothing_beside_it() {
        let (world, [level, coins, gold, boxed]) = scene();
        let registry = ComponentRegistry::with_defaults();

        let found = filter(&world, &registry, &Query::parse("gold"));

        assert_eq!(found.matched, HashSet::from([gold]));
        assert_eq!(found.shown, HashSet::from([level, coins, gold]));
        assert!(!found.shown.contains(&boxed));
    }

    #[test]
    fn a_type_is_a_component_name_and_every_one_must_be_there() {
        let (world, [_, _, gold, boxed]) = scene();
        let registry = ComponentRegistry::with_defaults();

        let sprites = filter(&world, &registry, &Query::parse("t:sprite2d"));
        assert_eq!(sprites.matched, HashSet::from([gold, boxed]));

        let solid = filter(&world, &registry, &Query::parse("t:Sprite2D t:Collider2D"));
        assert_eq!(solid.matched, HashSet::from([boxed]));

        let named_too = filter(&world, &registry, &Query::parse("t:Sprite2D coin"));
        assert_eq!(named_too.matched, HashSet::from([gold]));
    }

    #[test]
    fn a_type_nothing_is_called_is_reported_and_matches_nothing() {
        let (world, _) = scene();
        let registry = ComponentRegistry::with_defaults();

        let found = filter(&world, &registry, &Query::parse("t:Spirte"));

        assert!(found.matched.is_empty() && found.shown.is_empty());
        assert_eq!(found.unknown, ["spirte"]);
    }
}
