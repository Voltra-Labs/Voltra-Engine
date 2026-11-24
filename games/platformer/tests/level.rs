//! The first level played headless: walked right into a coin, the counter
//! counting it, and a coin worth more counting for more.

use voltra_assets::AssetPath;
use voltra_core::{App, Headless};
use voltra_ecs::World;
use voltra_platformer::components::{Coin, CoinCounter};
use voltra_platformer::{Platformer, FIRST_LEVEL};
use voltra_scene::Text;

const FRAME: f32 = 1.0 / 60.0;

fn level(game: Platformer) -> Headless {
    let app = App::default().with_game(game);
    let mut app = app;
    app.gravity = voltra_render::glam::Vec2::new(0.0, -9.81);
    Headless::open(app, &AssetPath::new(FIRST_LEVEL).expect("a path")).expect("the level loads")
}

pub fn counter(world: &World) -> String {
    let (entity, _) = world
        .query::<CoinCounter>()
        .next()
        .expect("the level has a counter");
    world.get::<Text>(entity).expect("a text").content.clone()
}

#[test]
fn the_level_holds_the_games_components() {
    let session = level(Platformer::default());
    assert_eq!(session.world().query::<Coin>().count(), 3);
    assert_eq!(counter(session.world()), "Coins 0 / 3");
}

#[test]
fn walking_right_takes_the_first_coin_and_counts_it() {
    let mut session = level(Platformer::default());
    session.step(30, FRAME);
    assert!(session.press("move_right"));
    let mut frames = 0;
    while counter(session.world()) == "Coins 0 / 3" && frames < 240 {
        session.step(1, FRAME);
        frames += 1;
    }

    assert_eq!(
        counter(session.world()),
        "Coins 1 / 3",
        "after {frames} frames"
    );
    assert_eq!(session.world().query::<Coin>().count(), 2);
}

#[test]
fn a_coin_worth_more_counts_for_more() {
    let mut session = level(Platformer::default());
    let coins: Vec<_> = session.world().query::<Coin>().map(|(e, _)| e).collect();
    for coin in coins {
        session
            .world_mut()
            .get_mut::<Coin>(coin)
            .expect("a coin")
            .value = 5;
    }
    session.step(30, FRAME);
    session.press("move_right");
    session.step(150, FRAME);

    assert!(
        counter(session.world()).starts_with("Coins 5 / 15"),
        "{}",
        counter(session.world())
    );
}
