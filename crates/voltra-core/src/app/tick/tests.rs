use super::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, RigidBody, Sensor, Transform};

const FRAME: f32 = 1.0 / 60.0;
const G: Vec2 = Vec2::new(0.0, -10.0);

/// A counter a hook can bump and a test can read afterwards.
fn counter() -> (Rc<Cell<u32>>, Rc<Cell<u32>>) {
    let count = Rc::new(Cell::new(0));
    (count.clone(), count)
}

fn app_with_a_falling_body() -> (App, Entity) {
    let mut app = App {
        gravity: G,
        ..Default::default()
    };
    let body = app.world.spawn();
    app.world.insert(body, Transform::default());
    app.world.insert(body, Collider::Circle { radius: 0.5 });
    app.world.insert(body, RigidBody::new_dynamic(1.0));
    (app, body)
}

fn height(app: &App, entity: Entity) -> f32 {
    app.world
        .get::<Transform>(entity)
        .expect("the transform is there")
        .translation
        .y
}

#[test]
fn the_per_frame_tick_runs_once_a_frame_while_the_world_is_live() {
    let (count, seen) = counter();
    let mut app = App::default()
        .with_simulation()
        .with_update(move |_| count.set(count.get() + 1));

    app.advance(FRAME);
    app.advance(FRAME);

    assert_eq!(seen.get(), 2);
}

#[test]
fn the_per_frame_tick_does_not_run_while_the_world_is_stopped() {
    // The editor while authoring. Game logic that ran here would edit the
    // scene behind the author's back.
    let (count, seen) = counter();
    let mut app = App::default().with_update(move |_| count.set(count.get() + 1));

    app.advance(FRAME);

    assert_eq!(seen.get(), 0);
}

#[test]
fn the_per_frame_tick_is_told_the_frame_delta() {
    let delta = Rc::new(Cell::new(0.0));
    let seen = delta.clone();
    let mut app = App::default()
        .with_simulation()
        .with_update(move |tick| delta.set(tick.delta));

    app.advance(0.25);

    assert_eq!(seen.get(), 0.25);
}

#[test]
fn the_tick_reads_this_frame_s_input() {
    let pressed = Rc::new(Cell::new(false));
    let seen = pressed.clone();
    let mut app = App::default().with_simulation().with_update(move |tick| {
        pressed.set(tick.input.was_key_pressed(crate::KeyCode::Space));
    });

    app.input.on_key(crate::KeyCode::Space, true, false);
    app.advance(FRAME);

    assert!(seen.get(), "the edge must reach the game");
}

#[test]
fn a_tick_saves_to_the_game_s_slots() {
    let dir = voltra_testkit::scratch_root();
    let mut app = App::default()
        .with_save_dir(&dir)
        .with_simulation()
        .with_update(|tick| {
            if let Err(e) = tick.saves.save("checkpoint", &42u32) {
                panic!("the save failed: {e}");
            }
        });

    app.advance(FRAME);

    assert_eq!(
        app.saves().load::<u32>("checkpoint").expect("reads"),
        Some(42)
    );
    assert_eq!(app.saves().dir(), Some(dir.path()));
}

#[test]
fn a_bound_key_reaches_the_tick_as_its_action() {
    use crate::input::{ActionMap, Binding};

    let mut map = ActionMap::new();
    map.bind("jump", Binding::Key(crate::KeyCode::Space));
    let pressed = Rc::new(Cell::new(false));
    let seen = pressed.clone();
    let mut app = App::default()
        .with_actions(map)
        .with_simulation()
        .with_update(move |tick| pressed.set(tick.input.action("jump").pressed()));

    app.input.on_key(crate::KeyCode::Space, true, false);
    app.advance(FRAME);

    assert!(seen.get());
}

#[test]
fn a_rebind_from_the_tick_drives_the_action_next_frame() {
    use crate::input::{ActionMap, Binding};

    let mut map = ActionMap::new();
    map.bind("jump", Binding::Key(crate::KeyCode::Space));
    let held = Rc::new(Cell::new(false));
    let seen = held.clone();
    let mut app = App::default()
        .with_actions(map)
        .with_simulation()
        .with_update(move |tick| {
            if let Some(binding) = tick.input.pressed_binding() {
                tick.actions.rebind("jump", 0, binding);
            }
            held.set(tick.input.action("jump").is_held());
        });

    app.input.on_key(crate::KeyCode::KeyJ, true, false);
    app.advance(FRAME);
    assert!(!seen.get(), "J was not jump when the frame began");
    app.input.end_frame();
    app.advance(FRAME);
    assert!(seen.get(), "and is from the next frame on");
    assert_eq!(
        app.actions().bindings("jump"),
        &[Binding::Key(crate::KeyCode::KeyJ)]
    );
}

#[test]
fn the_fixed_tick_runs_once_per_step_owed() {
    let (count, seen) = counter();
    let mut app = App::default()
        .with_simulation()
        .with_fixed_update(move |_| count.set(count.get() + 1));

    app.advance(FRAME * 3.0);

    assert_eq!(seen.get(), 3);
}

#[test]
fn the_fixed_tick_is_told_the_fixed_step_not_the_frame() {
    let delta = Rc::new(Cell::new(0.0));
    let seen = delta.clone();
    let mut app = App::default()
        .with_simulation()
        .with_fixed_update(move |tick| delta.set(tick.delta));

    app.advance(FRAME * 2.5);

    assert!(
        (seen.get() - FRAME).abs() < 1e-6,
        "expected the fixed step, got {}",
        seen.get()
    );
}

#[test]
fn a_frame_that_owes_no_step_runs_no_fixed_tick() {
    let (count, seen) = counter();
    let mut app = App::default()
        .with_simulation()
        .with_fixed_update(move |_| count.set(count.get() + 1));

    app.advance(FRAME * 0.5);

    assert_eq!(seen.get(), 0, "half a step owes nothing yet");
}

#[test]
fn a_requested_step_runs_the_fixed_tick_even_while_stopped() {
    // The editor's Step button: one step of the whole world, logic and all.
    let (count, seen) = counter();
    let mut app = App::default().with_fixed_update(move |_| count.set(count.get() + 1));

    app.request_steps(1);
    app.advance(0.0);

    assert_eq!(seen.get(), 1);
}

#[test]
fn what_the_tick_writes_is_simulated_in_the_same_frame() {
    // The reason the tick runs before the steps rather than after: a
    // velocity set from input must not wait a step to be integrated.
    let (app, body) = app_with_a_falling_body();
    let mut app = app.with_simulation().with_update(move |tick| {
        if let Some(rigid) = tick.world.get_mut::<RigidBody>(body) {
            rigid.velocity = Vec2::new(0.0, 5.0);
        }
    });

    app.advance(FRAME);

    assert!(
        height(&app, body) > 0.0,
        "the body should have risen this frame, not next"
    );
}

/// A static sensor box at the origin with a dynamic ball sitting inside
/// it, and no gravity to move either of them.
fn app_with_a_trigger() -> (App, Entity, Entity) {
    let mut app = App::default();

    let trigger = app.world.spawn();
    app.world.insert(trigger, Transform::default());
    app.world.insert(
        trigger,
        Collider::Box {
            half_extents: Vec2::splat(1.0),
        },
    );
    app.world.insert(trigger, Sensor);

    let ball = app.world.spawn();
    app.world.insert(ball, Transform::default());
    app.world.insert(ball, Collider::Circle { radius: 0.5 });
    app.world.insert(ball, RigidBody::new_dynamic(1.0));

    (app, trigger, ball)
}

/// How many events each turn of a hook was handed, in order.
type Log = Rc<RefCell<Vec<usize>>>;

fn log() -> (Log, Log) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    (seen.clone(), seen)
}

#[test]
fn the_per_frame_tick_is_handed_each_event_exactly_once() {
    let (app, trigger, ball) = app_with_a_trigger();
    let (record, seen) = log();
    let touched = Rc::new(Cell::new(None));
    let other = touched.clone();
    let mut app = app.with_simulation().with_update(move |tick| {
        record.borrow_mut().push(tick.events.len());
        if let Some(event) = tick.events.first() {
            other.set(event.other(ball));
        }
    });

    app.advance(FRAME);
    app.advance(FRAME);
    app.advance(FRAME * 0.1);

    assert_eq!(
        *seen.borrow(),
        vec![0, 1, 0],
        "nothing has stepped, then the overlap begins, then it is spent"
    );
    assert_eq!(touched.get(), Some(trigger));
}

#[test]
fn the_per_frame_tick_sees_every_step_of_the_frame_not_only_the_last() {
    // A pickup taken on the first of two steps in one frame must not be
    // lost because the second step replaced what physics remembers.
    let (mut app, _, ball) = app_with_a_trigger();
    app.world
        .get_mut::<RigidBody>(ball)
        .expect("it is a body")
        .velocity = Vec2::new(0.0, -100.0);

    let (record, seen) = log();
    let mut app = app
        .with_simulation()
        .with_update(move |tick| record.borrow_mut().push(tick.events.len()));

    app.advance(FRAME * 2.0);
    app.advance(FRAME * 0.1);

    assert_eq!(
        *seen.borrow(),
        vec![0, 2],
        "it began inside and ended outside, both in one frame"
    );
}

#[test]
fn the_fixed_tick_sees_the_events_of_the_step_before_it() {
    let (app, _, _) = app_with_a_trigger();
    let (record, seen) = log();
    let mut app = app
        .with_simulation()
        .with_fixed_update(move |tick| record.borrow_mut().push(tick.events.len()));

    app.advance(FRAME);
    app.advance(FRAME);

    assert_eq!(
        *seen.borrow(),
        vec![0, 1],
        "the first step has nothing behind it; the second follows the overlap"
    );
}

#[test]
fn the_tick_sees_the_contacts_of_the_last_step() {
    // How a game asks whether it is standing on something.
    let (mut app, _) = app_with_a_falling_body();
    let floor = app.world.spawn();
    app.world
        .insert(floor, Transform::from_translation(Vec2::new(0.0, -1.0)));
    app.world.insert(
        floor,
        Collider::Box {
            half_extents: Vec2::new(10.0, 0.5),
        },
    );
    app.world.insert(floor, RigidBody::new_static());

    let (count, seen) = counter();
    let mut app = app.with_simulation().with_update(move |tick| {
        if !tick.contacts.is_empty() {
            count.set(count.get() + 1);
        }
    });

    for _ in 0..120 {
        app.advance(FRAME);
    }

    assert!(seen.get() > 0, "the body lands and the tick should see it");
}

/// A ball hung from the world on a joint too weak for it: it breaks on the
/// first step.
fn app_with_a_weak_joint() -> (App, Entity) {
    let (mut app, ball) = app_with_a_falling_body();
    app.world.insert(
        ball,
        voltra_scene::Joint2D {
            break_force: Some(0.5),
            ..voltra_scene::Joint2D::default()
        },
    );
    (app, ball)
}

#[test]
fn each_hook_is_handed_a_broken_joint_once() {
    let (app, ball) = app_with_a_weak_joint();
    let (frame_record, frame_seen) = log();
    let (fixed_record, fixed_seen) = log();
    let named = Rc::new(Cell::new(None));
    let name = named.clone();
    let mut app = app
        .with_simulation()
        .with_update(move |tick| {
            frame_record.borrow_mut().push(tick.broken_joints.len());
            if let Some(broken) = tick.broken_joints.first() {
                name.set(Some(broken.entity));
            }
        })
        .with_fixed_update(move |tick| fixed_record.borrow_mut().push(tick.broken_joints.len()));

    app.advance(FRAME);
    app.advance(FRAME);
    app.advance(FRAME);

    assert_eq!(
        *frame_seen.borrow(),
        vec![0, 1, 0],
        "the frame after it broke"
    );
    assert_eq!(
        *fixed_seen.borrow(),
        vec![0, 1, 0],
        "the step after it broke"
    );
    assert_eq!(named.get(), Some(ball));
    assert!(app.world.get::<voltra_scene::Joint2D>(ball).is_none());
}
