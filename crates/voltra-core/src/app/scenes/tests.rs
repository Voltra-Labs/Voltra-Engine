use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, ComponentRegistry, Name, Persistent, SceneId, Transform2D};
use voltra_testkit::scratch_root;

use crate::app::App;

const FRAME: f32 = 1.0 / 60.0;

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a valid asset path")
}

/// Writes a scene of named entities at `root/name`.
fn write_scene(root: &Path, name: &str, entities: &[&str]) {
    let mut world = World::new();
    for entity_name in entities {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Name::new(*entity_name));
        world.insert(entity, Transform2D::default());
    }
    let file = voltra_scene::format::to_scene_file(&world, &ComponentRegistry::with_defaults())
        .expect("saves");
    let full = root.join(name);
    std::fs::create_dir_all(full.parent().expect("a parent")).expect("dir");
    std::fs::write(full, ron::to_string(&file).expect("serialises")).expect("writes");
}

fn root_with_levels() -> voltra_testkit::Scratch {
    let root = scratch_root();
    write_scene(&root, "scenes/level2.ron", &["ground", "goal"]);
    write_scene(&root, "scenes/overlay.ron", &["banner"]);
    root
}

fn named(world: &mut World, name: &str) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    entity
}

fn names(world: &World) -> Vec<String> {
    let mut names: Vec<String> = world
        .query::<Name>()
        .map(|(_, name)| name.as_str().to_owned())
        .collect();
    names.sort();
    names
}

/// An app whose tick runs `act` on its first frame only.
fn app_doing(root: &Path, act: impl Fn(&mut crate::Tick<'_>) + 'static) -> App {
    let done = Rc::new(Cell::new(false));
    App::default()
        .with_asset_root(root)
        .with_simulation()
        .with_update(move |tick| {
            if !done.replace(true) {
                act(tick);
            }
        })
}

#[test]
fn a_scene_change_replaces_every_root_with_the_new_scene() {
    let root = root_with_levels();
    let mut app = app_doing(&root, |tick| tick.load_scene(&path("scenes/level2.ron")));
    named(&mut app.world, "old");
    // Made in code with no id: a scene change must still take it away.
    let bare = app.world.spawn();
    app.world.insert(bare, Transform2D::default());

    app.advance(FRAME);

    assert_eq!(names(&app.world), vec!["goal", "ground"]);
    assert!(!app.world.is_alive(bare));
}

#[test]
fn the_change_waits_for_the_end_of_the_frame() {
    let root = root_with_levels();
    let seen = Rc::new(Cell::new(false));
    let saw = seen.clone();
    let old_slot: Rc<Cell<Option<Entity>>> = Rc::new(Cell::new(None));
    let old = old_slot.clone();
    let mut app = app_doing(&root, move |tick| {
        tick.load_scene(&path("scenes/level2.ron"));
        let entity = old.get().expect("set before the frame");
        saw.set(tick.world.is_alive(entity));
    });
    old_slot.set(Some(named(&mut app.world, "old")));

    app.advance(FRAME);

    assert!(seen.get(), "the tick that asked still had its world");
}

#[test]
fn a_persistent_root_and_its_children_survive() {
    let root = root_with_levels();
    let mut app = app_doing(&root, |tick| tick.load_scene(&path("scenes/level2.ron")));
    let player = named(&mut app.world, "player");
    app.world.insert(player, Persistent);
    let hat = named(&mut app.world, "hat");
    hierarchy::set_parent(&mut app.world, hat, player).expect("parents");
    let lone = named(&mut app.world, "lone");
    let kept_child = named(&mut app.world, "kept child");
    app.world.insert(kept_child, Persistent);
    hierarchy::set_parent(&mut app.world, kept_child, lone).expect("parents");

    app.advance(FRAME);

    assert_eq!(names(&app.world), vec!["goal", "ground", "hat", "player"]);
}

#[test]
fn a_scene_that_will_not_load_changes_nothing() {
    let root = root_with_levels();
    let mut app = app_doing(&root, |tick| tick.load_scene(&path("scenes/nowhere.ron")));
    named(&mut app.world, "old");

    app.advance(FRAME);

    assert_eq!(names(&app.world), vec!["old"]);
}

#[test]
fn an_added_scene_sits_beside_the_current_one_until_unloaded() {
    let root = root_with_levels();
    let load = Rc::new(Cell::new(None));
    let handle = load.clone();
    let frame = Rc::new(Cell::new(0));
    let mut app = App::default()
        .with_asset_root(&root)
        .with_simulation()
        .with_update(move |tick| {
            frame.set(frame.get() + 1);
            match frame.get() {
                1 => handle.set(Some(tick.add_scene(&path("scenes/overlay.ron")))),
                2 => tick.unload_scene(handle.get().expect("added on frame one")),
                _ => {}
            }
        });
    named(&mut app.world, "level");

    app.advance(FRAME);
    assert_eq!(names(&app.world), vec!["banner", "level"]);
    app.advance(FRAME);
    assert_eq!(names(&app.world), vec!["level"]);
    assert!(load.get().is_some());
}

#[test]
fn loading_the_same_scene_twice_gives_two_copies_with_their_own_ids() {
    let root = root_with_levels();
    let mut app = app_doing(&root, |tick| {
        tick.add_scene(&path("scenes/overlay.ron"));
        tick.add_scene(&path("scenes/overlay.ron"));
    });

    app.advance(FRAME);

    let ids: Vec<SceneId> = app.world.query::<SceneId>().map(|(_, id)| *id).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn a_scene_change_sweeps_additive_loads_and_their_handles() {
    let root = root_with_levels();
    let frame = Rc::new(Cell::new(0));
    let load = Rc::new(Cell::new(None));
    let handle = load.clone();
    let mut app = App::default()
        .with_asset_root(&root)
        .with_simulation()
        .with_update(move |tick| {
            frame.set(frame.get() + 1);
            match frame.get() {
                1 => handle.set(Some(tick.add_scene(&path("scenes/overlay.ron")))),
                2 => tick.load_scene(&path("scenes/level2.ron")),
                3 => tick.unload_scene(handle.get().expect("added")),
                _ => {}
            }
        });

    for _ in 0..3 {
        app.advance(FRAME);
    }

    assert_eq!(names(&app.world), vec!["goal", "ground"]);
}

#[test]
fn the_platformer_s_second_level_loads_with_every_component_known() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/scenes/platformer-2.ron");
    let mut world = World::new();
    voltra_scene::format::load(&path, &ComponentRegistry::with_defaults(), &mut world)
        .expect("the shipped level loads");
    assert!(world.entity_count() >= 4);
    assert!(
        world
            .query::<voltra_scene::UnknownComponents>()
            .next()
            .is_none(),
        "nothing in it is unregistered"
    );
}

#[test]
fn a_scene_saved_with_an_older_copy_of_a_prefab_arrives_in_step() {
    use voltra_scene::{PrefabInstance, PrefabPart, Sprite2D};
    let root = scratch_root();
    let registry = ComponentRegistry::with_defaults();
    let write = |name: &str, world: &World| {
        let file = voltra_scene::format::to_scene_file(world, &registry).expect("saves");
        let full = root.join(name);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("dir");
        std::fs::write(full, ron::to_string(&file).expect("serialises")).expect("writes");
    };
    // The prefab as it is now: a blue coin.
    let mut prefab = World::new();
    let coin = named(&mut prefab, "coin");
    prefab.insert(coin, Sprite2D::new([0.0, 0.0, 1.0, 1.0]));
    let record = *prefab.get::<SceneId>(coin).expect("an id");
    write("prefabs/coin.ron", &prefab);
    // A level saved when it was red.
    let mut level = World::new();
    let copy = named(&mut level, "coin");
    level.insert(copy, Transform2D::default());
    level.insert(copy, Sprite2D::new([1.0, 0.0, 0.0, 1.0]));
    level.insert(copy, PrefabInstance::new(path("prefabs/coin.ron")));
    level.insert(copy, PrefabPart::new(record));
    write("scenes/level.ron", &level);

    let mut app = app_doing(&root, |tick| tick.load_scene(&path("scenes/level.ron")));
    app.advance(FRAME);

    let (_, sprite) = app.world.query::<Sprite2D>().next().expect("the coin");
    assert_eq!(sprite.color, [0.0, 0.0, 1.0, 1.0]);
}
