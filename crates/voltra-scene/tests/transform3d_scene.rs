//! A scene with 3D entities in it reads and writes back byte for byte:
//! `tests/fixtures/three_d/scene.ron` is what a save of the world built
//! here writes, a turned and scaled `Transform3D` under a parent among them.

use std::path::{Path, PathBuf};

use voltra_ecs::World;
use voltra_render::glam::{Quat, Vec3};
use voltra_scene::format::{self, to_scene_file, to_text};
use voltra_scene::hierarchy::{self, set_parent};
use voltra_scene::{ComponentRegistry, Name, SceneId, Transform3D};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/three_d/scene.ron")
}

/// A courtyard's start: a gate turned to face the player, a lamp on it.
fn built() -> World {
    let mut world = World::new();
    let gate = world.spawn();
    world.insert(
        gate,
        SceneId(
            "019fe500-0000-7000-8000-000000000301"
                .parse()
                .expect("a uuid"),
        ),
    );
    world.insert(gate, Name::new("Gate"));
    world.insert(
        gate,
        Transform3D::from_translation(Vec3::new(0.0, 0.0, -12.5))
            .with_rotation(Transform3D::from_euler_degrees(Vec3::new(0.0, 180.0, 0.0)).rotation)
            .with_scale(Vec3::new(2.0, 3.0, 0.25)),
    );
    let lamp = world.spawn();
    world.insert(
        lamp,
        SceneId(
            "019fe500-0000-7000-8000-000000000302"
                .parse()
                .expect("a uuid"),
        ),
    );
    world.insert(lamp, Name::new("Lamp"));
    world.insert(
        lamp,
        Transform3D::from_translation(Vec3::new(0.5, 2.0, 0.1))
            .with_rotation(Quat::from_rotation_x(-0.35)),
    );
    set_parent(&mut world, lamp, gate).expect("3D under 3D");
    world
}

fn text_of(world: &World) -> String {
    to_text(&to_scene_file(world, &ComponentRegistry::with_defaults()).expect("saves"))
        .expect("writes")
}

#[test]
fn a_3d_scene_round_trips_byte_for_byte() {
    let committed = std::fs::read_to_string(fixture())
        .expect("the fixture is committed")
        .replace("\r\n", "\n");
    assert_eq!(text_of(&built()), committed, "a save writes the fixture");

    let mut world = World::new();
    format::load(&fixture(), &ComponentRegistry::with_defaults(), &mut world).expect("loads");
    assert_eq!(text_of(&world), committed, "and reading it changes nothing");

    let lamp = world
        .query::<Name>()
        .find(|(_, name)| name.as_str() == "Lamp")
        .map(|(e, _)| e)
        .expect("the lamp");
    let at = hierarchy::world_matrix_3d(&world, lamp).transform_point3(Vec3::ZERO);
    // The gate's half turn takes the lamp's offset to (−1, 6, −0.025)
    // after its scale, then −12.5 along Z.
    assert!(
        (at - Vec3::new(-1.0, 6.0, -12.525)).length() < 1e-4,
        "{at:?}"
    );
}

#[test]
#[ignore = "writes into the working tree; run with --ignored to regenerate"]
fn regenerate_the_fixture() {
    std::fs::create_dir_all(fixture().parent().expect("a folder")).expect("made");
    format::save(&built(), &ComponentRegistry::with_defaults(), &fixture()).expect("written");
}
