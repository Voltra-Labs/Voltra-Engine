//! A character moved by `move_and_slide`: walls stop it, slopes are walked
//! up or slid down by their angle, a floor of tiles has no seams, a low step
//! is stepped onto, and a jump ends on a ceiling.
//!
//! Run it with `cargo run -p voltra-core --example character`. `A`/`D` or
//! the arrow keys walk, `Space` jumps.

use std::cell::Cell;
use std::rc::Rc;

use voltra_core::{move_and_slide, App, Tick, WindowConfig};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Camera, CharacterBody2D, Collider, Sprite, Transform};

/// World units per second at full tilt.
const WALK_SPEED: f32 = 3.5;
/// Upward speed a jump starts with.
const JUMP_SPEED: f32 = 6.5;
/// World units per second squared, down.
const GRAVITY: f32 = 18.0;
/// How much of the way to the character the camera closes per second.
const CAMERA_FOLLOW: f32 = 5.0;

fn main() {
    voltra_core::logging::init(false);
    let mut app = App::new(WindowConfig {
        title: "Voltra — character".into(),
        ..Default::default()
    });
    let me = spawn_level(&mut app);
    let camera = app.world.spawn();
    app.world
        .insert(camera, Transform::from_translation(Vec2::new(0.0, 1.0)));
    app.world.insert(camera, Camera::new(4.0));

    // A jump pressed this frame, spent by the next step that can.
    let jump = Rc::new(Cell::new(false));
    let asked = jump.clone();
    // The velocity the character keeps between steps.
    let mut velocity = Vec2::ZERO;

    app.with_simulation()
        .with_update(move |tick| {
            if tick.input.action("jump").pressed() {
                asked.set(true);
            }
            follow(tick, camera, me);
        })
        .with_fixed_update(move |tick| {
            let grounded = tick
                .world
                .get::<CharacterBody2D>(me)
                .is_some_and(CharacterBody2D::is_on_floor);
            velocity.x = tick
                .input
                .action_axis("move_left", "move_right")
                .clamp(-1.0, 1.0)
                * WALK_SPEED;
            velocity.y -= GRAVITY * tick.delta;
            if jump.replace(false) && grounded {
                velocity.y = JUMP_SPEED;
            }
            match move_and_slide(tick.world, me, velocity, tick.delta) {
                Some(slide) => velocity = slide.velocity,
                None => log::error!("the character lost its body or collider"),
            }
        })
        .run();
}

/// A solid, drawn block: `half` either way of `at`, turned by `degrees`.
fn block(app: &mut App, at: Vec2, half: Vec2, degrees: f32, color: [f32; 4]) -> Entity {
    let entity = app.world.spawn();
    app.world.insert(
        entity,
        Transform::from_translation(at)
            .with_rotation(degrees.to_radians())
            .with_scale(half * 2.0),
    );
    app.world.insert(entity, Sprite::new(color));
    // The collider is half a unit either way: the scale makes it the block.
    app.world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
    entity
}

/// A ramp rising from `foot` at `degrees`, `length` long, sunk into the
/// floor at its foot so it meets the floor without a step.
fn ramp(app: &mut App, foot: Vec2, length: f32, degrees: f32, color: [f32; 4]) {
    let along = Vec2::from_angle(degrees.to_radians());
    let half = Vec2::new(length / 2.0 + 0.3, 0.15);
    let centre = foot + along * (length / 2.0 - 0.3) - along.perp() * half.y;
    let ramp = block(app, centre, half, degrees, color);
    // Under the floor, which hides the part sunk into it.
    if let Some(sprite) = app.world.get_mut::<Sprite>(ramp) {
        sprite.sort_order = -1;
    }
}

/// The level, and the character. Answers the character.
fn spawn_level(app: &mut App) -> Entity {
    let (stone, moss, steep) = (
        [0.36, 0.38, 0.45, 1.0],
        [0.35, 0.55, 0.35, 1.0],
        [0.6, 0.35, 0.3, 1.0],
    );
    // A floor of separate tiles, its top at y = 0.
    for i in -12..24 {
        let shade = if i % 2 == 0 {
            stone
        } else {
            [0.32, 0.34, 0.41, 1.0]
        };
        block(
            app,
            Vec2::new(i as f32 * 0.5 + 0.25, -0.25),
            Vec2::splat(0.25),
            0.0,
            shade,
        );
    }
    // A wall at the left end, and a ceiling low enough to bump.
    block(app, Vec2::new(-6.25, 1.5), Vec2::new(0.25, 1.5), 0.0, stone);
    block(app, Vec2::new(-3.0, 1.6), Vec2::new(1.5, 0.15), 0.0, stone);
    // A low step, stepped onto, and a high one, jumped onto.
    block(app, Vec2::new(1.5, 0.02), Vec2::new(0.5, 0.02), 0.0, moss);
    block(app, Vec2::new(3.0, 0.2), Vec2::new(0.5, 0.2), 0.0, moss);
    // A slope gentle enough to walk up, and one too steep, which is slid down.
    ramp(app, Vec2::new(4.5, 0.0), 2.5, 25.0, moss);
    ramp(app, Vec2::new(9.5, 0.0), 2.0, 60.0, steep);

    let me = app.world.spawn();
    app.world.insert(
        me,
        Transform::from_translation(Vec2::new(-1.0, 1.0)).with_scale(Vec2::new(0.4, 0.6)),
    );
    app.world.insert(me, Sprite::new([1.0, 0.8, 0.3, 1.0]));
    app.world.insert(
        me,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
    app.world.insert(me, CharacterBody2D::default());
    me
}

/// Eases the camera towards the character, per frame.
fn follow(tick: &mut Tick<'_>, camera: Entity, target: Entity) {
    let Some(&Transform { translation, .. }) = tick.world.get::<Transform>(target) else {
        return;
    };
    let Some(transform) = tick.world.get_mut::<Transform>(camera) else {
        return;
    };
    let t = 1.0 - (-CAMERA_FOLLOW * tick.delta).exp();
    transform.translation += (translation + Vec2::new(0.0, 0.8) - transform.translation) * t;
}
