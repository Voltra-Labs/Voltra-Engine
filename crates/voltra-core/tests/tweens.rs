//! A game's tick fading and moving a sprite, chaining two tweens, firing a
//! timer, and despawning an entity whose tween and timer then stop — run
//! headless at 60 frames a second.

use std::cell::RefCell;
use std::rc::Rc;

use voltra_core::{App, Headless};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::keyframes::{Ease, Property, Value};
use voltra_scene::{Name, SceneId, Sprite2D, TimerId, Transform2D, TweenId};

const FRAME: f32 = 1.0 / 60.0;

/// What the game saw, frame by frame.
#[derive(Default)]
struct Seen {
    started: bool,
    move_right: Option<TweenId>,
    move_up: Option<TweenId>,
    shrink: Option<TweenId>,
    ring: Option<TimerId>,
    orphan: Option<TimerId>,
    beat: Option<TimerId>,
    right_done_at: Option<u32>,
    up_done_at: Option<u32>,
    rang_at: Option<u32>,
    orphan_rang: bool,
    beats: u32,
    frame: u32,
    /// Whether the shrink was still running the frame after its entity went.
    shrink_after_despawn: Option<bool>,
}

fn named(world: &voltra_ecs::World, name: &str) -> Entity {
    world
        .query::<Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("{name} is in the world"))
}

#[test]
fn a_sprite_fades_and_moves_two_tweens_chain_a_timer_fires_and_the_despawned_stop() {
    let seen = Rc::new(RefCell::new(Seen::default()));
    let log = Rc::clone(&seen);
    let mut app = App::default().with_update(move |tick| {
        let mut s = log.borrow_mut();
        s.frame += 1;
        let hero = named(tick.world, "Hero");
        if !s.started {
            s.started = true;
            let doomed = named(tick.world, "Doomed");
            // Fade out over a second while moving right, then up.
            tick.tween(
                hero,
                Property::SpriteColor,
                Value::Color([1.0, 1.0, 1.0, 0.0]),
                1.0,
                Ease::Linear,
            );
            let right = tick.tween(
                hero,
                Property::Translation,
                Value::Vec2(Vec2::new(4.0, 0.0)),
                1.0,
                Ease::EaseInOut,
            );
            s.move_right = Some(right);
            s.move_up = Some(tick.tweens.then(
                right,
                hero,
                Property::Translation,
                Value::Vec2(Vec2::new(4.0, 4.0)),
                0.5,
                Ease::Linear,
            ));
            // A slow shrink on an entity the timer will despawn.
            s.shrink = Some(tick.tween(
                doomed,
                Property::Scale,
                Value::Vec2(Vec2::ZERO),
                3.0,
                Ease::Linear,
            ));
            s.ring = Some(tick.after(0.75, Some(hero)));
            s.orphan = Some(tick.after(2.0, Some(doomed)));
            s.beat = Some(tick.every(0.25, None));
            return;
        }
        let frame = s.frame;
        if s.rang_at.is_some_and(|rang| frame == rang + 1) {
            s.shrink_after_despawn = s.shrink.map(|t| tick.tweens.is_running(t));
        }
        if s.move_right.is_some_and(|t| tick.tweens.finished(t)) {
            s.right_done_at = Some(frame);
        }
        if s.move_up.is_some_and(|t| tick.tweens.finished(t)) {
            s.up_done_at = Some(frame);
        }
        if let Some(beat) = s.beat {
            s.beats += tick.timers.times(beat);
        }
        if s.orphan.is_some_and(|t| tick.timers.fired(t)) {
            s.orphan_rang = true;
        }
        if s.ring.is_some_and(|t| tick.timers.fired(t)) {
            s.rang_at = Some(frame);
            let doomed = named(tick.world, "Doomed");
            tick.world.despawn(doomed);
        }
    });
    for (name, at) in [("Hero", Vec2::ZERO), ("Doomed", Vec2::new(-3.0, 0.0))] {
        let e = app.world.spawn();
        app.world.insert(e, SceneId::new());
        app.world.insert(e, Name::new(name));
        app.world.insert(e, Transform2D::from_translation(at));
        app.world.insert(e, Sprite2D::new([1.0, 1.0, 1.0, 1.0]));
    }
    let mut session = Headless::new(app);
    let hero = named(session.world(), "Hero");
    let doomed = named(session.world(), "Doomed");
    let state = |s: &Headless| {
        let t = s
            .world()
            .get::<Transform2D>(hero)
            .expect("placed")
            .translation;
        let a = s.world().get::<Sprite2D>(hero).expect("a sprite").color[3];
        (t, a)
    };

    // Frame 1 starts them; 30 frames later, half a second in.
    session.step(31, FRAME);
    let (at, alpha) = state(&session);
    assert!((alpha - 0.5).abs() < 0.02, "half faded: {alpha}");
    assert!(
        (at.x - 2.0).abs() < 0.1 && at.y == 0.0,
        "halfway right, eased: {at}"
    );
    let doomed_scale = session
        .world()
        .get::<Transform2D>(doomed)
        .expect("placed")
        .scale;
    assert!(doomed_scale.x < 1.0, "the shrink is running");

    // The timer at 0.75 s despawns the doomed one; its tween and its own
    // timer stop with it.
    // Made in frame 1, first stepped in frame 2: 45 frames is frame 47.
    session.step(17, FRAME);
    assert!(!session.world().is_alive(doomed));
    session.step(28, FRAME);
    let (at, alpha) = state(&session);
    assert_eq!(alpha, 0.0, "faded out");
    assert!(at.y > 0.0 && at.x == 4.0, "on its way up: {at}");

    session.step(60, FRAME);
    let (at, _) = state(&session);
    assert_eq!(at, Vec2::new(4.0, 4.0), "the chain's end");
    let s = seen.borrow();
    let right = s.right_done_at.expect("the move right finished");
    let up = s.up_done_at.expect("the move up finished");
    assert!(
        (60..=62).contains(&right),
        "a second after frame 1: {right}"
    );
    assert!((89..=92).contains(&up), "half a second later: {up}");
    let rang = s.rang_at.expect("the timer fired");
    assert!((46..=47).contains(&rang), "0.75 s after frame 1: {rang}");
    assert!(!s.orphan_rang, "the despawned owner's timer never fires");
    // Frames 2 to 136: 135 steps, 2.25 seconds — the ninth beat falls on
    // the last step, where the sum of sixtieths may land either side.
    assert!(
        (8..=9).contains(&s.beats),
        "every quarter second: {}",
        s.beats
    );
    assert_eq!(
        s.shrink_after_despawn,
        Some(false),
        "the shrink stopped with its entity"
    );
}
