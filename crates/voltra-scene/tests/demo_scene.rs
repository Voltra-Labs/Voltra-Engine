//! The sample content this repository ships, and the test that keeps it honest.
//!
//! Two tests with opposite jobs. `regenerate_the_demo_assets` writes
//! `assets/sprites/checker.png` and `assets/scenes/demo.ron`, and is
//! `#[ignore]`d because a test run must never rewrite the working tree. The
//! rest load what is committed and assert it still means what it meant — a
//! renamed component or a changed scene format would otherwise leave the demo
//! quietly broken until someone launched the editor.
//!
//! The scene file is generated rather than hand-written on purpose: it is
//! written by the same `save` path the editor's Save menu uses, so it cannot
//! drift from the format the loader expects.

use std::path::{Path, PathBuf};

use voltra_assets::AssetPath;
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::format::{load, save, VERSION};
use voltra_scene::{ComponentRegistry, SceneId, Sprite, Transform};

/// The repository's `assets` directory, from this crate's manifest.
fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("assets")
}

fn checker_png() -> PathBuf {
    assets_dir().join("sprites/checker.png")
}

fn demo_scene() -> PathBuf {
    assets_dir().join("scenes/demo.ron")
}

/// The world both the generator and the guard describe.
///
/// Three sprites: two naming one PNG so the shared-texture case is visible,
/// one with no texture at all so the tinted-white case is too. The two
/// textured ones are deliberately *not* adjacent in sort order, so the demo
/// also shows the batch splitting into three runs rather than two.
fn demo_world() -> World {
    let mut world = World::new();

    let checker = AssetPath::new("sprites/checker.png").expect("a valid asset path");

    let left = world.spawn();
    world.insert(left, SceneId::new());
    world.insert(left, Transform::from_translation(Vec2::new(-1.2, 0.0)));
    world.insert(
        left,
        Sprite {
            texture: Some(checker.clone()),
            ..Sprite::default().with_sort_order(0)
        },
    );

    let middle = world.spawn();
    world.insert(middle, SceneId::new());
    world.insert(
        middle,
        Transform::from_translation(Vec2::new(0.0, 0.0)).with_scale(Vec2::splat(1.5)),
    );
    world.insert(
        middle,
        Sprite {
            color: [1.0, 0.4, 0.2, 1.0],
            ..Sprite::default().with_sort_order(1)
        },
    );

    let right = world.spawn();
    world.insert(right, SceneId::new());
    world.insert(right, Transform::from_translation(Vec2::new(1.2, 0.0)));
    world.insert(
        right,
        Sprite {
            texture: Some(checker),
            ..Sprite::default().with_sort_order(2)
        },
    );

    world
}

/// Writes a 64x64 blue-and-white checker with 8-pixel cells.
///
/// Not magenta: magenta-and-black is the missing-texture signal, and a sample
/// asset that looks like a failure teaches the wrong thing. Hard-edged cells
/// because they make a wrong UV, a flipped V or a smeared filter obvious by
/// eye in the viewport.
fn write_checker(path: &Path) {
    use image::ImageEncoder;

    const SIZE: u32 = 64;
    const CELL: u32 = 8;
    const LIGHT: [u8; 4] = [236, 240, 245, 255];
    const BLUE: [u8; 4] = [56, 118, 214, 255];

    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dark = ((x / CELL) + (y / CELL)) % 2 == 1;
            pixels.extend_from_slice(if dark { &BLUE } else { &LIGHT });
        }
    }

    std::fs::create_dir_all(path.parent().expect("the PNG has a parent")).expect("sprites dir");
    let file = std::fs::File::create(path).expect("creating the checker PNG");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(&pixels, SIZE, SIZE, image::ExtendedColorType::Rgba8)
        .expect("encoding the checker PNG");
}

#[test]
#[ignore = "writes into the working tree; run with --ignored to regenerate"]
fn regenerate_the_demo_assets() {
    write_checker(&checker_png());

    let world = demo_world();
    let registry = ComponentRegistry::with_defaults();
    save(&world, &registry, &demo_scene()).expect("writing the demo scene");
}

#[test]
fn the_committed_demo_scene_loads() {
    let registry = ComponentRegistry::with_defaults();
    let mut world = World::new();
    load(&demo_scene(), &registry, &mut world).expect("the committed demo scene must load");

    let sprites: Vec<Sprite> = world
        .query::<Sprite>()
        .map(|(_, sprite)| sprite.clone())
        .collect();
    assert_eq!(sprites.len(), 3, "got {sprites:?}");

    let textured: Vec<&Sprite> = sprites.iter().filter(|s| s.texture.is_some()).collect();
    assert_eq!(textured.len(), 2, "two sprites must name the checker");
    assert_eq!(
        textured[0].texture, textured[1].texture,
        "both must name the *same* path, or the shared-texture case is not shown"
    );
    assert!(
        sprites.iter().any(|s| s.texture.is_none()),
        "one sprite must stay untextured so the tinted-white case is shown"
    );
    assert!(
        sprites.iter().all(|s| s.texture_handle.is_none()),
        "a handle must never come off disk"
    );
}

#[test]
fn the_committed_checker_is_a_readable_png() {
    let bytes = std::fs::read(checker_png()).expect("the checker must be committed");
    let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .expect("the committed checker must decode");
    assert_eq!((decoded.width(), decoded.height()), (64, 64));
}

#[test]
fn the_demo_scene_is_written_at_the_current_format_version() {
    let text = std::fs::read_to_string(demo_scene()).expect("the demo scene must be committed");
    assert!(
        text.contains(&format!("version: {VERSION}")),
        "the demo scene is stale against VERSION {VERSION}"
    );
}
