//! The arena played headless from the game's own code: rocks fall more and
//! more often, the clock runs while the player is untouched, and the first
//! rock to reach the player ends the run.

use dodge::{Dodge, Rock, Run, FIRST_SCENE};
use voltra_assets::AssetPath;
use voltra_core::{App, Headless};
use voltra_scene::{Name, Transform2D};

const FRAME: f32 = 1.0 / 60.0;

fn session() -> Headless {
    let scene = AssetPath::new(FIRST_SCENE).expect("a path");
    Headless::open(App::default().with_game(Dodge::default()), &scene).expect("the arena loads")
}

fn run(session: &Headless) -> Run {
    *session.world().query::<Run>().next().expect("a run").1
}

fn player(session: &Headless) -> voltra_ecs::Entity {
    session
        .world()
        .query::<Name>()
        .find(|(_, n)| n.as_str() == "Player")
        .map(|(e, _)| e)
        .expect("a player")
}

#[test]
fn rocks_fall_and_the_clock_runs() {
    let mut session = session();
    // Out of the way, high in a corner, where rocks have not yet fallen far.
    let p = player(&session);
    session
        .world_mut()
        .get_mut::<Transform2D>(p)
        .expect("placed")
        .translation = voltra_render::glam::Vec2::new(-30.0, 30.0);
    session.step(60 * 2, FRAME);
    let rocks = session.world().query::<Rock>().count();
    assert!(rocks >= 2, "rocks are falling: {rocks}");
    let r = run(&session);
    assert!(!r.over && r.seconds > 1.9, "{r:?}");
}

#[test]
fn a_rock_on_the_player_ends_the_run_and_fades_it() {
    let mut session = session();
    session.step(1, FRAME);
    // A rock dropped right on the player.
    let p = player(&session);
    let at = session
        .world()
        .get::<Transform2D>(p)
        .expect("placed")
        .translation;
    let rock = session.world_mut().spawn();
    let world = session.world_mut();
    world.insert(rock, Rock);
    world.insert(
        rock,
        Transform2D::from_translation(at + voltra_render::glam::Vec2::new(0.0, 0.6)),
    );
    world.insert(rock, voltra_scene::Collider2D::Circle { radius: 0.5 });
    world.insert(rock, voltra_scene::Sensor);
    world.insert(
        rock,
        voltra_scene::RigidBody2D {
            body_type: voltra_scene::body::BodyType::Kinematic,
            velocity: voltra_render::glam::Vec2::new(0.0, -5.0),
            ..voltra_scene::RigidBody2D::default()
        },
    );
    session.step(30, FRAME);
    let r = run(&session);
    assert!(r.over, "hit: {r:?}");
    let seconds = r.seconds;
    session.step(60, FRAME);
    assert_eq!(run(&session).seconds, seconds, "the clock stopped");
    let alpha = session
        .world()
        .get::<voltra_scene::Sprite2D>(p)
        .expect("a sprite")
        .color[3];
    assert!(alpha < 0.05, "faded out: {alpha}");
}
