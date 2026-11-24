//! The world's side of prefab mode: the scene set aside, the prefab read
//! in its place, and the scene put back with its instances following the
//! prefab as it was saved.
//!
//! Pure world work over the registry and the prefab store — no frame, no
//! GPU — so every step is testable on its own; [`super`] strings them
//! together with the editor's state.

use std::path::Path;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};
use voltra_scene::format::{from_scene_file, to_scene_file, SceneFile};
use voltra_scene::prefab::{self, PrefabInstance, Prefabs, SyncReport};
use voltra_scene::{ComponentRegistry, SceneId};

/// Takes the scene out of the world, answering it as a file to put back.
/// Nothing is despawned unless it was written: a scene that cannot be
/// recorded stays where it is.
pub(super) fn set_aside(
    world: &mut World,
    registry: &ComponentRegistry,
) -> Result<SceneFile, String> {
    let scene = to_scene_file(world, registry)
        .map_err(|e| format!("could not set the scene aside: {e}"))?;
    despawn_scene(world);
    Ok(scene)
}

/// Reads the prefab at `file` into the emptied world, and brings the
/// prefabs nested in it in step with their own files. A file that will not
/// read leaves the world empty, for the caller to put the scene back.
pub(super) fn open_prefab(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    file: &Path,
) -> Result<(), String> {
    voltra_scene::format::load(file, registry, world).map_err(|e| e.to_string())?;
    prefab::sync_all(world, registry, prefabs);
    Ok(())
}

/// Puts `scene` back in place of whatever the world holds.
pub(super) fn put_back(world: &mut World, registry: &ComponentRegistry, scene: &SceneFile) {
    despawn_scene(world);
    if let Err(e) = from_scene_file(scene, registry, world) {
        // Written by this build, with this registry, in this session: no
        // version or component can trip it. Loud all the same, since the
        // scene the author came back to is not all there.
        log::error!("could not put the scene back after editing a prefab: {e}");
    }
}

/// Reads `edited` again and brings every instance in the world in step —
/// with it, and with any prefab saved on the way (one nested in it, edited
/// in a prefab mode of its own).
pub(super) fn follow(
    world: &mut World,
    registry: &ComponentRegistry,
    prefabs: &mut Prefabs,
    edited: &AssetPath,
) -> SyncReport {
    prefabs.reload(edited);
    prefab::sync_all(world, registry, prefabs)
}

/// The prefab, through the ones nested in the world, that writing the
/// world to `target` would make contain itself — Unity's and Godot's
/// refusal at save, made here for a prefab saved from its own mode, where
/// dropping the prefab into itself is one drag away.
pub(crate) fn cycle_through(
    world: &World,
    prefabs: &mut Prefabs,
    target: &AssetPath,
) -> Option<AssetPath> {
    let sources: Vec<AssetPath> = world
        .query::<PrefabInstance>()
        .map(|(_, link)| link.source.clone())
        .collect();
    prefab::reaches(prefabs, sources, target)
}

/// Every entity that is part of the scene, by the rule save and play use.
fn despawn_scene(world: &mut World) {
    let scene: Vec<Entity> = world.query::<SceneId>().map(|(e, _)| e).collect();
    for entity in scene {
        world.despawn(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{Name, Sprite, Transform};

    fn scratch() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "voltra-prefab-mode-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("prefabs")).expect("a folder");
        root
    }

    fn sprite_of(world: &World, name: &str) -> [f32; 4] {
        world
            .query::<Name>()
            .find(|(_, n)| n.as_str() == name)
            .and_then(|(e, _)| world.get::<Sprite>(e))
            .expect("the sprite")
            .color
    }

    /// A world holding one red coin, written out as a prefab and linked to
    /// it, and a second instance overriding its colour.
    fn scene(
        root: &Path,
        prefabs: &mut Prefabs,
        registry: &ComponentRegistry,
    ) -> (World, AssetPath) {
        let path = AssetPath::new("prefabs/coin.prefab.ron").expect("a path");
        let mut world = World::new();
        let coin = world.spawn();
        world.insert(coin, SceneId::new());
        world.insert(coin, Name::new("Coin"));
        world.insert(coin, Transform::default());
        world.insert(coin, Sprite::new([1.0, 0.0, 0.0, 1.0]));
        prefab::write(&world, registry, prefabs, coin, &path).expect("written");
        prefab::link(&mut world, coin, path.clone());
        assert!(root.join(path.as_str()).is_file());

        let handle = prefabs.load(&path);
        let at = Transform::from_translation(voltra_render::glam::Vec2::new(3.0, 0.0));
        let other = prefab::spawn(&mut world, registry, prefabs.get(handle), &at)
            .expect("spawned")
            .roots[0];
        world.insert(other, Name::new("Blue coin"));
        world.insert(other, Sprite::new([0.0, 0.0, 1.0, 1.0]));
        prefab::refresh_overrides(&mut world, registry, prefabs, other).expect("compared");
        (world, path)
    }

    #[test]
    fn a_prefab_saved_in_its_mode_reaches_the_scene_it_was_opened_from() {
        let root = scratch();
        let registry = ComponentRegistry::with_defaults();
        let mut prefabs = Prefabs::new(&root);
        let (mut world, path) = scene(&root, &mut prefabs, &registry);
        let before = world.query::<SceneId>().count();

        let parked = set_aside(&mut world, &registry).expect("set aside");
        assert_eq!(world.query::<SceneId>().count(), 0);
        let file = root.join(path.as_str());
        open_prefab(&mut world, &registry, &mut prefabs, &file).expect("opened");
        assert_eq!(world.query::<SceneId>().count(), 1, "the prefab alone");

        // Edited and saved as the prefab, as Save does in prefab mode.
        let (coin, _) = world.query::<Name>().next().expect("the coin");
        world.insert(coin, Sprite::new([0.0, 1.0, 0.0, 1.0]));
        assert_eq!(cycle_through(&world, &mut prefabs, &path), None);
        voltra_scene::format::save(&world, &registry, &file).expect("saved");

        put_back(&mut world, &registry, &parked);
        let report = follow(&mut world, &registry, &mut prefabs, &path);

        assert_eq!(world.query::<SceneId>().count(), before);
        assert_eq!(sprite_of(&world, "Coin"), [0.0, 1.0, 0.0, 1.0], "followed");
        assert_eq!(
            sprite_of(&world, "Blue coin"),
            [0.0, 0.0, 1.0, 1.0],
            "its override kept"
        );
        assert_eq!(report.changed, 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_prefab_holding_itself_is_found_before_it_is_saved() {
        let root = scratch();
        let registry = ComponentRegistry::with_defaults();
        let mut prefabs = Prefabs::new(&root);
        let (mut world, path) = scene(&root, &mut prefabs, &registry);
        set_aside(&mut world, &registry).expect("set aside");
        open_prefab(
            &mut world,
            &registry,
            &mut prefabs,
            &root.join(path.as_str()),
        )
        .expect("opened");

        // The prefab dropped into its own mode.
        let handle = prefabs.load(&path);
        prefab::spawn(
            &mut world,
            &registry,
            prefabs.get(handle),
            &Transform::default(),
        )
        .expect("spawned");

        assert_eq!(cycle_through(&world, &mut prefabs, &path), Some(path));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_prefab_that_will_not_read_leaves_the_world_for_the_scene_to_come_back() {
        let root = scratch();
        let registry = ComponentRegistry::with_defaults();
        let mut prefabs = Prefabs::new(&root);
        let (mut world, _) = scene(&root, &mut prefabs, &registry);
        let before = world.query::<SceneId>().count();
        std::fs::write(root.join("prefabs/broken.prefab.ron"), "(not a scene").expect("written");

        let parked = set_aside(&mut world, &registry).expect("set aside");
        let opened = open_prefab(
            &mut world,
            &registry,
            &mut prefabs,
            &root.join("prefabs/broken.prefab.ron"),
        );
        assert!(opened.is_err());
        put_back(&mut world, &registry, &parked);

        assert_eq!(world.query::<SceneId>().count(), before);
        let _ = std::fs::remove_dir_all(&root);
    }
}
