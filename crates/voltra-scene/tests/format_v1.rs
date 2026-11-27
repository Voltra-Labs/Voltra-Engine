//! Files written before format version 2 still load, into what their
//! version 2 twins load into: `tests/fixtures/v1` is 0.4's spelling — a
//! scene with every renamed component, instances overriding them, a
//! variant, a clip with the old property names — and `tests/fixtures/v2`
//! is what `cargo xtask migrate` (and an editor save) writes from it.

use std::path::{Path, PathBuf};

use voltra_assets::AssetPath;
use voltra_ecs::World;
use voltra_scene::format::{self, to_scene_file, to_text, SceneError, SceneFile, VERSION};
use voltra_scene::prefab::{sync_all, Prefabs};
use voltra_scene::{
    AnimationClip, Camera2D, Collider2D, ComponentRegistry, PrefabPart, RigidBody2D, SceneId,
    Sprite2D, Transform2D, UnknownComponents,
};

fn fixtures(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(version)
}

/// `path` under `version`'s fixtures loaded into a world of its own, as a
/// scene opens: read, migrated, spawned.
fn loaded(version: &str, path: &str) -> World {
    let mut world = World::new();
    format::load(
        &fixtures(version).join(path),
        &ComponentRegistry::with_defaults(),
        &mut world,
    )
    .expect("the fixture loads");
    world
}

fn saved(world: &World) -> SceneFile {
    to_scene_file(world, &ComponentRegistry::with_defaults()).expect("saves")
}

/// The text after a file's leading comments: which version it is is said
/// there, and differs between twins.
fn body(text: &str) -> &str {
    let mut rest = text;
    while rest.starts_with("//") {
        rest = rest.split_once('\n').map_or("", |(_, after)| after);
    }
    rest
}

const SCENES: [&str; 3] = [
    "scene.ron",
    "prefabs/thing.prefab.ron",
    "prefabs/thing_variant.prefab.ron",
];

#[test]
fn a_version_one_file_loads_into_the_world_its_twin_does() {
    for path in SCENES {
        let old = loaded("v1", path);
        let new = loaded("v2", path);
        assert_eq!(saved(&old), saved(&new), "{path}");
        assert_eq!(saved(&old).version, VERSION);
    }
}

#[test]
fn every_renamed_component_arrives_as_its_new_type() {
    let world = loaded("v1", "scene.ron");
    assert_eq!(world.query::<Transform2D>().count(), 6);
    assert_eq!(world.query::<Camera2D>().count(), 1);
    assert_eq!(world.query::<Sprite2D>().count(), 5);
    assert_eq!(world.query::<RigidBody2D>().count(), 3);
    assert_eq!(world.query::<Collider2D>().count(), 3);
    // Nothing fell through to unknown but the one no build registers.
    let unknown: Vec<&str> = world
        .query::<UnknownComponents>()
        .flat_map(|(_, u)| u.0.keys().map(String::as_str))
        .collect();
    assert_eq!(unknown, ["Health"]);
}

#[test]
fn overrides_name_the_renamed_components() {
    let world = loaded("v1", "scene.ron");
    let mut overridden: Vec<String> = world
        .query::<PrefabPart>()
        .flat_map(|(_, part)| part.overridden.iter().cloned())
        .collect();
    overridden.sort();
    assert_eq!(overridden, ["Collider2D", "RigidBody2D", "Sprite2D"]);
    let variant = loaded("v1", "prefabs/thing_variant.prefab.ron");
    let mut overridden: Vec<String> = variant
        .query::<PrefabPart>()
        .flat_map(|(_, part)| part.overridden.iter().cloned())
        .collect();
    overridden.sort();
    assert_eq!(overridden, ["Collider2D", "Sprite2D", "Transform2D"]);
}

#[test]
fn a_save_of_a_version_one_file_is_its_twin_byte_for_byte() {
    for path in SCENES {
        let text = to_text(&saved(&loaded("v1", path))).expect("writes");
        let twin = std::fs::read_to_string(fixtures("v2").join(path)).expect("the twin");
        assert_eq!(body(&text), body(&twin), "{path}");
    }
}

#[test]
fn instances_sync_with_old_prefabs_as_with_new_ones() {
    // The scene's instances brought in step with their prefab and its
    // variant: read through the store, migrated as they are read.
    let synced = |version: &str| {
        let registry = ComponentRegistry::with_defaults();
        let mut world = loaded(version, "scene.ron");
        let mut prefabs = Prefabs::new(fixtures(version));
        sync_all(&mut world, &registry, &mut prefabs);
        let thing =
            prefabs.load(&AssetPath::new("prefabs/thing_variant.prefab.ron").expect("a path"));
        assert!(
            prefabs.get(thing).source().is_some() && !prefabs.get(thing).is_empty(),
            "the {version} variant reads"
        );
        saved(&world)
    };
    let old = synced("v1");
    assert_eq!(old, synced("v2"));
    // The instance overriding its sprite kept it; the one that did not has
    // the variant's.
    let sprite = |id: &str| {
        let id: SceneId = ron::from_str(&format!("\"{id}\"")).expect("an id");
        let record = old.entities.iter().find(|r| r.id == id).expect("there");
        record.components[Sprite2D::NAME]
            .into_rust::<Sprite2D>()
            .expect("a sprite")
            .color
    };
    assert_eq!(
        sprite("019fe900-0000-7000-8000-000000000003"),
        [0.0, 1.0, 0.0, 1.0]
    );
    assert_eq!(
        sprite("019fe900-0000-7000-8000-000000000005"),
        [0.25, 0.5, 1.0, 1.0]
    );
}

#[test]
fn the_staged_loader_migrates_as_the_store_does() {
    let root = fixtures("v1");
    let path = AssetPath::new("scene.ron").expect("a path");
    let mut staged = voltra_scene::prefab::StagedScene::read(&root, &path).expect("reads");
    let registry = ComponentRegistry::with_defaults();
    assert!(staged.check(&registry).is_ok());
    let mut world = World::new();
    while staged
        .spawn_next(&mut world, &registry)
        .expect("spawns")
        .is_some()
    {}
    assert_eq!(world.query::<Transform2D>().count(), 6);
}

#[test]
fn an_old_clip_reads_as_its_twin() {
    let read = |version: &str| -> AnimationClip {
        let text = std::fs::read_to_string(fixtures(version).join("animations/thing.anim.ron"))
            .expect("the clip");
        ron::from_str(&text).expect("reads")
    };
    let old = read("v1");
    assert_eq!(old, read("v2"));
    assert_eq!(old.tracks.len(), 7, "no track lost");
    let text = voltra_scene::ron_store::to_text(&old).expect("writes");
    let twin = std::fs::read_to_string(fixtures("v2").join("animations/thing.anim.ron"))
        .expect("the twin");
    assert_eq!(body(&text), body(&twin));
}

#[test]
fn a_newer_version_is_refused() {
    let text = std::fs::read_to_string(fixtures("v2").join("scene.ron")).expect("the twin");
    let v3 = text.replacen("version: 2", "version: 3", 1);
    assert!(matches!(
        format::parse(&v3),
        Err(SceneError::UnsupportedVersion {
            found: 3,
            supported: 2
        })
    ));
}
