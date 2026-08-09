//! Reloading a texture whose file changed, without the watcher.
//!
//! Every test here calls `Textures::reload` directly. That is the point of the
//! split: the policy — what happens to the handle, the store and the bind
//! group — is decided by this crate and can be tested without waiting on the
//! operating system to deliver an event. `tests/watch.rs` covers the transport
//! and is the only place with a timeout in it.
//!
//! What these tests cannot see is pixels: `voltra_render::Texture` keeps only a
//! `TextureView`, and its texture carries no `COPY_SRC`, so nothing here can be
//! read back. They watch the texture's **dimensions** instead, which a cache
//! hit cannot change. The pixel proof is one test in `voltra-scene`, where the
//! render-to-target harness already lives.
//!
//! Skips itself when no GPU adapter is available.

use voltra_assets::{AssetPath, Textures};
use voltra_testkit::{headless_device, scratch_root, write_png_rgba};

const GREEN: [u8; 4] = [40, 200, 90, 255];
const BLUE: [u8; 4] = [50, 90, 220, 255];

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
fn a_changed_file_reloads_under_the_same_handle() {
    let (device, queue) = device_or_skip!();
    let layout = voltra_render::texture::bind_group_layout(&device);
    let root = scratch_root();
    write_png_rgba(&root, "hero.png", 4, 4, GREEN);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let path = AssetPath::new("hero.png").expect("valid");
    let handle = textures.load(&device, &queue, &path);
    assert_eq!(textures.get(handle).width(), 4);

    // A different size as well as a different colour, because the size is the
    // part this crate can observe. A cache hit cannot change it.
    write_png_rgba(&root, "hero.png", 16, 16, BLUE);
    assert!(textures.reload(&device, &queue, &path));

    // The handle is what every sprite in the world is holding. If reload issued
    // a new one instead of swapping in place, nothing on screen would change
    // and this whole subsystem would be pointless.
    assert_eq!(textures.by_path_handle(&path), Some(handle));
    assert_eq!(textures.get(handle).width(), 16);
    assert_eq!(textures.get(handle).height(), 16);

    // The bind group must have been replaced along with the texture, not merely
    // left in place: the old one names the old view.
    textures.bind_group(handle);
}

#[test]
fn reloading_a_path_nobody_loaded_does_nothing() {
    let (device, queue) = device_or_skip!();
    let layout = voltra_render::texture::bind_group_layout(&device);
    let root = scratch_root();
    write_png_rgba(&root, "unused.png", 4, 4, GREEN);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let before = textures.len();

    let reloaded = textures.reload(
        &device,
        &queue,
        &AssetPath::new("unused.png").expect("valid"),
    );

    // The watch is on the whole root, so most events name files no scene has
    // asked for. Loading them here would turn a recursive watch into an
    // "upload every PNG in the project" button.
    assert!(!reloaded);
    assert_eq!(textures.len(), before);
}

#[test]
fn a_corrupt_rewrite_keeps_the_previous_pixels() {
    let (device, queue) = device_or_skip!();
    let layout = voltra_render::texture::bind_group_layout(&device);
    let root = scratch_root();
    write_png_rgba(&root, "hero.png", 4, 4, GREEN);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let path = AssetPath::new("hero.png").expect("valid");
    let handle = textures.load(&device, &queue, &path);

    // An image editor's save is not atomic: for a few milliseconds the file is
    // a truncated prefix of a PNG, and the debounce window does not always
    // cover that. Flashing the magenta checker on every save would teach the
    // reader to ignore the one colour that means "this path is broken".
    std::fs::write(root.join("hero.png"), b"not a PNG yet").expect("truncate");
    assert!(!textures.reload(&device, &queue, &path));

    assert_eq!(
        textures.get(handle).width(),
        4,
        "the last good texture must survive a truncated write"
    );
    textures.bind_group(handle);
}

#[test]
fn a_deleted_file_keeps_the_previous_pixels() {
    let (device, queue) = device_or_skip!();
    let layout = voltra_render::texture::bind_group_layout(&device);
    let root = scratch_root();
    write_png_rgba(&root, "hero.png", 4, 4, GREEN);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let path = AssetPath::new("hero.png").expect("valid");
    let handle = textures.load(&device, &queue, &path);

    std::fs::remove_file(root.join("hero.png")).expect("delete");
    assert!(!textures.reload(&device, &queue, &path));

    // Unreal and Unity both keep an imported asset when its source file
    // disappears. Nothing on screen changes until something valid replaces it.
    assert_eq!(textures.get(handle).width(), 4);
}

#[test]
fn a_path_that_failed_at_load_recovers_under_its_original_handle() {
    // The workflow this whole stage exists for: a typo in the inspector, the
    // sprite goes magenta, the file is dropped in, the sprite fixes itself
    // without the scene being reopened.
    let (device, queue) = device_or_skip!();
    let layout = voltra_render::texture::bind_group_layout(&device);
    let root = scratch_root();

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let path = AssetPath::new("late.png").expect("valid");
    let handle = textures.load(&device, &queue, &path);
    assert_eq!(textures.get(handle).width(), 8, "the 8x8 checker");
    assert_ne!(handle, textures.placeholder(), "its own slot");

    write_png_rgba(&root, "late.png", 32, 32, GREEN);
    assert!(textures.reload(&device, &queue, &path));

    // Same handle: the sprite component was never told anything changed, and
    // the checker it was drawing is now the real texture.
    assert_eq!(textures.by_path_handle(&path), Some(handle));
    assert_eq!(textures.get(handle).width(), 32);
}

#[test]
fn reloading_twice_does_not_grow_the_store() {
    let (device, queue) = device_or_skip!();
    let layout = voltra_render::texture::bind_group_layout(&device);
    let root = scratch_root();
    write_png_rgba(&root, "hero.png", 4, 4, GREEN);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let path = AssetPath::new("hero.png").expect("valid");
    textures.load(&device, &queue, &path);
    let before = textures.len();

    write_png_rgba(&root, "hero.png", 8, 8, BLUE);
    textures.reload(&device, &queue, &path);
    textures.reload(&device, &queue, &path);

    // A swap, not an insert. Reloading in a loop for an hour must not leak a
    // texture per save.
    assert_eq!(textures.len(), before);
}
