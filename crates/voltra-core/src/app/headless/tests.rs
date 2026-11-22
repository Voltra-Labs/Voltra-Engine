//! The shipped scenes, stepped without a window.

use std::path::PathBuf;

use voltra_scene::{BodyType, RigidBody, SceneId, Transform};

use super::*;

fn assets() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

fn sandbox() -> Headless {
    let app = App::new(crate::WindowConfig::default()).with_asset_root(assets());
    Headless::open(app, &AssetPath::new("scenes/sandbox.ron").expect("valid"))
        .expect("the sandbox loads")
}

/// The first dynamic body in the world, by id and height.
fn a_falling_body(session: &Headless) -> (SceneId, f32) {
    let world = session.world();
    world
        .query::<RigidBody>()
        .filter(|(_, body)| body.body_type == BodyType::Dynamic)
        .find_map(|(entity, _)| {
            Some((
                *world.get::<SceneId>(entity)?,
                world.get::<Transform>(entity)?.translation.y,
            ))
        })
        .expect("the sandbox has a dynamic body")
}

fn height(session: &Headless, id: SceneId) -> f32 {
    let entity = entity_with_id(session.world(), id).expect("still there");
    session
        .world()
        .get::<Transform>(entity)
        .expect("placed")
        .translation
        .y
}

#[test]
fn a_scene_steps_without_a_window_and_its_bodies_fall() {
    let mut session = sandbox();
    let (body, before) = a_falling_body(&session);
    session.step(30, 1.0 / 60.0);
    assert_eq!(session.frames(), 30);
    assert!(
        height(&session, body) < before,
        "{} then {}",
        before,
        height(&session, body)
    );
}

#[test]
fn a_component_is_set_by_name_from_text_and_read_back() {
    let mut session = sandbox();
    let (body, _) = a_falling_body(&session);
    session
        .set_component(
            body,
            "Transform",
            "(translation: (0.0, 50.0), rotation: 0.0, scale: (1.0, 1.0))",
        )
        .expect("a transform reads");
    assert_eq!(height(&session, body), 50.0);
    let text = session.entity_ron(body).expect("written");
    assert!(
        text.contains("Transform") && text.contains("50.0"),
        "{text}"
    );

    assert!(matches!(
        session.set_component(body, "Transform", "(translation: oops)"),
        Err(HeadlessError::Scene(_))
    ));
    assert!(matches!(
        session.set_component(body, "Physics", "()"),
        Err(HeadlessError::NoComponent(_))
    ));
    assert!(matches!(
        session.set_component(SceneId::new(), "Transform", "()"),
        Err(HeadlessError::NoEntity(_))
    ));
}

#[test]
fn a_fed_action_reaches_the_game_s_tick() {
    let app = App::new(crate::WindowConfig::default()).with_asset_root(assets());
    let mut session = Headless::new(app.with_fixed_update(|tick| {
        let push = tick.input.action_axis("move_left", "move_right");
        let mover = tick.world.query::<Transform>().map(|(e, _)| e).next();
        if let Some(transform) = mover.and_then(|e| tick.world.get_mut::<Transform>(e)) {
            transform.translation.x += push * tick.delta;
        }
    }));
    let mover = session.world_mut().spawn();
    session.world_mut().insert(mover, Transform::default());
    assert!(session.press("move_right"));
    session.step(60, 1.0 / 60.0);
    let x = session
        .world()
        .get::<Transform>(mover)
        .expect("placed")
        .translation
        .x;
    assert!((x - 1.0).abs() < 0.05, "a second at full tilt: {x}");
    assert!(session.release("move_right"));
    session.step(30, 1.0 / 60.0);
    let still = session
        .world()
        .get::<Transform>(mover)
        .expect("placed")
        .translation;
    assert_eq!(still.x, x, "let go, it stops");
    assert!(!session.press("no such action"));
}

#[test]
fn a_scene_that_is_not_there_is_an_error() {
    let app = App::new(crate::WindowConfig::default()).with_asset_root(assets());
    assert!(Headless::open(app, &AssetPath::new("scenes/missing.ron").expect("valid")).is_err());
}

#[test]
fn a_screenshot_draws_the_scene_through_its_camera() {
    use voltra_scene::{Camera, Sprite};
    let app = App::new(crate::WindowConfig::default()).with_asset_root(assets());
    let mut session = Headless::new(app);
    let world = session.world_mut();
    let camera = world.spawn();
    world.insert(camera, Transform::default());
    world.insert(camera, Camera::new(1.0));
    let square = world.spawn();
    world.insert(
        square,
        Transform::default().with_scale(voltra_render::glam::Vec2::splat(1.0)),
    );
    world.insert(square, Sprite::new([1.0, 0.0, 0.0, 1.0]));

    let shot = match session.screenshot(64, 64) {
        Ok(shot) => shot,
        Err(ScreenshotError::NoDevice) => {
            eprintln!("no adapter: skipping");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    assert_eq!((shot.width, shot.height), (64, 64));
    let centre = shot.pixel(32, 32).expect("on the image");
    assert!(
        centre[0] > 200 && centre[1] < 40 && centre[2] < 40,
        "red in the middle: {centre:?}"
    );
    let corner = shot.pixel(1, 1).expect("on the image");
    assert!(
        corner[0] < 100,
        "the clear colour at the corner: {corner:?}"
    );
    let png = shot.png_bytes().expect("encodes");
    assert_eq!(&png[..4], b"\x89PNG");

    // Again, at another size, reusing the device.
    let wide = session.screenshot(96, 48).expect("a second shot");
    assert_eq!(wide.rgba.len(), 96 * 48 * 4);
}

#[test]
fn a_session_hands_back_what_the_engine_logged_while_it_ran() {
    crate::logging::init(false);
    let app = App::new(crate::WindowConfig::default()).with_asset_root(assets());
    let mut session = Headless::new(app);
    let _ = session.take_logs();
    // A follower naming a target that is not there warns, once.
    let follower = session.world_mut().spawn();
    session.world_mut().insert(follower, Transform::default());
    session
        .world_mut()
        .insert(follower, voltra_scene::CameraFollow2D::new(SceneId::new()));
    session.step(3, 1.0 / 60.0);
    let lines = session.take_logs();
    let warned = lines
        .iter()
        .filter(|l| l.message.contains("a follower's target"))
        .count();
    assert!(warned >= 1, "the warning is handed back: {lines:?}");
    session.step(3, 1.0 / 60.0);
    assert!(
        !session
            .take_logs()
            .iter()
            .any(|l| l.message.contains("a follower's target") && l.target.contains("camera_rig")),
        "warned once, and read once"
    );
}
