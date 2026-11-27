//! A character moved by `move_and_slide`: walls stop it, slopes are walked
//! up — at the speed it walks on the flat — or slid down by their angle, a
//! crate is pushed, a floor of tiles has no seams, a low step is stepped
//! onto, a jump ends on a ceiling, a one-way platform is jumped
//! up through, a lift — an animation clip — carries it, and the camera
//! follows with slack, a lead and the level's bounds, and shakes on a hard
//! landing. A state machine picks the character's motion from what the step
//! sets: on the ground a blend tree mixes idle, walk and run by its speed —
//! it speeds up over a moment, so the mix passes through the walk — and in
//! the air a jump or fall clip. An additive layer squashes it on every
//! landing, over whatever the ground or air state plays.
//!
//! Run it with `cargo run -p voltra-core --example character`. `A`/`D` or
//! the arrow keys walk, `Space` jumps.

use std::cell::Cell;
use std::rc::Rc;

use voltra_assets::AssetPath;
use voltra_core::{move_and_slide, App, WindowConfig};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{
    AnimationPlayer, Animator, Camera2D, CameraFollow2D, CameraShake, CharacterBody2D, Collider2D,
    FollowBounds, OneWay, RigidBody2D, SceneId, Sprite2D, Transform2D,
};

/// World units per second at full tilt.
const WALK_SPEED: f32 = 3.5;
/// World units per second gained (or lost) each second towards the speed
/// the stick asks for.
const ACCELERATION: f32 = 12.0;
/// Upward speed a jump starts with.
const JUMP_SPEED: f32 = 6.5;
/// World units per second squared, down.
const GRAVITY: f32 = 18.0;
/// A landing faster than this, downwards, shakes the camera.
const HARD_LANDING: f32 = 9.0;

fn main() {
    voltra_core::logging::init(false);
    let mut app = App::new(WindowConfig {
        title: "Voltra — character".into(),
        ..Default::default()
    });
    let me = spawn_level(&mut app);
    let camera = spawn_camera(&mut app, me);

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
        })
        .with_fixed_update(move |tick| {
            let grounded = tick
                .world
                .get::<CharacterBody2D>(me)
                .is_some_and(CharacterBody2D::is_on_floor);
            let wanted = tick
                .input
                .action_axis("move_left", "move_right")
                .clamp(-1.0, 1.0)
                * WALK_SPEED;
            let step = ACCELERATION * tick.delta;
            velocity.x += (wanted - velocity.x).clamp(-step, step);
            velocity.y -= GRAVITY * tick.delta;
            if jump.replace(false) && grounded {
                velocity.y = JUMP_SPEED;
                // Off a moving platform with its speed, as a jump off a lift
                // should be.
                if let Some(body) = tick.world.get::<CharacterBody2D>(me) {
                    velocity += body.contact.platform_velocity;
                }
            }
            let falling = velocity.y;
            match move_and_slide(tick.world, me, velocity, tick.delta) {
                Some(slide) => {
                    // A hard landing knocks the camera, by how hard.
                    let landed = slide.contact.on_floor && !grounded;
                    if landed && falling < -HARD_LANDING {
                        if let Some(shake) = tick.world.get_mut::<CameraShake>(camera) {
                            shake.add_trauma((-falling - HARD_LANDING) * 0.1 + 0.3);
                        }
                    }
                    velocity = slide.velocity;
                    // What the state machine reads: it picks the state, and
                    // `speed` weighs the ground's idle, walk and run.
                    if let Some(animator) = tick.world.get_mut::<Animator>(me) {
                        animator.set_float("speed", velocity.x.abs());
                        animator.set_float("rising", velocity.y);
                        animator.set_bool("grounded", slide.contact.on_floor);
                        if landed {
                            // The impact layer's squash, over the rest.
                            animator.trigger("landed");
                        }
                    }
                }
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
        Transform2D::from_translation(at)
            .with_rotation(degrees.to_radians())
            .with_scale(half * 2.0),
    );
    app.world.insert(entity, Sprite2D::new(color));
    // The collider is half a unit either way: the scale makes it the block.
    app.world.insert(
        entity,
        Collider2D::Box {
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
    if let Some(sprite) = app.world.get_mut::<Sprite2D>(ramp) {
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
    for i in -12..40 {
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
    // A crate to push, a dynamic body the step moves.
    let crate_ = block(
        app,
        Vec2::new(-4.5, 0.4),
        Vec2::splat(0.3),
        0.0,
        [0.75, 0.5, 0.3, 1.0],
    );
    app.world.insert(crate_, RigidBody2D::new_dynamic(1.0));
    // A low step, stepped onto, and a high one, jumped onto.
    block(app, Vec2::new(1.5, 0.02), Vec2::new(0.5, 0.02), 0.0, moss);
    block(app, Vec2::new(3.0, 0.2), Vec2::new(0.5, 0.2), 0.0, moss);
    // A slope gentle enough to walk up, and one too steep, which is slid down.
    ramp(app, Vec2::new(4.5, 0.0), 2.5, 25.0, moss);
    ramp(app, Vec2::new(9.5, 0.0), 2.0, 60.0, steep);
    // A platform jumped up through from below, and stood on.
    let ledge = block(
        app,
        Vec2::new(13.0, 1.2),
        Vec2::new(1.0, 0.08),
        0.0,
        [0.5, 0.45, 0.7, 1.0],
    );
    app.world.insert(ledge, OneWay::default());
    // A lift up to a high ledge: moved by an animation clip, and riding it
    // is the character's floor carrying it.
    let lift = block(
        app,
        Vec2::new(15.5, 0.1),
        Vec2::new(0.6, 0.1),
        0.0,
        [0.8, 0.7, 0.4, 1.0],
    );
    match AssetPath::new("animations/lift.anim.ron") {
        Ok(clip) => {
            app.world.insert(lift, AnimationPlayer::new(clip));
        }
        Err(e) => log::error!("the lift's clip path: {e}"),
    }
    block(app, Vec2::new(19.5, 2.4), Vec2::new(1.4, 0.1), 0.0, stone);

    let me = app.world.spawn();
    app.world.insert(
        me,
        Transform2D::from_translation(Vec2::new(-1.0, 1.0)).with_scale(Vec2::new(0.4, 0.6)),
    );
    app.world.insert(me, Sprite2D::new([1.0, 0.8, 0.3, 1.0]));
    app.world.insert(
        me,
        Collider2D::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
    app.world.insert(
        me,
        CharacterBody2D {
            push_force: 15.0,
            constant_speed_on_slopes: true,
            ..CharacterBody2D::default()
        },
    );
    // Idle, walk, run, jump and fall, each a clip that squashes and tints
    // it, and a layer that squashes it on landing.
    match AssetPath::new("animations/hero.animator.ron") {
        Ok(machine) => {
            app.world.insert(me, Animator::new(machine));
        }
        Err(e) => log::error!("the character's state machine path: {e}"),
    }
    me
}

/// The camera: following the character with a little slack and a lead,
/// kept inside the level, and shaken by hard landings.
fn spawn_camera(app: &mut App, target: Entity) -> Entity {
    let id = SceneId::new();
    app.world.insert(target, id);
    let camera = app.world.spawn();
    app.world.insert(camera, Transform2D::default());
    app.world.insert(camera, Camera2D::new(4.0));
    app.world.insert(
        camera,
        CameraFollow2D {
            offset: Vec2::new(0.0, 0.8),
            dead_zone: Vec2::new(0.6, 0.4),
            look_ahead: 0.25,
            bounds: Some(FollowBounds {
                min: Vec2::new(-6.5, -2.0),
                max: Vec2::new(21.0, 8.0),
            }),
            ..CameraFollow2D::new(id)
        },
    );
    app.world.insert(camera, CameraShake::default());
    camera
}
