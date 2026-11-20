//! Every [`UiRect`] resolved to a box on a screen of a given size.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::rect::{Rect, UiRect};
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

    fn resolve(&mut self, world: &World, entity: Entity, screen: Rect) -> (Rect, u32) {
        if let Some(&known) = self.resolved.get(&entity) {
            return known;
        }
        let Some(rect) = world.get::<UiRect>(entity) else {
            return (screen, 0);
        };
        // Marked before recursing: `set_parent` refuses cycles, but a
        // hand-edited file can still hold one, and a box resolved against the
        // screen is a better answer than a stack overflow.
        self.resolved.insert(entity, (screen, 0));
        let parent = hierarchy::ancestors(world, entity)
            .into_iter()
            .find(|&ancestor| world.get::<UiRect>(ancestor).is_some());
        let (parent_rect, depth) = match parent {
            Some(parent) => {
                let (rect, depth) = self.resolve(world, parent, screen);
                (rect, depth + 1)
            }
            None => (screen, 0),
        };
        let resolved = (rect.resolve(parent_rect), depth);
        self.resolved.insert(entity, resolved);
        resolved
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
    fn an_entity_without_a_rect_is_not_in_the_layout() {
        let mut world = World::new();
        let plain = world.spawn();
        let layout = UiLayout::from_world(&world, SCREEN);
        assert_eq!(layout.rect(plain), None);
        assert!(layout.is_empty());
    }
}
