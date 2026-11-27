//! The first level's hill and ladder, made of tiles, played headless: the
//! walker walks up the hill's slope, and climbs the ladder by the ledge.

use voltra_assets::AssetPath;
use voltra_core::{App, Headless};
use voltra_ecs::Entity;
use voltra_platformer::ladder::Climbing;
use voltra_platformer::{walker, Platformer, FIRST_LEVEL};
use voltra_render::glam::Vec2;
use voltra_scene::Transform;

const FRAME: f32 = 1.0 / 60.0;

fn level() -> (Headless, Entity) {
    let mut app = App::default().with_game(Platformer::default());
    app.gravity = Vec2::new(0.0, -9.81);
    let session = Headless::open(app, &AssetPath::new(FIRST_LEVEL).expect("a path"))
        .expect("the level loads");
    let walker = walker::find(session.world()).expect("the level has a walker");
    (session, walker)
}

fn at(session: &Headless, walker: Entity) -> Vec2 {
    session
        .world()
        .get::<Transform>(walker)
        .expect("placed")
        .translation
}

fn put(session: &mut Headless, walker: Entity, to: Vec2) {
    walker::place(session.world_mut(), walker, to);
}

#[test]
fn the_walker_walks_up_the_hill() {
    let (mut session, me) = level();
    // On the ground left of the hill, whose slope starts at x = -11.5 and
    // whose top is a unit above the ground.
    put(&mut session, me, Vec2::new(-11.75, -1.8));
    session.step(30, FRAME);
    let ground = at(&session, me).y;

    assert!(session.press("move_right"));
    let mut highest = ground;
    for _ in 0..60 {
        session.step(1, FRAME);
        highest = highest.max(at(&session, me).y);
    }

    assert!(
        highest > ground + 0.8,
        "from {ground} it got to {highest}, short of the hill's top"
    );
}

#[test]
fn the_walker_climbs_the_ladder_and_falls_off_it_when_it_leaves() {
    let (mut session, me) = level();
    // At the foot of the ladder, right of the ledge (x 8..8.5).
    put(&mut session, me, Vec2::new(8.25, -1.85));
    session.step(10, FRAME);
    let foot = at(&session, me).y;

    assert!(session.press("move_up"));
    session.step(30, FRAME);
    let climbed = at(&session, me).y;
    assert!(climbed > foot + 0.8, "from {foot} to {climbed}");
    assert!(
        session.world().get::<Climbing>(me).is_some(),
        "no gravity on a ladder"
    );

    // Taken off the ladder in mid-air, gravity is back and it falls.
    put(&mut session, me, Vec2::new(12.0, 0.5));
    session.step(30, FRAME);
    assert!(at(&session, me).y < 0.0, "it fell: {}", at(&session, me).y);
    assert!(session.world().get::<Climbing>(me).is_none());
}

fn named(session: &Headless, name: &str) -> Entity {
    session
        .world()
        .query::<voltra_scene::Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .expect("the level has it")
}

/// How far the walker shoves the crate called `name`, walking left into it
/// for a second from `from`.
fn shove(name: &str, from: Vec2) -> f32 {
    let (mut session, me) = level();
    let crate_ = named(&session, name);
    put(&mut session, me, from);
    session.step(30, FRAME);
    let start = at(&session, crate_).x;
    assert!(session.press("move_left"));
    session.step(60, FRAME);
    let crate_x = at(&session, crate_).x;
    assert!(
        at(&session, me).x > crate_x,
        "the walker stays its side of the crate"
    );
    start - crate_x
}

#[test]
fn the_walker_pushes_the_crates_and_the_heavy_one_less() {
    // Right of each, on the ground.
    let light = shove("Crate 2", Vec2::new(-1.5, -1.8));
    let heavy = shove("Heavy crate", Vec2::new(-4.0, -1.8));
    assert!(light > 1.0, "pushed along: {light}");
    assert!(heavy > 0.1, "the heavy one moves too: {heavy}");
    assert!(heavy < light, "{heavy} against {light}");
}
