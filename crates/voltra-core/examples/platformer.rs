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
//! takes it — with a sound, which is the point of the ears on the camera.

use std::cell::Cell;
use std::rc::Rc;

use voltra_assets::AssetPath;
use voltra_core::{
    query, App, Binding, KeyCode, PlayParams, QueryFilter, Tick, Touch, WindowConfig,
};
use voltra_ecs::Entity;
use voltra_render::glam::{IVec2, Vec2};
use voltra_scene::{
    Align, AudioListener, AudioSource, Camera, Collider, ComponentRegistry, PhysicsMaterial,
    RigidBody, Sprite, Text, Tile, Tilemap, Transform, UiRect,
};

/// This example's own component, in the same world as the engine's.
///
/// An in-house ECS stores whatever type a game hands it, so gameplay tags
/// need no engine change and no enum with a variant per game. It is
/// registered under a name only because the coin is a prefab: a file can name
/// a component only the game knows, and the registry is how the game says
/// which type the name means.
#[derive(serde::Serialize, serde::Deserialize)]
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

    let mut registry = ComponentRegistry::with_defaults();
    registry.register::<Coin>("Coin");
    let mut app = App::new(WindowConfig {
        title: "Voltra — platformer".into(),
        ..Default::default()
    })
    .with_registry(registry);
    let walker = spawn_scene(&mut app);
    let camera = spawn_camera(&mut app);
    let counter = spawn_hud(&mut app);
    let mut coins = 0;

    // The one piece of state the two hooks share: a jump asked for this frame,
    // to be spent by the next step. Without it the fixed tick would have to
    // read the key edge itself and would miss it on a frame that owes no step.
    let jump = Rc::new(Cell::new(false));
    let requested = jump.clone();
    // F1, then any key: that key becomes jump. The whole of a rebinding
    // screen, minus the screen.
    let mut rebinding = false;

    app.with_simulation()
        .with_update(move |tick| {
            if rebinding {
                if let Some(binding) = tick.input.pressed_binding() {
                    tick.actions.rebind("jump", 0, binding);
                    log::info!("jump is now {binding:?}");
                    rebinding = false;
                }
            } else if tick.input.was_binding_pressed(Binding::Key(KeyCode::F1)) {
                log::info!("press a key for jump");
                rebinding = true;
            } else if tick.input.action("jump").pressed() {
                requested.set(true);
            }
            follow(tick, camera, walker);
        })
        .with_fixed_update(move |tick| {
            let taken = take_coins(tick, walker);
            if taken > 0 {
                coins += taken;
                show_coins(tick, counter, coins);
            }

            // Actions, from `assets/default.input.ron`: rebinding the keys
            // is an edit to that file, picked up while the game runs.
            let direction = tick.input.action_axis("move_left", "move_right");
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
/// The coins the level places, and so the most the counter can read.
const COINS: [f32; 3] = [2.0, 6.0, 8.0];

/// A HUD in the top-left corner: a dark panel and the coin counter on it.
///
/// Screen space, not world: it stays put while the camera follows the
/// walker. The counter is the panel's child, so moving the panel moves it,
/// and it draws over the panel without a sort order of its own.
fn spawn_hud(app: &mut App) -> Entity {
    let panel = app.world.spawn();
    app.world.insert(panel, voltra_scene::SceneId::new());
    app.world.insert(
        panel,
        UiRect::pinned(Vec2::ZERO, Vec2::splat(16.0), Vec2::new(180.0, 44.0)),
    );
    app.world
        .insert(panel, Sprite::new([0.08, 0.09, 0.12, 1.0]));

    let counter = app.world.spawn();
    app.world.insert(counter, voltra_scene::SceneId::new());
    app.world.insert(counter, UiRect::stretched(10.0));
    app.world.insert(
        counter,
        Text {
            size: 22.0,
            color: [1.0, 0.85, 0.35, 1.0],
            ..Text::new(coins_label(0))
        },
    );
    if let Err(e) = voltra_scene::hierarchy::set_parent(&mut app.world, counter, panel) {
        log::error!("could not put the counter on its panel: {e}");
    }
    counter
}

fn coins_label(coins: usize) -> String {
    format!("Coins {coins} / {}", COINS.len())
}

fn show_coins(tick: &mut Tick<'_>, counter: Entity, coins: usize) {
    if let Some(text) = tick.world.get_mut::<Text>(counter) {
        text.content = coins_label(coins);
    }
}

/// Takes every coin the walker touched this step, and says how many.
fn take_coins(tick: &mut Tick<'_>, walker: Entity) -> usize {
    let taken: Vec<Entity> = tick
        .events
        .iter()
        .filter(|event| event.touch == Touch::Began)
        .filter_map(|event| event.other(walker))
        .filter(|&other| tick.world.get::<Coin>(other).is_some())
        .collect();
    let taken_count = taken.len();

    for coin in taken {
        // Read before the despawn, obviously, but also *played* before it: the
        // sound outlives the entity that named it, because a one-shot from the
        // tick is not tied to a source the loop is following. A coin that had
        // to stay alive to be heard would be the wrong shape for every pickup
        // in every game.
        if let Some(source) = tick.world.get::<AudioSource>(coin) {
            if let Some(clip) = source.clip_handle {
                // On the source's own bus, so a game's effects slider reaches
                // the pickup the same way it reaches everything else.
                let bus = tick.audio.bus(&source.bus);
                tick.audio.play_with(
                    clip,
                    PlayParams {
                        bus,
                        ..PlayParams::default()
                    },
                );
            }
        }

        // Despawned mid-overlap on purpose: the pair ends by disappearing, and
        // the `Ended` event that follows is what a game would close a door on.
        tick.world.despawn(coin);
        log::info!("coin taken");
    }
    taken_count
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

    // Three copies of one file. The coin's sprite, spin, sound, sensor and
    // tag are authored once in `prefabs/coin.ron` rather than here, which is
    // the difference between a level and a function that builds one.
    for x in COINS {
        let at = Transform::from_translation(Vec2::new(x, -1.6));
        if let Err(e) = app.spawn(&asset("prefabs/coin.ron"), &at) {
            log::error!("could not place a coin: {e}");
        }
    }

    // A sign over the ledge, in the world rather than on the screen: it
    // scrolls with the level and sorts behind the walker.
    let sign = app.world.spawn();
    app.world
        .insert(sign, Transform::from_translation(Vec2::new(6.0, 0.4)));
    app.world.insert(
        sign,
        Text {
            size: 0.45,
            color: [1.0, 0.92, 0.6, 1.0],
            align: Align::Center,
            sort_order: 5,
            ..Text::new(
                "Jump!
Space",
            )
        },
    );

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
    // The ears ride with the eyes, which is where Unity puts them by default:
    // a listener somewhere else would pan sounds to the side the camera is not
    // looking from.
    app.world.insert(camera, AudioListener::default());
    camera
}

/// Side of one ground tile, in world units. Every ground block's size is a
/// whole number of these.
const TILE: f32 = 0.5;

/// The tileset's frames, as `sprites/ground.atlas.ron` lays them out.
const GRASS: u32 = 0;
const DIRT: u32 = 1;

/// A static block of ground, centred on `at`: one solid tile layer.
///
/// The boxes the bodies stand on are generated from the tiles — one box for
/// the whole block, since it has no gaps — so what is drawn and what is solid
/// cannot drift apart. Grass along the top row and dirt below.
fn ground(app: &mut App, at: Vec2, size: Vec2) {
    let columns = (size.x / TILE).round() as i32;
    let rows = (size.y / TILE).round() as i32;
    let mut layer = Tilemap::new(Vec2::splat(TILE));
    layer.texture = Some(asset("sprites/ground.png"));
    layer.atlas = Some(asset("sprites/ground.atlas.ron"));
    layer.sort_order = -1;
    layer.collision = true;
    for x in 0..columns {
        for y in 0..rows {
            let frame = if y == rows - 1 { GRASS } else { DIRT };
            layer.set(IVec2::new(x, y), Some(Tile::new(frame)));
        }
    }
    let tiles = app.world.spawn();
    app.world
        .insert(tiles, Transform::from_translation(at - size / 2.0));
    app.world.insert(tiles, layer);
}

/// A path under the asset root, for the sheets this example ships with.
fn asset(path: &str) -> AssetPath {
    AssetPath::new(path).expect("the paths in this file are literals under the asset root")
}
