//! Reading the world back in through another registry: what loading and
//! unloading a game does to the scene.

use voltra_ecs::World;
use voltra_scene::format::{from_scene_file, to_scene_file};
use voltra_scene::{ComponentRegistry, SceneError};

/// Writes `world` out through `from`, empties it — entities and storages —
/// and reads it back in through `to`.
///
/// Loading a game: the scene's `Coin`s, kept as unknown components while no
/// game knew the name, become the game's own type. Unloading one: its
/// components go back to being kept unknown, and no storage the game's
/// code made is left in the world.
///
/// A scene `to` cannot read — a component of the game's whose saved value
/// its type no longer accepts — is read back through `from` instead, which
/// wrote it, and the error is answered: the world is as it was.
///
/// Entities with no `SceneId` are transient by the same rule play's
/// snapshot uses, and do not survive.
pub(crate) fn rebuild(
    world: &mut World,
    from: &ComponentRegistry,
    to: &ComponentRegistry,
) -> Result<(), SceneError> {
    let scene = to_scene_file(world, from)?;
    world.clear();
    if let Err(error) = from_scene_file(&scene, to, world) {
        world.clear();
        from_scene_file(&scene, from, world)?;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{SceneId, Transform, UnknownComponents};

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Coin {
        value: u32,
    }

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Strict(u8);

    fn with_coin() -> ComponentRegistry {
        let mut registry = ComponentRegistry::with_defaults();
        registry.register::<Coin>("Coin");
        registry
    }

    #[test]
    fn a_game_component_becomes_unknown_and_back() {
        let game = with_coin();
        let plain = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let coin = world.spawn();
        world.insert(coin, SceneId::new());
        world.insert(coin, Transform::default());
        world.insert(coin, Coin { value: 3 });

        rebuild(&mut world, &game, &plain).expect("unloads");
        let (entity, _) = world.query::<SceneId>().next().expect("still there");
        assert_eq!(
            world.component_count::<Coin>(),
            0,
            "no storage of the game's type"
        );
        assert!(
            world.get::<UnknownComponents>(entity).is_some(),
            "kept as text"
        );
        assert!(world.get::<Transform>(entity).is_some());

        rebuild(&mut world, &plain, &game).expect("loads");
        let (entity, _) = world.query::<SceneId>().next().expect("still there");
        assert_eq!(world.get::<Coin>(entity), Some(&Coin { value: 3 }));
    }

    #[test]
    fn a_scene_the_game_cannot_read_is_left_as_it_was() {
        let plain = ComponentRegistry::with_defaults();
        let mut strict = ComponentRegistry::with_defaults();
        strict.register::<Strict>("Coin");
        let mut world = World::new();
        let coin = world.spawn();
        world.insert(coin, SceneId::new());
        world.insert(coin, Coin { value: 3 });
        rebuild(&mut world, &with_coin(), &plain).expect("unloads");

        let error = rebuild(&mut world, &plain, &strict).expect_err("`(value: 3)` is no Strict");

        assert!(error.to_string().contains("Coin"), "{error}");
        let (entity, _) = world.query::<SceneId>().next().expect("still there");
        assert!(
            world.get::<UnknownComponents>(entity).is_some(),
            "kept as it was"
        );
    }
}
