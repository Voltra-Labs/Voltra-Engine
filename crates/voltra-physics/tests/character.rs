//! A character walked, dropped and jumped around a small level by
//! `move_and_slide`, as a game's fixed tick would.

use voltra_ecs::{Entity, World};
use voltra_physics::move_and_slide;
use voltra_render::glam::Vec2;
use voltra_scene::{CharacterBody2D, Collider, Transform};

const DT: f32 = 1.0 / 60.0;
const GRAVITY: f32 = -9.81;

fn block(world: &mut World, at: Vec2, half: Vec2, rotation: f32) -> Entity {
    let entity = world.spawn();
    world.insert(
        entity,
        Transform::from_translation(at).with_rotation(rotation),
    );
    world.insert(entity, Collider::Box { half_extents: half });
    entity
}

/// A floor whose top is at y = 0.
fn floor(world: &mut World) -> Entity {
    block(world, Vec2::new(0.0, -0.5), Vec2::new(50.0, 0.5), 0.0)
}

fn character(world: &mut World, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(at));
    world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(0.25),
        },
    );
    world.insert(entity, CharacterBody2D::default());
    entity
}

/// Runs `ticks` steps of a game holding `walk` across and falling, and
/// answers the velocity it ends with.
fn run(
    world: &mut World,
    entity: Entity,
    mut velocity: Vec2,
    walk: Option<f32>,
    ticks: u32,
) -> Vec2 {
    for _ in 0..ticks {
        velocity.y += GRAVITY * DT;
        if let Some(walk) = walk {
            velocity.x = walk;
        }
        velocity = move_and_slide(world, entity, velocity, DT)
            .expect("a character")
            .velocity;
    }
    velocity
}

fn at(world: &World, entity: Entity) -> Vec2 {
    world.get::<Transform>(entity).expect("placed").translation
}

fn body(world: &World, entity: Entity) -> &CharacterBody2D {
    world.get::<CharacterBody2D>(entity).expect("a character")
}

// A crate root's modules sit beside it, so the folder is named.
#[path = "character/slide.rs"]
mod slide;
