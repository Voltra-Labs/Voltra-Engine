//! The level played headless from the game's own code: the ball breaks
//! bricks on its own, the score counts them, and a ball lost is served
//! again.

use breakout::{Ball, Breakout, Brick, Score, FIRST_SCENE};
use voltra_assets::AssetPath;
use voltra_core::{App, Headless};
use voltra_scene::{Text, Transform2D};

const FRAME: f32 = 1.0 / 60.0;

fn session() -> Headless {
    let scene = AssetPath::new(FIRST_SCENE).expect("a path");
    Headless::open(App::default().with_game(Breakout), &scene).expect("the level loads")
}

fn bricks(session: &Headless) -> usize {
    session.world().query::<Brick>().count()
}

fn score(session: &Headless) -> Score {
    *session.world().query::<Score>().next().expect("a score").1
}

#[test]
fn the_ball_breaks_bricks_and_the_score_counts_them() {
    let mut session = session();
    let at_start = bricks(&session);
    assert_eq!(at_start, 45);
    session.step(60 * 6, FRAME);
    let left = bricks(&session);
    assert!(
        left < at_start,
        "some bricks broke: {left} of {at_start} left"
    );
    let points = score(&session).points;
    assert!(points >= 10 * (at_start - left) as u32, "{points} points");
    let shown = session
        .world()
        .query::<Text>()
        .map(|(_, t)| t.content.clone())
        .find(|t| t.contains("Score") || t.contains("over") || t.contains("Cleared"))
        .expect("the HUD");
    assert!(shown.contains(&points.to_string()), "{shown}");
}

#[test]
fn the_ball_keeps_its_speed() {
    let mut session = session();
    session.step(120, FRAME);
    let (ball, settings) = session
        .world()
        .query::<Ball>()
        .next()
        .map(|(e, b)| (e, *b))
        .expect("a ball");
    let speed = session
        .world()
        .get::<voltra_scene::RigidBody2D>(ball)
        .expect("a body")
        .velocity
        .length();
    assert!((speed - settings.speed).abs() < 0.5, "{speed}");
}

#[test]
fn a_lost_ball_is_served_again_and_costs_one() {
    let mut session = session();
    // Thrown below the floor.
    let ball = session
        .world()
        .query::<Ball>()
        .next()
        .map(|(e, _)| e)
        .expect("a ball");
    session
        .world_mut()
        .get_mut::<Transform2D>(ball)
        .expect("placed")
        .translation
        .y = -10.0;
    session.step(2, FRAME);
    assert_eq!(score(&session).balls, 2);
    let y = session
        .world()
        .get::<Transform2D>(ball)
        .expect("placed")
        .translation
        .y;
    assert!(y > -4.0, "served above the paddle: {y}");
}
