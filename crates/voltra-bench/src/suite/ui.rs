//! UI layout: a tree of rects and stacks resolved against the screen.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::set_parent;
use voltra_scene::ui::stack::Direction;
use voltra_scene::{SceneId, UiLayout, UiRect, UiStack};

use super::Bench;
use crate::harness::{iter, measure};

const SCREEN: Vec2 = Vec2::new(1280.0, 720.0);

/// Rows of the menu, and cells in each row.
const ROWS: usize = 50;
const CELLS: usize = 19;

pub(super) fn benches() -> Vec<Bench> {
    vec![Bench::new("ui/layout/1k-elements", 15.0, |config| {
        let world = menu();
        measure(config, 1, iter(|| UiLayout::from_world(&world, SCREEN))).into()
    })]
}

fn element(world: &mut World, rect: UiRect, parent: Option<Entity>) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, rect);
    if let Some(parent) = parent {
        set_parent(world, entity, parent).expect("a fresh child of a fresh parent links");
    }
    entity
}

/// A screen-filling column of `ROWS` rows, each a row stack of `CELLS`
/// cells: 1 001 elements, every one placed by a stack — an inventory, a
/// level select, a settings page.
fn menu() -> World {
    let mut world = World::new();
    let root = element(&mut world, UiRect::stretched(0.0), None);
    world.insert(root, UiStack::default());
    for _ in 0..ROWS {
        let row = element(
            &mut world,
            UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(0.0, 12.0)),
            Some(root),
        );
        world.insert(
            row,
            UiStack {
                direction: Direction::Row,
                spacing: 2.0,
                ..UiStack::default()
            },
        );
        for _ in 0..CELLS {
            element(
                &mut world,
                UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(40.0, 0.0)),
                Some(row),
            );
        }
    }
    world
}
