//! The store: reading prefabs from disk, once, and again on a reload.

use voltra_render::glam::Vec2;
use voltra_testkit::scratch_root;

use super::*;

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a valid asset path")
}

fn write_ship(root: &std::path::Path, name: &str) {
    let mut world = World::new();
    named(&mut world, "hull", Vec2::ZERO);
    crate::format::save(&world, &registry(), &root.join(name)).expect("the fixture writes");
}

#[test]
fn a_file_on_disk_becomes_a_prefab_once() {
    let root = scratch_root();
    write_ship(&root, "ship.ron");
    let mut prefabs = Prefabs::new(&root);

    let first = prefabs.load(&path("ship.ron"));
    let before = prefabs.len();
    let second = prefabs.load(&path("ship.ron"));

    assert_eq!(first, second);
    assert_eq!(prefabs.len(), before);
    assert_eq!(prefabs.get(first).len(), 1);
}

#[test]
fn a_prefab_that_is_not_there_spawns_nothing_rather_than_panicking() {
    let mut prefabs = Prefabs::new(scratch_root());

    let handle = prefabs.load(&path("nowhere.ron"));

    assert!(prefabs.get(handle).is_empty());
    assert_ne!(
        handle,
        prefabs.empty(),
        "and it gets its own slot to repair"
    );
}

#[test]
fn a_reload_swaps_the_prefab_under_the_same_handle() {
    let root = scratch_root();
    std::fs::write(root.join("late.ron"), "not a scene").expect("the fixture writes");
    let mut prefabs = Prefabs::new(&root);
    let handle = prefabs.load(&path("late.ron"));
    assert!(prefabs.get(handle).is_empty());

    write_ship(&root, "late.ron");
    assert!(prefabs.reload(&path("late.ron")));

    assert_eq!(prefabs.get(handle).len(), 1);
}

#[test]
fn a_reload_that_will_not_read_keeps_the_previous_prefab() {
    let root = scratch_root();
    write_ship(&root, "ship.ron");
    let mut prefabs = Prefabs::new(&root);
    let handle = prefabs.load(&path("ship.ron"));

    std::fs::write(root.join("ship.ron"), "(version: 1, entit").expect("the truncation");

    assert!(!prefabs.reload(&path("ship.ron")));
    assert_eq!(prefabs.get(handle).len(), 1);
}

#[test]
fn reloading_a_path_nobody_loaded_does_nothing() {
    let root = scratch_root();
    write_ship(&root, "ship.ron");
    let mut prefabs = Prefabs::new(&root);

    assert!(!prefabs.reload(&path("ship.ron")));
    assert_eq!(prefabs.len(), 1);
}
