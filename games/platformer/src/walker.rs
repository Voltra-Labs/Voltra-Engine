//! The walker: found by its [`Walker`] mark, walked by the actions and
//! moved by `move_and_slide` — a `CharacterBody2D`, which stands on slopes,
//! climbs the hill at the speed it walks on the flat and pushes the crates.

use voltra_core::{move_and_slide, Tick};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{CharacterBody2D, CharacterContact, Transform};

use crate::components::Walker;
use crate::ladder::Climbing;

/// World units per second the walker travels at full tilt.
const WALK_SPEED: f32 = 4.0;
/// Upward speed a jump starts with, in world units per second.
const JUMP_SPEED: f32 = 7.0;
/// World units per second squared, down: the walker falls by its own
/// gravity, as a character moved by the game rather than by the step.
const GRAVITY: f32 = 9.81;

/// On the walker: the velocity it keeps between steps, which
/// `move_and_slide` answers and the next step starts from.
///
/// Not registered, so never saved: a save, a reload or a level change puts
/// the walker back standing still.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Motion {
    /// World units per second.
    pub velocity: Vec2,
}

/// The entity the keys move: the first carrying [`Walker`].
pub fn find(world: &World) -> Option<Entity> {
    world.query::<Walker>().map(|(entity, _)| entity).next()
}

/// The walker's velocity, standing still if it has none yet.
pub fn velocity(world: &World, walker: Entity) -> Vec2 {
    world
        .get::<Motion>(walker)
        .map_or(Vec2::ZERO, |motion| motion.velocity)
}

fn set_velocity(world: &mut World, walker: Entity, velocity: Vec2) {
    world.insert(walker, Motion { velocity });
}

/// Sets the walker's velocity from the move actions, and jumps it if
/// `jump` is asked for and it is standing on something. [`step`] moves it.
///
/// Horizontal velocity is set rather than accelerated: a walker that
/// slides to a halt is a crate pretending to be a character.
pub fn steer(tick: &mut Tick<'_>, walker: Entity, jump: bool) {
    // Actions, from `assets/default.input.ron` and the player's own
    // controls laid over it: rebinding the keys for everyone is an
    // edit to that file, picked up while the game runs.
    let direction = tick.input.action_axis("move_left", "move_right");
    let grounded = tick
        .world
        .get::<CharacterBody2D>(walker)
        .is_some_and(CharacterBody2D::is_on_floor);
    let mut velocity = velocity(tick.world, walker);
    velocity.x = direction.clamp(-1.0, 1.0) * WALK_SPEED;
    if jump && grounded {
        velocity.y = JUMP_SPEED;
    }
    set_velocity(tick.world, walker, velocity);
}

/// Moves the walker by its velocity for one step: falling unless it is on
/// a ladder, stopped and slid by what it meets, and shoving the crates it
/// walks into (the level's `CharacterBody2D` sets how hard). Logs once a
/// step if the walker is no character.
pub fn step(tick: &mut Tick<'_>, walker: Entity) {
    let mut velocity = velocity(tick.world, walker);
    if tick.world.get::<Climbing>(walker).is_none() {
        velocity.y -= GRAVITY * tick.delta;
    }
    match move_and_slide(tick.world, walker, velocity, tick.delta) {
        Some(slide) => set_velocity(tick.world, walker, slide.velocity),
        None => log::error!("the walker needs a CharacterBody2D, a Collider and a Transform"),
    }
}

/// Stops the walker walking: under a menu. It still falls, as a paused
/// game's character lands rather than hangs in the air.
pub fn halt(tick: &mut Tick<'_>, walker: Entity) {
    let mut velocity = velocity(tick.world, walker);
    velocity.x = 0.0;
    velocity.y = velocity.y.min(0.0);
    set_velocity(tick.world, walker, velocity);
    step(tick, walker);
}

/// Puts the walker at `at`, standing still: after a load or a level change.
/// What it last touched is forgotten too — the floor it stood on may be
/// gone, and a floor that is not would carry it back.
pub fn place(world: &mut World, walker: Entity, at: Vec2) {
    if let Some(transform) = world.get_mut::<Transform>(walker) {
        transform.translation = at;
    }
    if let Some(body) = world.get_mut::<CharacterBody2D>(walker) {
        body.contact = CharacterContact::default();
    }
    set_velocity(world, walker, Vec2::ZERO);
}
