//! Multi-editing through a world, no GPU: what is mixed, what is written,
//! and the undo entry it makes.

mod mixed;
mod sparse;
mod undo;
mod variants;
mod write;

use voltra_ecs::{Entity, World};
use voltra_scene::format::serial_tree::Step;
use voltra_scene::format::ComponentRegistry;
use voltra_scene::{SceneId, Sprite, Transform};

use super::MultiEdit;

pub(super) fn sprite(world: &mut World, color: [f32; 4]) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform::default());
    world.insert(entity, Sprite::new(color));
    entity
}

pub(super) const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
pub(super) const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
pub(super) const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Three sprites, the last the primary.
pub(super) fn three(colors: [[f32; 4]; 3]) -> (World, Vec<Entity>) {
    let mut world = World::new();
    let entities = colors.iter().map(|&c| sprite(&mut world, c)).collect();
    (world, entities)
}

pub(super) fn begin(world: &World, registry: &ComponentRegistry, selected: &[Entity]) -> MultiEdit {
    let primary = *selected.last().expect("something selected");
    MultiEdit::begin(world, registry, primary, selected.iter().copied())
}

pub(super) fn color() -> Vec<Step> {
    vec![Step::Field("color")]
}
