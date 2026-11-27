//! A scene change behind a loading screen: the screen up at once, progress
//! from 0 to 1, the new scene in and the screen gone, the physics held
//! still in between — and a file that will not load changing nothing.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider2D, Name, Persistent, RigidBody2D, Transform2D};
use voltra_testkit::scratch_root;

use super::{named, path, run_frame, write_scene, write_world, BudgetCheck};
use crate::app::{App, SceneChangeStatus};

/// What one frame's tick saw.
#[derive(Debug, Clone)]
struct Seen {
    status: Option<SceneChangeStatus>,
    screen: bool,
    old: bool,
    new: usize,
}

fn names(world: &World) -> Vec<String> {
    world
        .query::<Name>()
        .map(|(_, name)| name.as_str().to_owned())
        .collect()
}

fn root() -> voltra_testkit::Scratch {
    let root = scratch_root();
    write_scene(
        &root,
        "scenes/big.ron",
        "big",
        Vec2::ZERO,
        Vec2::splat(10.0),
        40,
        4,
    );
    let mut screen = World::new();
    named(&mut screen, "loading screen", Vec2::ZERO);
    write_world(&root, "prefabs/loading.prefab.ron", &screen);
    root
}

/// An app whose tick asks for `target` on its first frame and records what
/// every frame's tick sees.
fn app_changing_to(root: &Path, target: &str) -> (App, Rc<RefCell<Vec<Seen>>>) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    let target = path(target);
    let mut first = true;
    let mut app = App::default()
        .with_asset_root(root)
        .with_simulation()
        .with_update(move |tick| {
            if first {
                first = false;
                tick.change_scene(&target, Some(&path("prefabs/loading.prefab.ron")));
            }
            let names = names(tick.world);
            log.borrow_mut().push(Seen {
                status: tick.scene_change(),
                screen: names.iter().any(|n| n == "loading screen"),
                old: names.iter().any(|n| n == "old"),
                new: names.iter().filter(|n| n.starts_with("big/")).count(),
            });
        });
    named(&mut app.world, "old", Vec2::ZERO);
    (app, seen)
}

#[test]
fn progress_runs_from_zero_to_one_behind_the_screen_and_the_new_scene_appears() {
    let root = root();
    // Nothing but one unit a frame: the change takes many frames, and the
    // tick sees every step of it.
    let (mut app, seen) = app_changing_to(&root, "scenes/big.ron");
    app = app.with_load_budget(Duration::ZERO);
    // A persistent body: it survives, and must not fall while the level is
    // half built.
    let body = named(&mut app.world, "player", Vec2::new(0.0, 5.0));
    app.world.insert(body, Persistent);
    app.world.insert(body, RigidBody2D::new_dynamic(1.0));
    app.world.insert(body, Collider2D::default());
    app.gravity = Vec2::new(0.0, -9.81);

    let mut height_while_loading = Vec::new();
    let mut budget = BudgetCheck::default();
    for frame in 0..5000 {
        run_frame(&mut app);
        budget.frame(&app.load_stats());
        if app
            .scene_change()
            .is_some_and(SceneChangeStatus::is_loading)
        {
            let y = app
                .world
                .get::<Transform2D>(body)
                .expect("kept")
                .translation
                .y;
            height_while_loading.push(y);
        }
        if seen
            .borrow()
            .last()
            .is_some_and(|s| s.status.is_none() && frame > 2)
        {
            break;
        }
    }
    budget.finish();

    let seen = seen.borrow().clone();
    // The asking frame saw nothing yet; the next sees the screen at 0.
    assert_eq!(seen[0].status, None);
    assert_eq!(
        seen[1].status,
        Some(SceneChangeStatus::Loading { progress: 0.0 })
    );
    let loading: Vec<&Seen> = seen
        .iter()
        .filter(|s| s.status.is_some_and(SceneChangeStatus::is_loading))
        .collect();
    assert!(loading.len() > 100, "{} frames of loading", loading.len());
    assert!(
        loading.iter().all(|s| s.screen),
        "the screen is up throughout"
    );
    let progress: Vec<f32> = loading
        .iter()
        .map(|s| s.status.expect("loading").progress())
        .collect();
    assert!(
        progress.windows(2).all(|w| w[0] <= w[1]),
        "progress only rises: {progress:?}"
    );
    assert!(progress.last().is_some_and(|p| *p <= 1.0 && *p >= 0.9));
    // The new scene arrives a part at a time behind it.
    assert!(loading.iter().any(|s| s.new > 0 && s.new < 200));

    let finished = seen
        .iter()
        .find(|s| s.status == Some(SceneChangeStatus::Finished))
        .expect("finished");
    assert!(!finished.screen, "the screen is down");
    assert!(!finished.old, "the old scene is gone");
    assert_eq!(finished.new, 200);
    assert_eq!(seen.last().expect("frames").status, None);

    // The physics held still while the level was half built.
    assert!(height_while_loading.windows(2).all(|w| w[0] == w[1]));
    // And steps again after.
    let before = app
        .world
        .get::<Transform2D>(body)
        .expect("kept")
        .translation
        .y;
    for _ in 0..10 {
        run_frame(&mut app);
    }
    let after = app
        .world
        .get::<Transform2D>(body)
        .expect("kept")
        .translation
        .y;
    assert!(after < before, "falls again: {before} -> {after}");
}

#[test]
fn a_change_within_a_budget_is_still_whole_and_in_order() {
    let root = root();
    let (mut app, seen) = app_changing_to(&root, "scenes/big.ron");
    app = app.with_load_budget(Duration::from_millis(1));
    let mut budget = BudgetCheck::default();
    for frame in 0..2000 {
        run_frame(&mut app);
        budget.frame(&app.load_stats());
        if app.scene_change().is_none() && frame > 2 {
            break;
        }
    }
    budget.finish();
    let mut names = names(&app.world);
    names.sort();
    assert_eq!(names.len(), 200);
    assert!(names.iter().all(|n| n.starts_with("big/")));
    assert!(seen
        .borrow()
        .iter()
        .any(|s| s.status == Some(SceneChangeStatus::Finished)));
}

#[test]
fn a_file_that_will_not_load_leaves_the_old_scene_and_takes_the_screen_down() {
    let root = root();
    let (mut app, seen) = app_changing_to(&root, "scenes/missing.ron");
    for _ in 0..200 {
        run_frame(&mut app);
        if seen
            .borrow()
            .last()
            .is_some_and(|s| s.status == Some(SceneChangeStatus::Failed))
        {
            break;
        }
    }
    run_frame(&mut app);
    let last = seen.borrow().last().cloned().expect("frames");
    assert!(last.old, "the old scene stands");
    assert!(!last.screen, "the screen is down");
    assert!(seen
        .borrow()
        .iter()
        .any(|s| s.status == Some(SceneChangeStatus::Failed)));
}

#[test]
fn a_broken_component_is_found_before_anything_is_swept() {
    let root = root();
    let mut broken = World::new();
    named(&mut broken, "broken", Vec2::ZERO);
    let mut file = voltra_scene::format::to_scene_file(
        &broken,
        &voltra_scene::ComponentRegistry::with_defaults(),
    )
    .expect("saves");
    let bad = ron::value::RawValue::from_ron("(translation: \"left\")").expect("valid ron");
    file.entities[0]
        .components
        .insert("Transform".to_owned(), bad.to_owned());
    std::fs::write(
        root.join("scenes/broken.ron"),
        ron::to_string(&file).expect("serialises"),
    )
    .expect("writes");
    let (mut app, seen) = app_changing_to(&root, "scenes/broken.ron");
    for _ in 0..200 {
        run_frame(&mut app);
    }
    let last = seen.borrow().last().cloned().expect("frames");
    assert!(last.old, "the old scene stands");
    assert!(!last.screen);
}

#[test]
fn an_immediate_load_scene_drops_a_change_underway() {
    let root = root();
    let mut small = World::new();
    named(&mut small, "small", Vec2::ZERO);
    write_world(&root, "scenes/small.ron", &small);
    let (mut app, _) = app_changing_to(&root, "scenes/big.ron");
    app = app.with_load_budget(Duration::ZERO);
    for _ in 0..20 {
        run_frame(&mut app);
    }
    assert!(app
        .scene_change()
        .is_some_and(SceneChangeStatus::is_loading));
    app.load_scene(&path("scenes/small.ron"));
    run_frame(&mut app);
    assert_eq!(names(&app.world), vec!["small".to_owned()]);
    for _ in 0..20 {
        run_frame(&mut app);
    }
    assert_eq!(names(&app.world), vec!["small".to_owned()]);
}
