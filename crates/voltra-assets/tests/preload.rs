//! Preloading is loading, sooner: the same handles in the same order, the
//! same placeholder for a broken path, and nothing loaded twice.
//!
//! Skips itself when no GPU adapter is available.

use voltra_assets::{AssetPath, Textures};
use voltra_jobs::JobPool;
use voltra_render::texture;
use voltra_testkit::{headless_device, scratch_root, write_png_rgba};

fn path(name: &str) -> AssetPath {
    AssetPath::new(name).expect("valid")
}

#[test]
fn preloaded_textures_are_what_loading_them_one_by_one_gives() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let root = scratch_root();
    for (i, name) in ["a.png", "b.png", "c.png"].iter().enumerate() {
        write_png_rgba(&root, name, 4 + i as u32, 2, [i as u8 * 80, 0, 0, 255]);
    }
    std::fs::write(root.join("broken.png"), b"not a png").expect("writes");
    let layout = texture::bind_group_layout(&device);
    let names = [
        "b.png",
        "a.png",
        "broken.png",
        "b.png",
        "c.png",
        "missing.png",
    ];

    let mut one_by_one = Textures::new(&device, &queue, &layout, &root);
    let looped: Vec<_> = names
        .iter()
        .map(|name| one_by_one.load(&device, &queue, &path(name)))
        .collect();

    let mut preloaded = Textures::new(&device, &queue, &layout, &root);
    let pool = JobPool::new(3);
    let loaded = preloaded.preload(&device, &queue, names.iter().map(|name| path(name)), &pool);
    assert_eq!(loaded, 5, "a path asked for twice is loaded once");
    let after: Vec<_> = names
        .iter()
        .map(|name| preloaded.load(&device, &queue, &path(name)))
        .collect();
    assert_eq!(after, looped, "same handles, same order");
    for (a, b) in looped.iter().zip(&after) {
        assert_eq!(one_by_one.get(*a).width(), preloaded.get(*b).width());
    }
    assert_eq!(preloaded.len(), one_by_one.len());

    // Everything is in: a second preload loads nothing.
    assert_eq!(
        preloaded.preload(&device, &queue, names.iter().map(|name| path(name)), &pool),
        0
    );
}

#[test]
fn a_render_texture_is_made_by_its_load_not_decoded_by_a_preload() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let root = scratch_root();
    write_png_rgba(&root, "a.png", 4, 2, [200, 0, 0, 255]);
    std::fs::write(
        root.join("map.rendertexture.ron"),
        "(version: 1, width: 32, height: 16)",
    )
    .expect("writes");
    let layout = texture::bind_group_layout(&device);
    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let pool = JobPool::new(2);
    // A scene with a sprite on an image and two on the render texture: the
    // preload decodes the image and leaves the render texture alone.
    let names = ["a.png", "map.rendertexture.ron", "map.rendertexture.ron"];
    let loaded = textures.preload(&device, &queue, names.iter().map(|name| path(name)), &pool);
    assert_eq!(loaded, 1);
    let map = textures.load(&device, &queue, &path("map.rendertexture.ron"));
    let targets = textures
        .render_texture(map)
        .expect("made as a render texture");
    assert_eq!(
        (targets.picture.width(), targets.picture.height()),
        (32, 16)
    );
}
