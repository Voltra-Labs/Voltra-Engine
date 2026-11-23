//! The whole of a hot reload, on a scratch game crate: built, played, a
//! constant in its tick edited, the change noticed, rebuilt, and the new
//! library swapped in mid-play with the world's state carried across — then
//! a broken edit that leaves the last good code running.
//!
//! Ignored by default: the scratch crate is built outside this workspace,
//! so the first run compiles the engine once more into its own target
//! folder — minutes, and gigabytes — before the test is a few seconds.
//! `cargo test -p voltra-platformer --test hot_reload -- --ignored`.
//!
//! The scratch crate is made a member of this workspace for the length
//! of the test — `games/scratch-game-<pid>`, removed after, with
//! `Cargo.lock` put back — because nothing else builds the engine's types
//! as this test's: cargo hashes a path dependency by its path *from the
//! workspace's root*, and a `TypeId` hashes that in.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use voltra_core::game::builder::{BuildOutcome, GameBuild};
use voltra_core::game::library::{library_file, GameLibrary};
use voltra_core::game::watch::SourceWatcher;
use voltra_core::{App, Headless};
use voltra_scene::SceneId;

const FRAME: f32 = 1.0 / 60.0;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace is there")
}

/// The scratch game: a `Mover` whose `distance` grows by `SPEED` each
/// fixed step.
fn source(speed: &str) -> String {
    format!(
        r#"use serde::{{Deserialize, Serialize}};
use voltra_core::{{Game, Tick}};
use voltra_scene::ComponentRegistry;

const SPEED: f32 = {speed};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mover {{
    pub distance: f32,
}}

#[derive(Default)]
pub struct Scratch;

impl Game for Scratch {{
    fn register(&self, registry: &mut ComponentRegistry) {{
        registry.register::<Mover>("Mover").with_template();
    }}
    fn fixed_update(&mut self, tick: &mut Tick<'_>) {{
        for (_, mover) in tick.world.query_mut::<Mover>() {{
            mover.distance += SPEED;
        }}
    }}
}}

voltra_core::export_game!(Scratch);
"#
    )
}

/// The scratch crate in this workspace's `games/`, while it lives: made by
/// [`Scratch::new`], and taken out again — with `Cargo.lock` as it was —
/// when it drops, a failed assertion included.
struct Scratch {
    dir: PathBuf,
    lock: Vec<u8>,
}

impl Scratch {
    fn new() -> Self {
        let root = workspace();
        let dir = root
            .join("games")
            .join(format!("scratch-game-{}", std::process::id()));
        let lock = std::fs::read(root.join("Cargo.lock")).expect("the lockfile");
        let manifest = r#"[package]
name = "scratch-game"
version.workspace = true
edition.workspace = true
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
voltra-core.workspace = true
voltra-scene.workspace = true
serde.workspace = true
"#;
        std::fs::create_dir_all(dir.join("src")).expect("the scratch folder");
        std::fs::write(dir.join("Cargo.toml"), manifest).expect("the manifest");
        std::fs::write(dir.join("src/lib.rs"), source("1.0")).expect("the source");
        Self { dir, lock }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::write(workspace().join("Cargo.lock"), &self.lock);
    }
}

/// Where the scratch crate builds: kept between runs, so only the first
/// compiles the engine.
fn target_dir() -> PathBuf {
    workspace().join("target").join("hot-reload-scratch")
}

/// Builds the scratch crate in this workspace, into its own target folder:
/// the test's own build holds the workspace's.
fn build() -> BuildOutcome {
    let target = target_dir().display().to_string();
    let args = [
        "build",
        "--package",
        "scratch-game",
        "--lib",
        "--target-dir",
        &target,
    ]
    .map(str::to_owned);
    let mut build = GameBuild::start(&workspace(), &args).expect("cargo starts");
    let start = Instant::now();
    loop {
        if let Some(outcome) = build.poll() {
            return outcome;
        }
        assert!(
            start.elapsed() < Duration::from_secs(3600),
            "the build never ended"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn library() -> PathBuf {
    target_dir()
        .join("debug")
        .join(library_file("scratch-game"))
}

fn distance(session: &Headless, id: SceneId) -> f32 {
    let ron = session.entity_ron(id).expect("the mover is there");
    let at = ron.find("distance:").expect("its distance") + "distance:".len();
    ron[at..]
        .trim_start()
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("a number in {ron}"))
}

fn waits_for_change(watcher: &mut SourceWatcher) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if watcher.changed() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
#[ignore = "builds a scratch game crate outside the workspace: minutes, the first time"]
fn editing_a_constant_reloads_the_game_mid_play_and_the_world_survives() {
    let scratch = Scratch::new();
    let dir = scratch.dir.clone();
    assert_eq!(build(), BuildOutcome::Built, "the first build");
    let first = GameLibrary::open(&library()).expect("the scratch library matches this build");

    let mut session = Headless::new(App::default().with_game(first.new_game()));
    let id = SceneId::new();
    let mover = session.world_mut().spawn();
    session.world_mut().insert(mover, id);
    session
        .set_component(id, "Mover", "(distance: 0.0)")
        .expect("a Mover");
    session.step(10, FRAME);
    let before = distance(&session, id);
    assert!((before - 10.0).abs() < 1e-3, "ten steps at 1: {before}");

    // The edit: noticed by the watch, rebuilt, swapped in mid-play.
    let mut watcher = SourceWatcher::new(&dir).expect("watching the scratch crate");
    std::fs::write(dir.join("src/lib.rs"), source("5.0")).expect("the edit");
    assert!(waits_for_change(&mut watcher), "the edit is seen");
    assert_eq!(build(), BuildOutcome::Built, "the edited build");
    let second = GameLibrary::open(&library()).expect("the rebuilt library");
    session
        .swap_game(&second)
        .expect("the world reads through the new code");
    // SAFETY: the swap rebuilt the world and registry through `second`.
    unsafe { first.close() };

    assert!(
        (distance(&session, id) - before).abs() < 1e-3,
        "the state survived the swap"
    );
    session.step(10, FRAME);
    let after = distance(&session, id);
    assert!(
        (after - (before + 50.0)).abs() < 1e-3,
        "ten steps at 5: {after}"
    );

    // A broken edit: the build fails, says why, and the last good code runs.
    std::fs::write(dir.join("src/lib.rs"), source("\"fast\"")).expect("the broken edit");
    assert!(waits_for_change(&mut watcher));
    let BuildOutcome::Failed(output) = build() else {
        panic!("a string is no f32");
    };
    let errors = voltra_core::game::builder::summary(&output, 4);
    assert!(errors.contains("error"), "{errors}");
    session.step(2, FRAME);
    assert!(
        (distance(&session, id) - (after + 10.0)).abs() < 1e-3,
        "still the code at 5"
    );
}
