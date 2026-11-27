//! Where a view is: its own turn or its parents', normalised; a click
//! through it; the shape it draws into.

use super::tests::spawn;
use super::*;
use crate::hierarchy::link::set_parent;
use crate::transform::Transform2D;
use voltra_render::glam::Vec2;

#[test]
fn the_view_turns_as_its_entity_does_through_its_parents() {
    let mut world = World::new();
    let ship = spawn(&mut world, Camera2D::default(), Vec2::new(10.0, 0.0));
    world
        .get_mut::<Transform2D>(ship)
        .expect("spawned with one")
        .rotation = 0.5;
    let eye = spawn(&mut world, Camera2D::default(), Vec2::new(0.0, 2.0));
    world
        .get_mut::<Transform2D>(eye)
        .expect("spawned with one")
        .rotation = 0.25;
    set_parent(&mut world, eye, ship).expect("a plain reparent");

    // By default only its own turn: a camera on a rolling ball stays
    // upright, as before views turned — though it still rides the parent.
    let own = view(&world, eye, 1.0).expect("it has a camera");
    assert!((own.rotation - 0.25).abs() < 1e-6, "{}", own.rotation);
    let expected = Vec2::new(10.0, 0.0) + Vec2::from_angle(0.5).rotate(Vec2::new(0.0, 2.0));
    assert!((own.position - expected).length() < 1e-5);

    world
        .get_mut::<Camera2D>(eye)
        .expect("a camera")
        .inherit_rotation = true;
    let inherited = view(&world, eye, 1.0).expect("it has a camera");
    assert!(
        (inherited.rotation - 0.75).abs() < 1e-5,
        "{}",
        inherited.rotation
    );
    assert!((inherited.position - expected).length() < 1e-5);
}

#[test]
fn inheriting_the_turn_is_off_and_not_written_by_default() {
    let written = ron::to_string(&Camera2D::default()).expect("writes");
    assert!(!written.contains("inherit_rotation"), "{written}");
    let read: Camera2D =
        ron::from_str("(size: 5.0, priority: 0, active: true)").expect("an old camera reads");
    assert!(!read.inherit_rotation);
}

#[test]
fn a_turn_that_cancels_out_or_goes_round_whole_is_upright() {
    let mut world = World::new();
    let parent = spawn(&mut world, Camera2D::default(), Vec2::ZERO);
    world.get_mut::<Transform2D>(parent).expect("t").rotation = 0.3;
    let child = spawn(
        &mut world,
        Camera2D {
            inherit_rotation: true,
            ..Camera2D::default()
        },
        Vec2::ZERO,
    );
    world.get_mut::<Transform2D>(child).expect("t").rotation = -0.3;
    set_parent(&mut world, child, parent).expect("a plain reparent");
    assert_eq!(placement(&world, child).1, 0.0, "0.3 − 0.3 is upright");

    assert_eq!(normalised_turn(std::f32::consts::TAU), 0.0);
    assert_eq!(normalised_turn(-std::f32::consts::TAU), 0.0);
    assert_eq!(normalised_turn(4.0 * std::f32::consts::PI), 0.0);
    assert_eq!(normalised_turn(f32::NAN), 0.0);
    assert!((normalised_turn(std::f32::consts::TAU + 0.5) - 0.5).abs() < 1e-5);
    assert!((normalised_turn(-0.5) + 0.5).abs() < 1e-6);
    assert!((normalised_turn(3.5) - (3.5 - std::f32::consts::TAU)).abs() < 1e-5);
}

#[test]
fn a_camera_s_aspect_is_its_viewport_s_share_of_the_screen() {
    let mut world = World::new();
    let left = spawn(
        &mut world,
        Camera2D {
            viewport: Viewport::LEFT,
            ..Camera2D::default()
        },
        Vec2::ZERO,
    );
    let full = spawn(&mut world, Camera2D::default(), Vec2::ZERO);
    let plain = world.spawn();
    assert_eq!(aspect(&world, left, 2.0, None), 1.0, "half of a 2:1 screen");
    assert_eq!(aspect(&world, full, 2.0, None), 2.0);
    assert_eq!(
        aspect(&world, plain, 2.0, None),
        2.0,
        "no camera: the screen's"
    );
    assert_eq!(aspect(&world, full, f32::NAN, None), 1.0);
}

#[test]
fn a_click_through_a_turned_camera_picks_what_is_drawn_under_it() {
    use crate::pick;
    use crate::sprite::sheets::Sheets;
    use crate::Sprite2D;

    let mut world = World::new();
    let eye = spawn(&mut world, Camera2D::new(5.0), Vec2::new(2.0, 1.0));
    world
        .get_mut::<Transform2D>(eye)
        .expect("spawned with one")
        .rotation = std::f32::consts::FRAC_PI_2;
    // Two units to the camera's right in the world: turned a quarter turn
    // left, the view shows it straight below its centre.
    let target = world.spawn();
    world.insert(target, Transform2D::from_translation(Vec2::new(4.0, 1.0)));
    world.insert(target, Sprite2D::default());
    // And one where an unturned click would land, to be missed.
    let decoy = world.spawn();
    world.insert(decoy, Transform2D::from_translation(Vec2::new(2.0, -1.0)));
    world.insert(decoy, Sprite2D::default());

    let viewport = Vec2::new(400.0, 400.0);
    let view = view(&world, eye, 1.0).expect("it has a camera");
    // Two units below the centre: 5 units are 200 pixels, so 80 pixels.
    let click = Vec2::new(200.0, 280.0);
    let at = view.viewport_to_world(click, viewport);
    assert!(at.abs_diff_eq(Vec2::new(4.0, 1.0), 1e-4), "{at}");
    let sheets = Sheets::default().through(&view);
    assert_eq!(pick::sprite_at(&world, at, sheets), Some(target));
    assert!(view
        .world_to_viewport(Vec2::new(4.0, 1.0), viewport)
        .abs_diff_eq(click, 1e-3));
}
