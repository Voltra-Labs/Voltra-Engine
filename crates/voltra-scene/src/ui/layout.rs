//! Every [`UiRect`] resolved to a box on a screen of a given size.

use std::collections::HashMap;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::rect::{Rect, UiRect};
use super::scroll::UiScroll;
use super::stack::UiStack;
use crate::hierarchy;

/// Where one element landed.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placed {
    rect: Rect,
    /// How many UI ancestors it has.
    depth: u32,
    /// What of it may show: the overlap of every [`UiScroll`] above it.
    clip: Option<Rect>,
    /// The nearest scroll above it, whose content it is.
    scroll: Option<Entity>,
}

/// The screen box of every entity with a [`UiRect`], how many UI ancestors
/// it has, and the part of it a scroll above it lets show.
///
/// Computed once per frame for the whole world, as [`WorldTransforms`] is,
/// so a deep panel tree costs one walk rather than one per element per
/// question. Rebuilt from the screen size each time, which is what makes a
/// resize — a window dragged, an editor panel resized — just the next frame.
///
/// [`WorldTransforms`]: crate::WorldTransforms
#[derive(Debug, Clone, Default)]
pub struct UiLayout {
    resolved: HashMap<Entity, Placed>,
    /// For each scroll, the box its content covers, as laid out.
    content: HashMap<Entity, Rect>,
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

        let mut parent: Option<(Entity, Placed)> = None;
        for link in chain {
            if let Some(&known) = self.resolved.get(&link) {
                parent = Some((link, known));
                continue;
            }
            let Some(rect) = world.get::<UiRect>(link) else {
                continue;
            };
            let placed = match parent {
                Some((above, outer)) => {
                    let inside = Inside::of(world, above, outer);
                    match self.stacked(world, above, &inside, link) {
                        Some(placed) => placed,
                        None => inside.place(rect.resolve(outer.rect)),
                    }
                }
                None => Placed {
                    rect: rect.resolve(screen),
                    depth: 0,
                    clip: None,
                    scroll: None,
                },
            };
            self.record(link, placed);
            parent = Some((link, placed));
        }
    }

    /// Where `child` goes when `parent` stacks it: the whole stack is
    /// arranged at once, every child of it recorded, since where one goes
    /// depends on all the others. `None` when `parent` is no [`UiStack`] or
    /// `child` is not its own child.
    fn stacked(
        &mut self,
        world: &World,
        parent: Entity,
        inside: &Inside,
        child: Entity,
    ) -> Option<Placed> {
        let stack = world.get::<UiStack>(parent)?;
        if hierarchy::parent_of(world, child) != Some(parent) {
            return None;
        }
        let mut found = None;
        for (entity, rect) in stack.arrange(world, parent, inside.parent.rect) {
            let placed = inside.place(rect);
            if entity == child {
                found = Some(placed);
            }
            if !self.resolved.contains_key(&entity) {
                self.record(entity, placed);
            }
        }
        found
    }

    fn record(&mut self, entity: Entity, placed: Placed) {
        if let Some(scroll) = placed.scroll {
            self.content
                .entry(scroll)
                .and_modify(|bounds| *bounds = bounds.union(placed.rect))
                .or_insert(placed.rect);
        }
        self.resolved.insert(entity, placed);
    }

    /// The box `entity` occupies, or `None` if it has no `UiRect`.
    pub fn rect(&self, entity: Entity) -> Option<Rect> {
        self.resolved.get(&entity).map(|placed| placed.rect)
    }

    /// The part of the screen `entity` may show in: `None` when no scroll
    /// above it masks it, an empty box when one masks all of it.
    pub fn clip(&self, entity: Entity) -> Option<Rect> {
        self.resolved.get(&entity).and_then(|placed| placed.clip)
    }

    /// The box of `entity` that shows and takes the pointer: its rect, cut
    /// by its clip.
    pub fn visible(&self, entity: Entity) -> Option<Rect> {
        let placed = self.resolved.get(&entity)?;
        Some(match placed.clip {
            Some(clip) => placed.rect.intersection(clip),
            None => placed.rect,
        })
    }

    /// The nearest [`UiScroll`] whose content `entity` is.
    pub fn scroll_of(&self, entity: Entity) -> Option<Entity> {
        self.resolved.get(&entity).and_then(|placed| placed.scroll)
    }

    /// The box everything scrolled by `scroll` covers, as laid out with its
    /// current offset; `None` with nothing in it.
    pub fn content(&self, scroll: Entity) -> Option<Rect> {
        self.content.get(&scroll).copied()
    }

    /// How many UI ancestors `entity` has: 0 for one placed on the screen.
    pub fn depth(&self, entity: Entity) -> u32 {
        self.resolved.get(&entity).map_or(0, |placed| placed.depth)
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

/// What a parent hands its children: its box, moved by its scroll if it
/// has one, and the clip and scroll they inherit.
struct Inside {
    parent: Placed,
    shift: Vec2,
    clip: Option<Rect>,
    scroll: Option<Entity>,
}

impl Inside {
    fn of(world: &World, parent: Entity, placed: Placed) -> Self {
        match world.get::<UiScroll>(parent) {
            Some(scroll) => Self {
                parent: placed,
                shift: -scroll.shift(),
                clip: Some(match placed.clip {
                    Some(clip) => clip.intersection(placed.rect),
                    None => placed.rect,
                }),
                scroll: Some(parent),
            },
            None => Self {
                parent: placed,
                shift: Vec2::ZERO,
                clip: placed.clip,
                scroll: placed.scroll,
            },
        }
    }

    /// A child laid out at `rect` in the parent's unscrolled box.
    fn place(&self, rect: Rect) -> Placed {
        Placed {
            rect: rect.translated(self.shift),
            depth: self.parent.depth + 1,
            clip: self.clip,
            scroll: self.scroll,
        }
    }
}

#[cfg(test)]
mod tests;
