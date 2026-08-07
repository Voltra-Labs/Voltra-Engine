//! Loading and caching real PNGs against a real GPU device.
//!
//! The cache's whole promise is that two sprites naming one file share one GPU
//! texture, and that a path that cannot be loaded still yields something
//! drawable. Neither is provable without a device.
//!
//! Each test skips itself when no adapter is available so CI machines without
//! one still pass.

mod common;

use common::{headless_device, scratch_root, write_png};
use voltra_assets::{AssetPath, Textures};

macro_rules! device_or_skip {
    () => {
        match headless_device() {
            Some(pair) => pair,
            None => {
                eprintln!("no GPU adapter; skipping");
                return;
            }
        }
    };
}

#[test]
fn the_same_path_twice_returns_the_same_handle() {
    let (device, queue) = device_or_skip!();
    let root = scratch_root();
    write_png(&root, "sprites/hero.png", 4, 4);

    let mut textures = Textures::new(&device, &queue, &root);
    let path = AssetPath::new("sprites/hero.png").expect("valid");

    let first = textures.load(&device, &queue, &path);
    let second = textures.load(&device, &queue, &path);

    assert_eq!(first, second, "one file must not upload twice");
    assert_ne!(first, textures.placeholder(), "it should have loaded");
    assert_eq!(textures.len(), 2, "the placeholder plus one texture");
}

#[test]
fn two_spellings_of_one_path_share_a_texture() {
    let (device, queue) = device_or_skip!();
    let root = scratch_root();
    write_png(&root, "sprites/hero.png", 4, 4);

    let mut textures = Textures::new(&device, &queue, &root);
    let plain = textures.load(
        &device,
        &queue,
        &AssetPath::new("sprites/hero.png").expect("valid"),
    );
    let dotted = textures.load(
        &device,
        &queue,
        &AssetPath::new(r".\sprites\hero.png").expect("valid"),
    );

    assert_eq!(plain, dotted);
    assert_eq!(textures.len(), 2);
}

#[test]
fn different_paths_return_different_handles() {
    let (device, queue) = device_or_skip!();
    let root = scratch_root();
    write_png(&root, "hero.png", 4, 4);
    write_png(&root, "villain.png", 8, 8);

    let mut textures = Textures::new(&device, &queue, &root);
    let hero = textures.load(&device, &queue, &AssetPath::new("hero.png").expect("valid"));
    let villain = textures.load(
        &device,
        &queue,
        &AssetPath::new("villain.png").expect("valid"),
    );

    assert_ne!(hero, villain);
    assert_eq!(textures.get(hero).width(), 4);
    assert_eq!(textures.get(villain).width(), 8);
}

#[test]
fn a_missing_file_yields_the_placeholder() {
    let (device, queue) = device_or_skip!();
    let root = scratch_root();

    let mut textures = Textures::new(&device, &queue, &root);
    let handle = textures.load(
        &device,
        &queue,
        &AssetPath::new("sprites/absent.png").expect("valid"),
    );

    assert_eq!(handle, textures.placeholder());
    assert_eq!(textures.len(), 1, "a failure must not store a texture");
}

#[test]
fn a_corrupt_png_yields_the_placeholder() {
    let (device, queue) = device_or_skip!();
    let root = scratch_root();
    std::fs::write(root.join("broken.png"), b"this is not a PNG").expect("seed file");

    let mut textures = Textures::new(&device, &queue, &root);
    let handle = textures.load(
        &device,
        &queue,
        &AssetPath::new("broken.png").expect("valid"),
    );

    assert_eq!(handle, textures.placeholder());
}

#[test]
fn a_failed_path_is_cached_rather_than_retried() {
    // Without this, a sprite with a broken path re-reads the disk and logs a
    // warning every frame it is drawn.
    let (device, queue) = device_or_skip!();
    let root = scratch_root();

    let mut textures = Textures::new(&device, &queue, &root);
    let path = AssetPath::new("absent.png").expect("valid");
    let first = textures.load(&device, &queue, &path);

    // Putting the file there afterwards must not change the answer: the miss
    // is now a cache entry like any other. 12c's hot reload is what will
    // invalidate it.
    write_png(&root, "absent.png", 4, 4);
    let second = textures.load(&device, &queue, &path);

    assert_eq!(first, second);
    assert_eq!(second, textures.placeholder());
}

#[test]
fn the_placeholder_is_an_eight_by_eight_texture() {
    let (device, queue) = device_or_skip!();
    let root = scratch_root();

    let textures = Textures::new(&device, &queue, &root);
    let placeholder = textures.get(textures.placeholder());

    assert_eq!(placeholder.width(), 8);
    assert_eq!(placeholder.height(), 8);
    assert_eq!(textures.len(), 1);
    assert!(!textures.is_empty());
}
