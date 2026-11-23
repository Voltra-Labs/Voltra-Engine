//! Every [`UiRect`] resolved to a box on a screen of a given size.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::rect::{Rect, UiRect};
use super::stack::UiStack;
use crate::hierarchy;

/// The screen box of every entity with a [`UiRect`], and how many UI
/// ancestors it has.
///
/// Computed once per frame for the whole world, as [`WorldTransforms`] is,
/// so a deep panel tree costs one walk rather than one per element per
/// question. Rebuilt from the screen size each time, which is what makes a
/// resize — a window dragged, an editor panel resized — just the next frame.
///
/// [`WorldTransforms`]: crate::WorldTransforms
#[derive(Debug, Clone, Default)]
pub struct UiLayout {
    resolved: HashMap<Entity, (Rect, u32)>,
}

impl UiLayout {
    /// Resolves every `UiRect` against `screen`, in points.
    ///
    /// The parent rect is the nearest ancestor that has a `UiRect`; an
    /// ancestor without one is passed over, and with none at all the parent
    /// is the screen.
    pub fn from_world(world: &World, screen: Vec2) -> Self {
        let screen = Rect::screen(screen);
        let mut layout = Self::default();
        for (entity, _) in world.query::<UiRect>() {
            layout.resolve(world, entity, screen);
        }
        layout
    }

    /// Resolves `entity` and its UI ancestors, top-down from the root.
    ///
    /// Iterative over one [`hierarchy::ancestors`] walk, never recursive: that
    /// walk is bounded by `MAX_DEPTH` and stops at a cycle, so a scene file
    /// with a pathologically deep or looping chain costs a bounded loop
    /// rather than the stack. An ancestor already resolved — by an earlier
    /// entity sharing the chain — is reused, not recomputed.
    fn resolve(&mut self, world: &World, entity: Entity, screen: Rect) {
        if self.resolved.contains_key(&entity) {
            return;
        }
        let mut chain: Vec<Entity> = hierarchy::ancestors(world, entity)
            .into_iter()
            .filter(|&ancestor| world.get::<UiRect>(ancestor).is_some())
            .collect();
        chain.reverse();
        chain.push(entity);

        let mut parent: Option<(Entity, Rect, u32)> = None;
        for link in chain {
            if let Some(&(rect, depth)) = self.resolved.get(&link) {
                parent = Some((link, rect, depth));
                continue;
            }
            let Some(rect) = world.get::<UiRect>(link) else {
                continue;
            };
            let resolved = match parent {
                Some((above, parent_rect, depth)) => {
                    let placed = self
                        .stacked(world, above, parent_rect, depth + 1, link)
                        .unwrap_or_else(|| rect.resolve(parent_rect));
                    (placed, depth + 1)
                }
                None => (rect.resolve(screen), 0),
            };
            self.resolved.insert(link, resolved);
            parent = Some((link, resolved.0, resolved.1));
        }
    }

    /// Where `child` goes when `parent` stacks it: the whole stack is
    /// arranged at once, every child of it recorded at `depth`, since where
    /// one goes depends on all the others. `None` when `parent` is no
    /// [`UiStack`] or `child` is not its own child.
    fn stacked(
        &mut self,
        world: &World,
        parent: Entity,
        parent_rect: Rect,
        depth: u32,
        child: Entity,
    ) -> Option<Rect> {
        let stack = world.get::<UiStack>(parent)?;
        if hierarchy::parent_of(world, child) != Some(parent) {
            return None;
        }
        let mut found = None;
        for (entity, rect) in stack.arrange(world, parent, parent_rect) {
            if entity == child {
                found = Some(rect);
            }
            self.resolved.entry(entity).or_insert((rect, depth));
        }
        found
    }

    /// The box `entity` occupies, or `None` if it has no `UiRect`.
    pub fn rect(&self, entity: Entity) -> Option<Rect> {
        self.resolved.get(&entity).map(|(rect, _)| *rect)
    }

    /// How many UI ancestors `entity` has: 0 for one placed on the screen.
    pub fn depth(&self, entity: Entity) -> u32 {
        self.resolved.get(&entity).map_or(0, |(_, depth)| *depth)
    }

    /// The order screen-space elements draw and pick in: `sort_order`, then
    /// depth — so a child draws over its panel without a sort order of its
    /// own — then the entity's index.
    pub fn draw_key(&self, entity: Entity, sort_order: i32) -> (i32, u32, u32) {
        (sort_order, self.depth(entity), entity.index())
    }

    pub fn len(&self) -> usize {
        self.resolved.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resolved.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SceneId, Transform};

    fn ui(world: &mut World, rect: UiRect) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, rect);
        entity
    }

    const SCREEN: Vec2 = Vec2::new(800.0, 600.0);

    #[test]
    fn a_root_rect_resolves_against_the_screen() {
        let mut world = World::new();
        let e = ui(&mut world, UiRect::stretched(0.0));
        let layout = UiLayout::from_world(&world, SCREEN);
        assert_eq!(layout.rect(e), Some(Rect::screen(SCREEN)));
        assert_eq!(layout.depth(e), 0);
    }

    #[test]
    fn a_child_rect_resolves_against_its_parent() {
        let mut world = World::new();
        let panel = ui(
            &mut world,
            UiRect::pinned(Vec2::ZERO, Vec2::splat(100.0), Vec2::splat(200.0)),
        );
        let label = ui(&mut world, UiRect::stretched(10.0));
        hierarchy::set_parent(&mut world, label, panel).expect("parents");

        let layout = UiLayout::from_world(&world, SCREEN);

        assert_eq!(
            layout.rect(label),
            Some(Rect::new(Vec2::splat(110.0), Vec2::splat(290.0)))
        );
        assert_eq!(layout.depth(label), 1);
    }

    #[test]
    fn an_ancestor_without_a_rect_is_passed_over() {
        let mut world = World::new();
        let panel = ui(
            &mut world,
            UiRect::pinned(Vec2::ZERO, Vec2::splat(100.0), Vec2::splat(200.0)),
        );
        let group = world.spawn();
        world.insert(group, SceneId::new());
        world.insert(group, Transform::default());
        let label = ui(&mut world, UiRect::stretched(0.0));
        hierarchy::set_parent(&mut world, group, panel).expect("parents");
        hierarchy::set_parent(&mut world, label, group).expect("parents");

        let layout = UiLayout::from_world(&world, SCREEN);
        assert_eq!(layout.rect(label), layout.rect(panel));
    }

    #[test]
    fn a_resize_moves_what_is_anchored_to_the_far_edge() {
        let mut world = World::new();
        let corner = ui(
            &mut world,
            UiRect::pinned(Vec2::ONE, Vec2::ZERO, Vec2::splat(10.0)),
        );
        let big = UiLayout::from_world(&world, SCREEN);
        let small = UiLayout::from_world(&world, Vec2::new(400.0, 300.0));
        assert_eq!(big.rect(corner).map(|r| r.max), Some(SCREEN));
        assert_eq!(
            small.rect(corner).map(|r| r.max),
            Some(Vec2::new(400.0, 300.0))
        );
    }

    #[test]
    fn a_child_sorts_over_its_parent_at_the_same_order() {
        let mut world = World::new();
        let label = ui(&mut world, UiRect::stretched(0.0));
        let panel = ui(&mut world, UiRect::stretched(0.0));
        hierarchy::set_parent(&mut world, label, panel).expect("parents");

        let layout = UiLayout::from_world(&world, SCREEN);
        assert!(layout.draw_key(label, 0) > layout.draw_key(panel, 0));
    }

    #[test]
    fn a_chain_deeper_than_any_stack_resolves_without_overflowing() {
        // A crafted scene file can link entities into a chain no editor
        // would build; `set_parent` would refuse it, so `Parent` is written
        // directly, the way a loaded file would.
        let mut world = World::new();
        let mut previous = ui(&mut world, UiRect::stretched(0.0));
        let mut last = previous;
        for _ in 0..20_000 {
            last = ui(&mut world, UiRect::stretched(0.0));
            let id = *world.get::<SceneId>(previous).expect("an id");
            world.insert(last, hierarchy::Parent::resolved(id, previous));
            previous = last;
        }

        let layout = UiLayout::from_world(&world, SCREEN);
        assert_eq!(layout.rect(last), Some(Rect::screen(SCREEN)));
    }

    #[test]
    fn a_parent_cycle_resolves_rather_than_looping() {
        let mut world = World::new();
        let a = ui(&mut world, UiRect::stretched(1.0));
        let b = ui(&mut world, UiRect::stretched(1.0));
        let (id_a, id_b) = (
            *world.get::<SceneId>(a).expect("an id"),
            *world.get::<SceneId>(b).expect("an id"),
        );
        world.insert(a, hierarchy::Parent::resolved(id_b, b));
        world.insert(b, hierarchy::Parent::resolved(id_a, a));

        let layout = UiLayout::from_world(&world, SCREEN);
        assert!(layout.rect(a).is_some() && layout.rect(b).is_some());
    }

    #[test]
    fn an_entity_without_a_rect_is_not_in_the_layout() {
        let mut world = World::new();
        let plain = world.spawn();
        let layout = UiLayout::from_world(&world, SCREEN);
        assert_eq!(layout.rect(plain), None);
        assert!(layout.is_empty());
    }
}
