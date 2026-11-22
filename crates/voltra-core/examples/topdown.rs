//! A game seen from above: walls, a player, and a chaser that finds its way
//! to the player round them.
//!
//! The chaser asks a navigation grid — baked once from the walls, grown by
//! its own size — for a path, walks it waypoint by waypoint with
//! `move_and_slide`, and asks again every half second as the player moves.
//! Behind a wall it cannot get round, it goes as near as it can.
//!
//! Run it with `cargo run -p voltra-core --example topdown`. `WASD` or the
//! arrow keys move.

use voltra_core::nav::{NavGrid, NavSettings, Path};
use voltra_core::{move_and_slide, App, WindowConfig};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Camera, CameraFollow2D, CharacterBody2D, Collider, SceneId, Sprite, Transform};

const PLAYER_SPEED: f32 = 4.0;
const CHASER_SPEED: f32 = 2.6;
/// Half the chaser's size: what the grid is grown by.
const CHASER_HALF: f32 = 0.3;
/// How often the chaser looks for a new way, in seconds.
const REPATH: f32 = 0.5;

fn main() {
    voltra_core::logging::init(false);
    let mut app = App::new(WindowConfig {
        title: "Voltra — top down".into(),
        ..Default::default()
    });
    spawn_maze(&mut app);
    let player = mover(&mut app, Vec2::new(-6.0, -4.0), 0.25, [0.4, 0.8, 1.0, 1.0]);
    let chaser = mover(
        &mut app,
        Vec2::new(6.0, 4.0),
        CHASER_HALF,
        [1.0, 0.4, 0.35, 1.0],
    );
    spawn_camera(&mut app, player);

    // Baked once: the walls do not move. A game whose walls do rebakes.
    let grid = NavGrid::bake_all(
        &app.world,
        &NavSettings {
            cell_size: 0.25,
            clearance: Vec2::splat(CHASER_HALF),
            ..NavSettings::default()
        },
    );
    log::info!(
        "navigation grid: {:?} cells, {} blocked",
        grid.size(),
        grid.blocked_count()
    );

    let mut path = Path::default();
    let mut since = REPATH;
    app.with_simulation()
        .with_fixed_update(move |tick| {
            let input = Vec2::new(
                tick.input.action_axis("move_left", "move_right"),
                tick.input.action_axis("move_down", "move_up"),
            )
            .clamp_length_max(1.0);
            move_and_slide(tick.world, player, input * PLAYER_SPEED, tick.delta);

            let at = |entity: Entity| {
                tick.world
                    .get::<Transform>(entity)
                    .map_or(Vec2::ZERO, |t| t.translation)
            };
            let (here, target) = (at(chaser), at(player));
            since += tick.delta;
            if since >= REPATH {
                since = 0.0;
                path = grid.find_path_closest(here, target).unwrap_or_default();
            }
            let heading = path
                .next_point(here, 0.1)
                .map_or(Vec2::ZERO, |point| (point - here).normalize_or_zero());
            move_and_slide(tick.world, chaser, heading * CHASER_SPEED, tick.delta);
        })
        .run();
}

/// A solid, drawn wall: `half` either way of `at`.
fn wall(app: &mut App, at: Vec2, half: Vec2) {
    let entity = app.world.spawn();
    app.world.insert(
        entity,
        Transform::from_translation(at).with_scale(half * 2.0),
    );
    app.world
        .insert(entity, Sprite::new([0.35, 0.37, 0.45, 1.0]));
    app.world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
}

/// The room and the walls inside it.
fn spawn_maze(app: &mut App) {
    // The outer walls of a 16×10 room.
    wall(app, Vec2::new(0.0, 5.25), Vec2::new(8.5, 0.25));
    wall(app, Vec2::new(0.0, -5.25), Vec2::new(8.5, 0.25));
    wall(app, Vec2::new(-8.25, 0.0), Vec2::new(0.25, 5.0));
    wall(app, Vec2::new(8.25, 0.0), Vec2::new(0.25, 5.0));
    // Inside: a few walls to go round.
    wall(app, Vec2::new(-3.0, 1.5), Vec2::new(0.25, 3.5));
    wall(app, Vec2::new(1.0, -1.5), Vec2::new(0.25, 3.5));
    wall(app, Vec2::new(4.5, 2.0), Vec2::new(2.0, 0.25));
    wall(app, Vec2::new(-5.5, -2.0), Vec2::new(1.5, 0.25));
}

/// A character moved by the game, with no gravity: every surface is a wall.
fn mover(app: &mut App, at: Vec2, half: f32, color: [f32; 4]) -> Entity {
    let entity = app.world.spawn();
    app.world.insert(entity, SceneId::new());
    app.world.insert(
        entity,
        Transform::from_translation(at).with_scale(Vec2::splat(half * 2.0)),
    );
    app.world.insert(entity, Sprite::new(color));
    app.world.insert(
        entity,
        Collider::Box {
            half_extents: Vec2::splat(0.5),
        },
    );
    app.world.insert(
        entity,
        CharacterBody2D {
            // Seen from above nothing is a floor: no snapping, no stepping.
            snap: 0.0,
            step_height: 0.0,
            ..CharacterBody2D::default()
        },
    );
    entity
}

fn spawn_camera(app: &mut App, player: Entity) {
    let Some(&id) = app.world.get::<SceneId>(player) else {
        return;
    };
    let camera = app.world.spawn();
    app.world.insert(camera, Transform::default());
    app.world.insert(camera, Camera::new(6.0));
    app.world.insert(
        camera,
        CameraFollow2D {
            dead_zone: Vec2::new(1.5, 1.0),
            ..CameraFollow2D::new(id)
        },
    );
}
