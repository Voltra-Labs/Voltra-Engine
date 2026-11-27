//! Lines, filled polygons and trails (stage 101), screenshotted headless:
//! each joint and cap where it should and should not cover, a concave
//! polygon filled without its notches, sorting against a sprite, and a
//! ribbon behind a moving body that fades towards its tail. Each skips
//! without a GPU adapter.

use voltra_core::{App, Headless, Screenshot, ScreenshotError, WindowConfig};
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{
    BodyType, Camera2D, Line2D, LineCap, LineJoint, Polygon2D, RigidBody2D, Sprite2D, Trail2D,
    Transform2D,
};

/// 128 × 128 pixels over four units: 32 a unit, the origin at the middle.
const SHOT: u32 = 128;
const PER_UNIT: f32 = 32.0;

fn session() -> Headless {
    let mut app =
        App::new(WindowConfig::default()).with_asset_root(voltra_testkit::scratch_root().keep());
    let camera = app.world.spawn();
    app.world.insert(camera, Transform2D::default());
    app.world.insert(camera, Camera2D::new(2.0));
    Headless::new(app)
}

fn shoot(session: &mut Headless) -> Option<Screenshot> {
    match session.screenshot(SHOT, SHOT) {
        Ok(shot) => Some(shot),
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no GPU adapter; skipping");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

/// The pixel over world point `at`.
fn pixel(shot: &Screenshot, at: Vec2) -> [u8; 4] {
    let x = (SHOT as f32 / 2.0 + at.x * PER_UNIT) as u32;
    let y = (SHOT as f32 / 2.0 - at.y * PER_UNIT) as u32;
    shot.pixel(x, y).expect("on the shot")
}

fn white(shot: &Screenshot, at: Vec2) -> bool {
    pixel(shot, at)[..3].iter().all(|&c| c > 200)
}

fn drawn<T: 'static>(world: &mut World, component: T) {
    let e = world.spawn();
    world.insert(e, Transform2D::default());
    world.insert(e, component);
}

/// One `line` on its own.
fn shot_of(line: Line2D) -> Option<Screenshot> {
    let mut session = session();
    drawn(session.world_mut(), line);
    shoot(&mut session)
}

#[test]
fn joints_fill_the_outer_corner_each_its_own_way() {
    // Right then up, half a unit wide: the outer corner is at (1.25, -1.25).
    let corner = |joint| Line2D {
        points: vec![
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
        ],
        width: 0.5,
        joint,
        ..Line2D::default()
    };
    let tip = Vec2::new(1.2, -1.2);
    let edge = Vec2::new(1.14, -1.14);
    let Some(miter) = shot_of(corner(LineJoint::Miter)) else {
        return;
    };
    let bevel = shot_of(corner(LineJoint::Bevel)).expect("a shot");
    let round = shot_of(corner(LineJoint::Round)).expect("a shot");
    for shot in [&miter, &bevel, &round] {
        assert!(white(shot, Vec2::new(0.0, -1.0)), "the line itself");
        assert!(!white(shot, Vec2::new(0.0, 0.0)), "not inside the corner");
    }
    assert!(white(&miter, tip), "a miter reaches its point");
    assert!(
        !white(&bevel, tip) && !white(&bevel, edge),
        "a bevel cuts it off"
    );
    assert!(
        !white(&round, tip) && white(&round, edge),
        "round, in between"
    );
}

#[test]
fn caps_end_an_open_line_each_its_own_way() {
    let line = |cap| Line2D {
        points: vec![Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0)],
        width: 0.5,
        begin_cap: cap,
        end_cap: cap,
        ..Line2D::default()
    };
    let past = Vec2::new(1.15, 0.0);
    let corner = Vec2::new(1.2, 0.2);
    let Some(butt) = shot_of(line(LineCap::Butt)) else {
        return;
    };
    let square = shot_of(line(LineCap::Square)).expect("a shot");
    let round = shot_of(line(LineCap::Round)).expect("a shot");
    assert!(!white(&butt, past), "a butt end stops at its point");
    assert!(
        white(&square, past) && white(&square, corner),
        "a square one goes on"
    );
    assert!(
        white(&round, past) && !white(&round, corner),
        "a round one rounds"
    );
    assert!(
        white(&round, Vec2::new(-1.15, 0.0)),
        "at the first point too"
    );
}

#[test]
fn a_concave_polygon_is_filled_without_its_notches() {
    let mut session = session();
    let star: Vec<Vec2> = (0..10)
        .map(|i| {
            let r = if i % 2 == 0 { 1.8 } else { 0.6 };
            Vec2::from_angle(std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 10.0)
                * r
        })
        .collect();
    drawn(
        session.world_mut(),
        Polygon2D {
            points: star,
            color: [1.0, 1.0, 1.0, 1.0],
            ..Polygon2D::default()
        },
    );
    let Some(shot) = shoot(&mut session) else {
        return;
    };
    assert!(white(&shot, Vec2::ZERO), "the middle");
    assert!(white(&shot, Vec2::new(0.0, 1.5)), "the top point");
    // Between the top point and the next, past the notch's inner corner.
    let notch = Vec2::from_angle(std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU / 20.0) * 1.2;
    assert!(!white(&shot, notch), "the notch is empty at {notch}");
}

#[test]
fn lines_and_polygons_sort_with_sprites() {
    let crossing = |line_order| {
        let mut session = session();
        let world = session.world_mut();
        let sprite = world.spawn();
        world.insert(sprite, Transform2D::default().with_scale(Vec2::splat(2.0)));
        world.insert(
            sprite,
            Sprite2D {
                sort_order: 0,
                ..Sprite2D::new([1.0, 0.0, 0.0, 1.0])
            },
        );
        drawn(
            world,
            Line2D {
                points: vec![Vec2::new(-1.5, 0.0), Vec2::new(1.5, 0.0)],
                width: 0.5,
                sort_order: line_order,
                ..Line2D::default()
            },
        );
        shoot(&mut session)
    };
    let Some(over) = crossing(1) else {
        return;
    };
    let under = crossing(-1).expect("a shot");
    assert!(white(&over, Vec2::ZERO), "drawn over the sprite");
    let red = pixel(&under, Vec2::ZERO);
    assert!(red[0] > 200 && red[1] < 60, "drawn under it: {red:?}");
    assert!(white(&under, Vec2::new(1.3, 0.0)), "and seen past its edge");
}

#[test]
fn a_trail_follows_a_moving_body_and_fades_behind_it() {
    let mut session = session();
    let world = session.world_mut();
    let body = world.spawn();
    world.insert(body, Transform2D::from_translation(Vec2::new(-1.8, 0.0)));
    world.insert(
        body,
        RigidBody2D {
            body_type: BodyType::Kinematic,
            velocity: Vec2::new(3.0, 0.0),
            ..RigidBody2D::default()
        },
    );
    world.insert(
        body,
        Trail2D {
            lifetime: 0.5,
            width: 0.4,
            min_distance: 0.02,
            ..Trail2D::default()
        },
    );
    // One second at three units a second: from -1.8 to 1.2; half a second
    // of ribbon reaches back to -0.3.
    session.step(60, 1.0 / 60.0);
    let Some(shot) = shoot(&mut session) else {
        return;
    };
    let at = session
        .world()
        .get::<Transform2D>(body)
        .expect("the body")
        .translation;
    assert!((at.x - 1.2).abs() < 0.1, "the body moved: {at}");
    let brightness = |x: f32| pixel(&shot, Vec2::new(x, 0.0))[0];
    let (fresh, older, gone, ahead) = (
        brightness(1.0),
        brightness(0.2),
        brightness(-0.8),
        brightness(1.6),
    );
    assert!(fresh > 200, "bright just behind the body: {fresh}");
    assert!(
        older < fresh && older > gone,
        "fading towards its tail: {fresh} {older} {gone}"
    );
    let clear = shot.pixel(0, 0).expect("a corner")[0];
    assert!(
        gone.abs_diff(clear) < 8 && ahead.abs_diff(clear) < 8,
        "nothing past its life or ahead: {gone} {ahead}, the clear colour {clear}"
    );
}
