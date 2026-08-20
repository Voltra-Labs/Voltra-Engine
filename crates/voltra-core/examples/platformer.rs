//! A game written against the loop: input moves a body, the camera follows.
//!
//! What the two hooks are for, in the smallest scene that shows it. The edge —
//! the frame the jump key went down — is read in the per-frame tick, because
//! that is the only place an edge is seen exactly once. The forces are applied
//! in the fixed tick, because a velocity written per frame is scaled by the
//! frame rate. Unity and Godot split `Update`/`FixedUpdate` and
//! `_process`/`_physics_process` over the same two facts.
//!
//! Run it with `cargo run -p voltra-core --example platformer`.
//! `A`/`D` or the arrow keys walk, `Space` jumps, and walking into a coin
//! takes it.

use std::cell::Cell;
use std::rc::Rc;

use voltra_assets::AssetPath;
use voltra_core::{query, App, KeyCode, QueryFilter, Tick, Touch, WindowConfig};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{
    Camera, Collider, PhysicsMaterial, RigidBody, Sensor, Sprite, SpriteAnimation, Transform,
};

/// This example's own component, in the same world as the engine's.
///
/// Nothing registers it and nothing else knows it exists: an in-house ECS
/// stores whatever type a game hands it, so gameplay tags need no engine
/// change and no enum with a variant per game.
struct Coin;

/// World units per second the walker travels at full tilt.
const WALK_SPEED: f32 = 4.0;
/// Upward speed a jump starts with, in world units per second.
const JUMP_SPEED: f32 = 7.0;
/// How much of the distance to the walker the camera closes per second.
///
/// A fraction rather than a speed: the camera eases in without overshooting,
/// and the frame rate cannot change where it ends up.
const CAMERA_FOLLOW: f32 = 6.0;
/// Steepest surface still counted as ground, as the cosine of its angle.
///
/// `0.5` is 60°, Unity's default slope limit. Below it a wall would count as a
/// floor and the walker could climb it by holding the jump key.
const GROUND_NORMAL: f32 = 0.5;
/// Half the walker's height, in world units: its scale times its half extent.
const WALKER_HALF_HEIGHT: f32 = 0.6;
/// Half its width, likewise.
const WALKER_HALF_WIDTH: f32 = 0.35;
/// How far past its feet the ground check looks.
///
/// Long enough to survive the fraction of a unit the solver leaves between two
/// resting bodies, short enough that a jump stops counting as grounded on the
/// frame it leaves the floor.
const GROUND_PROBE: f32 = 0.08;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut app = App::new(WindowConfig {
        title: "Voltra — platformer".into(),
        ..Default::default()
    });
    let walker = spawn_scene(&mut app);
    let camera = spawn_camera(&mut app);

    // The one piece of state the two hooks share: a jump asked for this frame,
    // to be spent by the next step. Without it the fixed tick would have to
    // read the key edge itself and would miss it on a frame that owes no step.
    let jump = Rc::new(Cell::new(false));
    let requested = jump.clone();

    app.with_simulation()
        .with_update(move |tick| {
            if tick.input.was_key_pressed(KeyCode::Space) {
                requested.set(true);
            }
            follow(tick, camera, walker);
        })
        .with_fixed_update(move |tick| {
            take_coins(tick, walker);

            let direction = tick.input.axis(KeyCode::KeyA, KeyCode::KeyD)
                + tick.input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight);
            let grounded = is_grounded(tick, walker);
            // Taken, not read: a jump is spent once, whether it was asked for
            // one step ago or three.
            let jumped = jump.replace(false) && grounded;

            let Some(body) = tick.world.get_mut::<RigidBody>(walker) else {
                return;
            };
            // Horizontal velocity is set rather than accelerated: this is a
            // character, not a crate, and a walker that slides to a halt is a
            // physics body pretending to be one.
            body.velocity.x = direction.clamp(-1.0, 1.0) * WALK_SPEED;
            if jumped {
                body.velocity.y = JUMP_SPEED;
            }
        })
        .run();
}

/// Takes any coin the walker began touching, in the step it touched it.
///
/// In the fixed tick because that is where an event is handed over exactly
/// once. Read per frame, the same `Began` would arrive on every frame that
/// owed no step, and a coin would be worth three of itself.
fn take_coins(tick: &mut Tick<'_>, walker: Entity) {
    let taken: Vec<Entity> = tick
        .events
        .iter()
        .filter(|event| event.touch == Touch::Began)
        .filter_map(|event| event.other(walker))
        .filter(|&other| tick.world.get::<Coin>(other).is_some())
        .collect();

    for coin in taken {
        // Despawned mid-overlap on purpose: the pair ends by disappearing, and
        // the `Ended` event that follows is what a game would close a door on.
        tick.world.despawn(coin);
        log::info!("coin taken");
    }
}

/// Whether `entity` has ground under its feet.
///
/// A pair of short rays rather than a scan of the contacts: they answer before
/// the first contact exists, they cost two queries instead of the whole
/// manifold list, and one ray per foot is what keeps a walker whose middle
/// overhangs the edge of the ledge standing on it. `QueryFilter` skips sensors
/// by default, so standing in a coin is not standing on one.
fn is_grounded(tick: &Tick<'_>, entity: Entity) -> bool {
    let Some(&Transform { translation, .. }) = tick.world.get::<Transform>(entity) else {
        return false;
    };
    let filter = QueryFilter::new().excluding(entity);
    let reach = WALKER_HALF_HEIGHT + GROUND_PROBE;

    [-WALKER_HALF_WIDTH, WALKER_HALF_WIDTH]
        .into_iter()
        .any(|x| {
            let foot = translation + Vec2::new(x, 0.0);
            query::ray(tick.world, foot, Vec2::NEG_Y, reach, filter)
                .is_some_and(|hit| hit.normal.y > GROUND_NORMAL)
        })
}

/// Eases the camera towards the walker, horizontally.
///
/// Per frame rather than per step, because this is what is looked at rather
/// than what is simulated — and because a camera that moved on the physics
/// clock would judder on any frame that owed two steps.
fn follow(tick: &mut Tick<'_>, camera: Entity, target: Entity) {
    let Some(&Transform { translation, .. }) = tick.world.get::<Transform>(target) else {
        return;
    };
    let Some(transform) = tick.world.get_mut::<Transform>(camera) else {
        return;
    };
    // Frame-rate independent easing: the fraction closed per second is the
    // constant, not the fraction closed per frame.
    let t = 1.0 - (-CAMERA_FOLLOW * tick.delta).exp();
    transform.translation.x += (translation.x - transform.translation.x) * t;
}

/// A floor, two crates to shove around, and the walker. Returns the walker.
fn spawn_scene(app: &mut App) -> Entity {
    ground(app, Vec2::new(0.0, -3.0), Vec2::new(24.0, 1.0));
    // A ledge to jump onto, so the jump has somewhere to go.
    ground(app, Vec2::new(6.0, -1.4), Vec2::new(4.0, 1.0));

    for (i, x) in [-3.0, -2.2].into_iter().enumerate() {
        let crate_entity = app.world.spawn();
        app.world.insert(
            crate_entity,
            Transform::from_translation(Vec2::new(x, 0.0)).with_scale(Vec2::splat(0.6)),
        );
        app.world.insert(
            crate_entity,
            Sprite::new([0.85, 0.55, 0.3, 1.0]).with_sort_order(i as i32),
        );
        app.world.insert(crate_entity, RigidBody::new_dynamic(0.5));
        app.world.insert(
            crate_entity,
            Collider::Box {
                half_extents: Vec2::splat(0.5),
            },
        );
    }

    for x in [2.0, 6.0, 8.0] {
        coin(app, Vec2::new(x, -1.6));
    }

    let walker = app.world.spawn();
    app.world.insert(
        walker,
        Transform::from_translation(Vec2::new(0.0, 0.0)).with_scale(Vec2::new(0.7, 1.2)),
    );
    app.world.insert(
        walker,
        Sprite::new([0.35, 0.75, 1.0, 1.0]).with_sort_order(10),
    );
    app.world.insert(
        walker,
        RigidBody {
            // A character that tips over is a ragdoll, not a walker.
            lock_rotation: true,
            ..RigidBody::new_dynamic(1.0)
        },
    );
    app.world.insert(
        walker,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
    // Slippery on purpose: friction against a wall would let the walker hang
    // on it, and the velocity is set directly anyway.
    app.world.insert(
        walker,
        PhysicsMaterial {
            friction: 0.0,
            restitution: 0.0,
        },
    );

    walker
}

/// The camera the frame is drawn through, since there is no editor here.
fn spawn_camera(app: &mut App) -> Entity {
    let camera = app.world.spawn();
    app.world
        .insert(camera, Transform::from_translation(Vec2::new(0.0, -0.5)));
    app.world.insert(camera, Camera::new(4.0));
    camera
}

/// A coin: a shape that reports being walked into and stops nothing.
fn coin(app: &mut App, at: Vec2) {
    let entity = app.world.spawn();
    app.world.insert(
        entity,
        Transform::from_translation(at).with_scale(Vec2::splat(0.4)),
    );
    // White, so the sheet's own colours come through untinted. The handles are
    // left unresolved on purpose: the world is populated before there is a
    // device, and `App` resolves every sprite's texture and atlas when the
    // loop resumes.
    let mut sprite = Sprite::new([1.0, 1.0, 1.0, 1.0]).with_sort_order(5);
    sprite.texture = Some(asset("sprites/coin.png"));
    sprite.atlas = Some(asset("sprites/coin.atlas.ron"));
    app.world.insert(entity, sprite);
    // Eight frames a second over a four-frame sheet: one spin every half
    // second, which reads as turning rather than flickering.
    app.world
        .insert(entity, SpriteAnimation::new(vec![0, 1, 2, 3], 8.0));
    app.world.insert(entity, Collider::Circle { radius: 0.5 });
    // No `RigidBody`: it is static geometry that happens to be a sensor. A
    // body would fall, and a sensor cannot rest on anything.
    app.world.insert(entity, Sensor);
    app.world.insert(entity, Coin);
}

fn ground(app: &mut App, at: Vec2, size: Vec2) {
    let entity = app.world.spawn();
    app.world
        .insert(entity, Transform::from_translation(at).with_scale(size));
    app.world
        .insert(entity, Sprite::new([0.25, 0.27, 0.32, 1.0]));
    app.world.insert(entity, RigidBody::new_static());
    app.world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
}

/// A path under the asset root, for the sheets this example ships with.
fn asset(path: &str) -> AssetPath {
    AssetPath::new(path).expect("the paths in this file are literals under the asset root")
}
