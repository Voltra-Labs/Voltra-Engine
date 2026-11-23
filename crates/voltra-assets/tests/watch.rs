//! The watcher transport: filesystem events in, `AssetPath`s out.
//!
//! The only tests in the workspace that wait on the operating system. They poll
//! with a bounded deadline rather than sleeping a fixed time, because how long
//! a platform takes to deliver an event is not a number this repository gets to
//! choose — and a fixed sleep is either flaky or slow, usually both.
//!
//! The reload *policy* is in `tests/hot_reload.rs` and needs none of this.

use std::path::Path;
use std::time::{Duration, Instant};

use voltra_assets::{AssetPath, AssetWatcher};
use voltra_testkit::{scratch_root, write_png};

/// How long to wait for an event before calling it a failure.
///
/// Generous: this bounds a broken watcher, it does not measure latency. A
/// working one answers in well under a second.
const DEADLINE: Duration = Duration::from_secs(10);

/// Polls `drain` until `path` shows up, or the deadline passes.
fn wait_for(watcher: &mut AssetWatcher, path: &AssetPath) -> bool {
    let start = Instant::now();
    while start.elapsed() < DEADLINE {
        if watcher.drain().contains(path) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Drains for a fixed window and returns everything seen.
///
/// For the negative cases, where the answer is "nothing arrives" and the only
/// way to be sure is to wait out the debounce and then some.
fn drain_for(watcher: &mut AssetWatcher, window: Duration) -> Vec<AssetPath> {
    let start = Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < window {
        seen.extend(watcher.drain());
        std::thread::sleep(Duration::from_millis(50));
    }
    seen
}

#[test]
fn a_rewritten_png_arrives_as_an_asset_path() {
    let root = scratch_root();
    write_png(&root, "sprites/hero.png", 4, 4);

    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");
    write_png(&root, "sprites/hero.png", 8, 8);

    let expected = AssetPath::new("sprites/hero.png").expect("valid");
    assert!(
        wait_for(&mut watcher, &expected),
        "no event for the rewritten PNG within {DEADLINE:?}"
    );
}

#[test]
fn a_new_png_in_a_new_subdirectory_arrives() {
    // The watch is recursive, and a directory created after it started must be
    // covered too — that is where an artist's new folder of sprites lands.
    let root = scratch_root();
    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");

    write_png(&root, "sprites/new/villain.png", 4, 4);

    let expected = AssetPath::new("sprites/new/villain.png").expect("valid");
    assert!(
        wait_for(&mut watcher, &expected),
        "no event for a PNG in a directory created after the watch started"
    );
}

#[test]
fn a_scene_save_is_seen_once_as_the_scene_and_never_as_its_temporary() {
    // `voltra-scene` writes `demo.ron.tmp` and renames it over `demo.ron`.
    // The scene is an asset since prefabs — any scene can be one — but the
    // temporary is not: its extension is `tmp`, so the filter drops it without
    // a rule about our own temporary files.
    let root = scratch_root();
    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");

    std::fs::write(root.join("demo.ron.tmp"), b"(version: 1)").expect("tmp");
    std::fs::rename(root.join("demo.ron.tmp"), root.join("demo.ron")).expect("rename");

    let seen = drain_for(&mut watcher, Duration::from_secs(2));
    let scene = AssetPath::new("demo.ron").expect("valid");
    // Deduplicated across drains as well as within one: the debouncer may
    // split the write and the rename across two batches.
    assert!(seen.contains(&scene), "the save was missed: {seen:?}");
    assert!(seen.iter().all(|path| *path == scene), "got {seen:?}");
}

#[test]
fn an_idle_watcher_drains_empty_without_blocking() {
    let root = scratch_root();
    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");

    let start = Instant::now();
    let drained = watcher.drain();

    assert!(drained.is_empty());
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "drain runs once per frame; it must never block"
    );
}

#[test]
fn a_root_that_does_not_exist_is_an_error_not_a_panic() {
    let missing = Path::new("voltra-no-such-root-anywhere");
    assert!(AssetWatcher::new(missing).is_err());
}

#[test]
fn a_rewritten_atlas_arrives_too() {
    // Re-cutting a sheet is an asset edit like repainting one, and the two
    // have to reach the editor the same way or slicing would need a restart.
    let root = scratch_root();
    std::fs::write(
        root.join("hero.atlas.ron"),
        b"(version: 1, grid: Some((cell: (16, 16), columns: 2, rows: 1)))",
    )
    .expect("the fixture writes");

    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");
    std::fs::write(
        root.join("hero.atlas.ron"),
        b"(version: 1, grid: Some((cell: (8, 16), columns: 4, rows: 1)))",
    )
    .expect("the re-cut writes");

    let expected = AssetPath::new("hero.atlas.ron").expect("valid");
    assert!(
        wait_for(&mut watcher, &expected),
        "no event for the rewritten atlas within {DEADLINE:?}"
    );
}

#[test]
fn project_files_arrive_too() {
    // The input map and the sorting layers are hot reloaded by the loop, so
    // the watch must report them; a filter that let only assets through kept
    // input-map edits from ever reaching a running game.
    let root = scratch_root();
    std::fs::write(root.join("default.input.ron"), "(version: 1)").expect("writes");
    std::fs::write(root.join("default.layers.ron"), "(version: 1, layers: [])").expect("writes");

    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");
    std::fs::write(root.join("default.input.ron"), "(version: 1, actions: {})").expect("writes");
    std::fs::write(
        root.join("default.layers.ron"),
        "(version: 1, layers: [\"A\"])",
    )
    .expect("writes");

    // Both, from one stream: they may well arrive in the same batch.
    let wanted: Vec<AssetPath> = ["default.input.ron", "default.layers.ron"]
        .into_iter()
        .map(|name| AssetPath::new(name).expect("valid"))
        .collect();
    let start = Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < DEADLINE && !wanted.iter().all(|w| seen.contains(w)) {
        seen.extend(watcher.drain());
        std::thread::sleep(Duration::from_millis(50));
    }
    for path in &wanted {
        assert!(
            seen.contains(path),
            "no event for {} within {DEADLINE:?}",
            path.as_str()
        );
    }
}

#[test]
fn reading_an_asset_is_not_a_change_to_it() {
    // inotify reports every open and close of a read: counted as changes,
    // the reload that reads the file raised another, for ever.
    let root = scratch_root();
    write_png(&root, "sprites/hero.png", 4, 4);
    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");
    // Whatever the write above raises, before the watch began or not.
    drain_for(&mut watcher, Duration::from_millis(600));

    for _ in 0..3 {
        std::fs::read(root.join("sprites/hero.png")).expect("readable");
    }

    let seen = drain_for(&mut watcher, Duration::from_millis(900));
    assert!(seen.is_empty(), "reads reported as changes: {seen:?}");
}
