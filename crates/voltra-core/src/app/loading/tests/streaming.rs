//! A camera walked across a generated world: regions come and go around it,
//! the world stays small, and no frame's load work runs over its budget.

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{Camera, Name, Persistent, StreamingAnchor, Transform, WorldRegions};
use voltra_testkit::scratch_root;

use super::{path, run_frame, write_grid, BudgetCheck};
use crate::app::App;

const COLUMNS: u32 = 24;
const ROWS: u32 = 4;
const CELL: f32 = 16.0;
const PER_REGION: usize = 60;
const RADIUS: f32 = 20.0;
const MARGIN: f32 = 6.0;

/// The world file, rewritten with this test's radius and margin.
fn world_root() -> (std::path::PathBuf, voltra_assets::AssetPath) {
    let root = scratch_root();
    let file = write_grid(&root, COLUMNS, ROWS, CELL, PER_REGION);
    let mut world = WorldRegions::read(&root, &file).expect("reads");
    world.load_radius = RADIUS;
    world.unload_margin = MARGIN;
    std::fs::write(
        root.join(file.as_str()),
        world.to_ron().expect("serialises"),
    )
    .expect("writes");
    (root, file)
}

/// The region tags among the world's names.
fn regions_in(world: &World) -> HashSet<String> {
    world
        .query::<Name>()
        .filter_map(|(_, name)| name.as_str().split_once('/').map(|(tag, _)| tag.to_owned()))
        .collect()
}

/// How many regions are within `reach` of `at`.
fn regions_within(world: &WorldRegions, at: Vec2, reach: f32) -> usize {
    world
        .regions
        .iter()
        .filter(|region| region.distance_to(at) <= reach)
        .count()
}

/// Walks an anchored camera along the middle of the world, west to east,
/// checking every frame; returns the regions that were ever in.
fn walk(
    app: &mut App,
    root: &Path,
    file: &voltra_assets::AssetPath,
    camera: Entity,
) -> HashSet<String> {
    let regions = WorldRegions::read(root, file).expect("reads");
    let total = COLUMNS as usize * ROWS as usize * PER_REGION;
    let y = CELL * ROWS as f32 / 2.0;
    let end = CELL * COLUMNS as f32;
    let speed = 0.4;
    let mut seen = HashSet::new();
    let mut most = 0;
    let mut frame = 0;
    let mut budget = BudgetCheck::default();
    let mut x = 0.0;
    while x <= end {
        app.world
            .insert(camera, Transform::from_translation(Vec2::new(x, y)));
        run_frame(app);
        budget.frame(&app.load_stats());

        // Bounded: never more than what a radius plus margin (and the
        // cell an anchor has just walked out of) can hold.
        let bound = regions_within(&regions, Vec2::new(x, y), RADIUS + MARGIN + CELL) * PER_REGION;
        let count = app.world.entity_count() - 1;
        assert!(
            count <= bound,
            "frame {frame}: {count} entities, more than {bound}"
        );
        most = most.max(count);
        seen.extend(regions_in(&app.world));
        x += speed;
        frame += 1;
    }
    budget.finish();
    assert!(
        most * 3 < total,
        "{most} entities at once is not a small part of {total}"
    );
    seen
}

fn anchored_camera(world: &mut World) -> Entity {
    let camera = world.spawn();
    world.insert(camera, Camera::new(8.0));
    world.insert(camera, Transform::default());
    world.insert(camera, StreamingAnchor::default());
    world.insert(camera, Persistent);
    camera
}

/// Every region the walk passed within reach of.
fn expected_regions(root: &Path, file: &voltra_assets::AssetPath) -> HashSet<String> {
    let regions = WorldRegions::read(root, file).expect("reads");
    let y = CELL * ROWS as f32 / 2.0;
    regions
        .regions
        .iter()
        .filter(|region| {
            region.distance_to(Vec2::new(
                region.center().x.clamp(0.0, CELL * COLUMNS as f32),
                y,
            )) < RADIUS - 1.0
        })
        .map(|region| {
            region
                .scene
                .as_str()
                .trim_start_matches("world/")
                .trim_end_matches(".ron")
                .to_owned()
        })
        .collect()
}

fn walk_with(workers: Option<usize>) {
    let (root, file) = world_root();
    let mut app = App::default()
        .with_asset_root(&root)
        .with_load_budget(Duration::from_millis(2));
    if let Some(workers) = workers {
        app = app.with_load_workers(workers);
    }
    let camera = anchored_camera(&mut app.world);
    app.stream_world(&file);
    let prefabs_before = app.prefabs().len();

    let seen = walk(&mut app, &root, &file, camera);

    let expected = expected_regions(&root, &file);
    let missed: Vec<&String> = expected.difference(&seen).collect();
    assert!(missed.is_empty(), "never loaded: {missed:?}");
    // Region scenes are not cached in the prefab store: what is read is
    // dropped when it is in.
    assert_eq!(app.prefabs().len(), prefabs_before);
    let stats = app.streaming().clone();
    assert_eq!(stats.regions, (COLUMNS * ROWS) as usize);
    assert_eq!(stats.broken, 0);

    // Stopping lets every region go, over frames, within the budget.
    app.stop_streaming();
    let mut budget = BudgetCheck::default();
    for _ in 0..2000 {
        run_frame(&mut app);
        budget.frame(&app.load_stats());
        if app.world.entity_count() == 1 && app.streaming().unloading == 0 {
            break;
        }
    }
    budget.finish();
    assert_eq!(app.world.entity_count(), 1, "only the camera is left");
    assert!(app.world.is_alive(camera));
}

#[test]
fn a_camera_walked_across_the_world_keeps_it_small_and_every_frame_in_budget() {
    walk_with(None);
}

#[test]
fn with_no_workers_the_loads_run_on_the_frame_within_the_budget() {
    // What the web does: no threads, every read and parse a unit.
    walk_with(Some(0));
}

#[test]
fn every_anchor_keeps_its_own_regions_in() {
    let (root, file) = world_root();
    let mut app = App::default()
        .with_asset_root(&root)
        .with_load_budget(Duration::from_secs(1));
    let y = CELL * ROWS as f32 / 2.0;
    for x in [8.0, CELL * COLUMNS as f32 - 8.0] {
        let anchor = app.world.spawn();
        app.world
            .insert(anchor, Transform::from_translation(Vec2::new(x, y)));
        app.world.insert(anchor, StreamingAnchor::with_radius(4.0));
    }
    app.stream_world(&file);
    for _ in 0..200 {
        run_frame(&mut app);
    }
    let regions = regions_in(&app.world);
    let first_column: HashSet<String> = (0..ROWS).map(|r| format!("r0_{r}")).collect();
    let last = COLUMNS - 1;
    let last_column: HashSet<String> = (0..ROWS).map(|r| format!("r{last}_{r}")).collect();
    assert!(
        regions.iter().any(|r| first_column.contains(r)),
        "{regions:?}"
    );
    assert!(
        regions.iter().any(|r| last_column.contains(r)),
        "{regions:?}"
    );
    assert!(!regions.contains("r7_1"), "nothing in between: {regions:?}");
}

#[test]
fn with_no_anchor_the_active_camera_is_one() {
    let (root, file) = world_root();
    let mut app = App::default()
        .with_asset_root(&root)
        .with_load_budget(Duration::from_secs(1));
    let camera = app.world.spawn();
    app.world.insert(camera, Camera::new(8.0));
    app.world
        .insert(camera, Transform::from_translation(Vec2::new(8.0, 8.0)));
    app.stream_world(&file);
    for _ in 0..100 {
        run_frame(&mut app);
    }
    assert!(regions_in(&app.world).contains("r0_0"));
    assert!(!regions_in(&app.world).contains("r10_0"));
}

#[test]
fn a_world_file_that_will_not_read_streams_nothing() {
    let root = scratch_root();
    let mut app = App::default().with_asset_root(&root);
    anchored_camera(&mut app.world);
    app.stream_world(&path("missing.world.ron"));
    run_frame(&mut app);
    assert_eq!(app.streaming().world, None);
    assert_eq!(app.world.entity_count(), 1);
}

#[test]
fn a_broken_region_is_left_out_and_the_rest_load() {
    let (root, file) = world_root();
    std::fs::write(root.join("world/r0_0.ron"), "not a scene").expect("writes");
    let mut app = App::default()
        .with_asset_root(&root)
        .with_load_budget(Duration::from_secs(1));
    let camera = anchored_camera(&mut app.world);
    app.world
        .insert(camera, Transform::from_translation(Vec2::new(16.0, 16.0)));
    app.stream_world(&file);
    for _ in 0..100 {
        run_frame(&mut app);
    }
    assert_eq!(app.streaming().broken, 1);
    let regions = regions_in(&app.world);
    assert!(!regions.contains("r0_0"));
    assert!(regions.contains("r1_1"));
}
