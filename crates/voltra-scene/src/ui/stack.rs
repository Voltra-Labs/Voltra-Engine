//! Rows and columns: a parent that places its children one after another.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::edges::Edges;
use super::rect::{Rect, UiRect};
use crate::hierarchy;

/// Which way a [`UiStack`] runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Direction {
    /// Left to right.
    Row,
    /// Top to bottom.
    #[default]
    Column,
}

/// Where the children sit along the stack, or across it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Align {
    #[default]
    /// At the start of the axis.
    Start,
    /// Centred on the axis.
    Center,
    /// At the end of the axis.
    End,
    /// Across the stack only: as wide as it is. Along it, the same as
    /// `Start` — a child's length is its own.
    Stretch,
}

/// Places the children of this entity's [`UiRect`] in a row or a column:
/// one after another, `spacing` points apart, inside `padding`.
///
/// Godot's `HBoxContainer` / `VBoxContainer`, Unity's horizontal and
/// vertical layout groups. A child keeps the size its own `UiRect`
/// resolves to inside the padded box — a fixed size, or a stretch across
/// it — and the stack decides where it goes; its anchors' position is
/// overridden. Children go in [`UiRect::slot`] order, then entity index:
/// the hierarchy keeps no sibling order, and a scene file's order is its
/// ids', so the order a menu reads in has to be said.
///
/// Only direct children with a `UiRect` are stacked; their own children
/// resolve inside them as usual.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiStack {
    #[serde(default)]
    /// Which way the children run.
    pub direction: Direction,
    /// Points between one child and the next.
    #[serde(default)]
    pub spacing: f32,
    /// Points kept clear inside each side of the stack.
    #[serde(default)]
    pub padding: Edges,
    /// The run of children along the stack, when they do not fill it.
    #[serde(default)]
    pub justify: Align,
    /// Each child across the stack.
    #[serde(default = "stretch")]
    pub align: Align,
}

fn stretch() -> Align {
    Align::Stretch
}

impl Default for UiStack {
    fn default() -> Self {
        Self {
            direction: Direction::Column,
            spacing: 8.0,
            padding: Edges::uniform(8.0),
            justify: Align::Start,
            align: Align::Stretch,
        }
    }
}

impl UiStack {
    /// Where each stacked child of `stack` goes, when the stack occupies
    /// `rect`. Children past the far end are still placed — they overflow,
    /// as in every toolkit, rather than vanish.
    pub fn arrange(&self, world: &World, stack: Entity, rect: Rect) -> Vec<(Entity, Rect)> {
        let mut children: Vec<(i32, Entity, UiRect)> = hierarchy::children_of(world, stack)
            .into_iter()
            .filter_map(|child| world.get::<UiRect>(child).map(|r| (r.slot, child, *r)))
            .collect();
        children.sort_by_key(|(slot, child, _)| (*slot, child.index()));

        let content = self.padding.shrink(rect);
        let spacing = if self.spacing.is_finite() {
            self.spacing.max(0.0)
        } else {
            0.0
        };
        // (along, across) in this stack's own axes.
        let axes = |v: Vec2| match self.direction {
            Direction::Row => (v.x, v.y),
            Direction::Column => (v.y, v.x),
        };
        let back = |along: f32, across: f32| match self.direction {
            Direction::Row => Vec2::new(along, across),
            Direction::Column => Vec2::new(across, along),
        };
        let (room_along, room_across) = axes(content.size());
        let (start_along, start_across) = axes(content.min);

        let sizes: Vec<(f32, f32)> = children
            .iter()
            .map(|(_, _, r)| axes(r.resolve(content).size()))
            .collect();
        let gaps = spacing * children.len().saturating_sub(1) as f32;
        let run: f32 = sizes.iter().map(|(along, _)| along).sum::<f32>() + gaps;
        let mut cursor = start_along
            + match self.justify {
                Align::Start | Align::Stretch => 0.0,
                Align::Center => (room_along - run) * 0.5,
                Align::End => room_along - run,
            };

        let mut placed = Vec::with_capacity(children.len());
        for ((_, child, _), (along, across)) in children.iter().zip(sizes) {
            let (across, offset) = match self.align {
                Align::Start => (across, 0.0),
                Align::Center => (across, (room_across - across) * 0.5),
                Align::End => (across, room_across - across),
                Align::Stretch => (room_across, 0.0),
            };
            let min = back(cursor, start_across + offset);
            placed.push((*child, Rect::new(min, min + back(along, across))));
            cursor += along + spacing;
        }
        placed
    }
}

#[cfg(test)]
mod tests;
