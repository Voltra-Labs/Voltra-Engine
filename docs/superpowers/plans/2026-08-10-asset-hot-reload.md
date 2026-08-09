# Asset Hot Reload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Saving a PNG over one the editor already drew changes what is on
screen, without reopening the scene — stage 12c, the last third of the asset
pipeline.

**Architecture:** A `notify` debouncer on one recursive watch of the asset root
turns filesystem events into `AssetPath`s, in `voltra-assets::watch`. `Textures`
gains a `reload` that re-reads one path and swaps the new texture into the slot
its handle already names, replacing the bind group with it. `App` drains the
watcher once per frame and feeds `reload`. Because the handle never changes,
`Sprite`, `SpriteBatch`, the scene format and the renderer are untouched.

**Tech Stack:** Rust 2021, `wgpu` 30, `winit` 0.30, `egui` 0.35,
`notify-debouncer-full` 0.7 (which re-exports `notify` 8.2), `image` 0.25 (PNG
only), `pollster` 1.0.

Design: [`docs/superpowers/specs/2026-08-10-asset-hot-reload-design.md`](../specs/2026-08-10-asset-hot-reload-design.md).

## Global Constraints

Copied from `CLAUDE.md`, `docs/ARCHITECTURE.md` and `docs/CONVENTIONS.md`.
Every task's requirements implicitly include this section.

- The engine is **2D only**. No depth buffer, no z-axis, no 3D scaffolding.
- Only `voltra-core` may depend on `winit`. Only `voltra-render` may depend on
  `wgpu`. Everything else goes through `voltra_render::wgpu`.
- `voltra-render` must **not** depend on `voltra-assets`. The direction is
  `voltra-scene → voltra-assets → voltra-render`.
- All versions live in the root `[workspace.dependencies]`; member crates write
  `dep.workspace = true`. Never pin a version inside a member crate.
- **Depend on `notify-debouncer-full` only, never on `notify` directly.** The
  debouncer re-exports the `notify` it was built against, as
  `notify_debouncer_full::notify`. Two entries could drift apart on a
  `cargo update` and produce two incompatible copies, the same failure mode the
  repo already avoids with `voltra_render::wgpu`.
- No `unwrap()` outside tests. `expect("why this cannot fail")` when the
  invariant is real. Log through `log`, never `println!`.
- One concept per file. Split a module into a directory past roughly 300 lines
  or a second concept, `foo.rs` + `foo/`, never `foo/mod.rs`.
- Acceptance for every task: `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings` clean, then
  `cargo test --workspace` green. All three, every task, before the commit.
- Conventional Commits, scope = crate without the `voltra-` prefix, imperative
  subject ≤50 chars.
- **Subagents never rewrite history.** `git commit` and `git commit --amend` on
  your own last commit are allowed. `rebase`, `reset --hard`, `push --force`,
  `hash-object`, `write-tree`, `commit-tree`, `update-ref` are not. If an amend
  cannot be done with plain `git commit --amend`, stop and report.
- Branch: `feature/asset-hot-reload`. Do not push; the dispatching session does.

## Research this plan is built on

Read before deciding anything differs from what is written here.

- **Event kinds are not filtered on.** Bevy's
  [#10576](https://github.com/bevyengine/bevy/issues/10576) is
  "`file_watcher` does not reload assets on all file changes": which
  `EventKind` a save produces varies by platform and by the writing program —
  some emit `Create` for an overwrite, some `Modify(Data)`, some a rename pair.
  So any event on a path with a texture extension triggers a reload *attempt*,
  and `reload` deciding the file is unreadable is what handles the rest.
- **Windows extended-length paths.**
  [bevy#18342](https://github.com/bevyengine/bevy/issues/18342): a canonicalized
  path on Windows carries the `\\?\` prefix, the watched root did not, and
  `strip_prefix` panicked on every hot reload. Canonicalize both sides, and drop
  the event rather than panicking when it still will not relativize.
- **Keeping the last good asset on a failed reload** is Unreal's and Unity's
  behaviour: an already-imported asset survives its source file becoming
  unreadable or disappearing.
- **The toggle.** Bevy: a `file_watcher` cargo feature. Unreal: Editor
  Preferences → Loading & Saving → Auto Reimport, one checkbox. Unity: Auto
  Refresh in Asset Pipeline preferences. Godot:
  `filesystem/refresh_on_focus`. All four ship it switchable and none leaves it
  on in a build, which is why `with_hot_reload` is opt-in.

## File Structure

**Created**

- `crates/voltra-assets/src/watch.rs` — `AssetWatcher`: filesystem events to
  changed `AssetPath`s. Knows nothing of textures, handles or the GPU.
- `crates/voltra-assets/tests/hot_reload.rs` — the reload policy, with a GPU and
  no watcher, so nothing waits on the operating system.
- `crates/voltra-assets/tests/watch.rs` — the watcher transport, with no GPU.

**Modified**

- `crates/voltra-assets/src/textures.rs` — `reload`, and a private slot per
  failed path instead of the shared placeholder handle.
- `crates/voltra-assets/src/error.rs` — `AssetError::Watch`.
- `crates/voltra-assets/src/lib.rs` — `pub mod watch;` and the re-export.
- `crates/voltra-assets/tests/headless_textures.rs` — four assertions that pin
  the old shared-placeholder invariant.
- `crates/voltra-scene/tests/headless_sprite_textures.rs` — one more.
- `crates/voltra-testkit/src/lib.rs` — `write_png_rgba`, with `write_png`
  delegating to it.
- `crates/voltra-core/src/app.rs` — `hot_reload`, `with_hot_reload`, build in
  `resumed`, drain in `update`.
- `crates/voltra-editor/src/main.rs` — opts in.
- `Cargo.toml`, `crates/voltra-assets/Cargo.toml` — the debouncer.
- `docs/ARCHITECTURE.md`, `README.md` — decisions and the roadmap.

## Execution waves

Tasks inside a wave touch disjoint files and can be dispatched in parallel.
Waves are sequential.

| Wave | Tasks | Why they cannot move |
| --- | --- | --- |
| 1 | Task 1, Task 3 | Task 3 only touches `watch.rs`, `error.rs`, `lib.rs` and the manifests; Task 1 only `textures.rs` and two test files. |
| 2 | Task 2 | Needs Task 1's per-path slots for its recovery test, and edits the same file. |
| 3 | Task 4 | Needs `reload` (Task 2) and `AssetWatcher` (Task 3). |
| 4 | Task 5 | Documents what every earlier task decided. |

---

### Task 1: A failed path gets its own placeholder slot

Today `Textures::load` caches the **shared** placeholder handle for a path that
fails (`textures.rs:98`). With hot reload that is a dead end: a `Sprite` stores
the handle it was given and nothing re-resolves it, so fixing the typo and
dropping the PNG in place could never repair a sprite already on screen —
`reload` would have to overwrite the one texture that every other broken path is
also drawing.

Each failure gets its own slot holding the placeholder's pixels. 256 bytes and
one bind group per broken path, and `reload` can then swap it like any other.

**Files:**
- Modify: `crates/voltra-assets/src/textures.rs`
- Modify: `crates/voltra-assets/tests/headless_textures.rs` (four tests)
- Modify: `crates/voltra-scene/tests/headless_sprite_textures.rs` (one test)

**Interfaces:**
- Produces: no signature changes. `Textures::placeholder()` stays and still
  means "the checker the store was seeded with"; what changes is that a failed
  `load` no longer returns it.

- [x] **Step 1: Update the tests that pin the old invariant**

In `crates/voltra-assets/tests/headless_textures.rs`, replace
`a_missing_file_yields_the_placeholder` (lines 87–102) with:

```rust
#[test]
fn a_missing_file_yields_a_checker_of_its_own() {
    let (device, queue) = device_or_skip!();
    let layout = voltra_render::texture::bind_group_layout(&device);
    let root = scratch_root();

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let handle = textures.load(
        &device,
        &queue,
        &AssetPath::new("sprites/absent.png").expect("valid"),
    );

    // Its own slot, not the shared placeholder handle: a `Sprite` stores the
    // handle it was given and nothing re-resolves it, so sharing one would
    // make a broken path unfixable by hot reload.
    assert_ne!(handle, textures.placeholder());
    assert_eq!(textures.get(handle).width(), 8, "it is still the checker");
    assert_eq!(textures.len(), 2, "the placeholder plus this path's copy");
    textures.bind_group(handle);
}
```

Replace `a_corrupt_png_yields_the_placeholder`'s assertion (line 118) with:

```rust
    assert_ne!(handle, textures.placeholder());
    assert_eq!(textures.get(handle).width(), 8, "it is still the checker");
```

Replace `a_failed_path_is_cached_rather_than_retried`'s assertions (lines
139–140) with:

```rust
    assert_eq!(first, second);
    assert_ne!(second, textures.placeholder());
    assert_eq!(
        textures.len(),
        2,
        "the retry must not have added a second copy"
    );
```

In `crates/voltra-scene/tests/headless_sprite_textures.rs`, in
`a_path_that_does_not_load_draws_the_placeholder_checker`, replace the
`assert_eq!` on `missing.1.texture_handle` with:

```rust
    assert!(
        missing.1.texture_handle.is_some(),
        "a failed load must still resolve to something drawable"
    );
    assert_ne!(
        missing.1.texture_handle,
        Some(textures.placeholder()),
        "a failed path gets its own slot so hot reload can fix it in place"
    );
```

The pixel assertions further down that test already prove the contents are the
magenta-and-black checker; leave them alone.

- [x] **Step 2: Run them and watch them fail**

Run: `cargo test -p voltra-assets --test headless_textures`
Expected: FAIL on `a_missing_file_yields_a_checker_of_its_own` — `assert_ne!`
fires because the handle *is* still the placeholder.

On a machine with no GPU adapter every one of these prints "no GPU adapter;
skipping" and passes. That is not a pass: this task cannot be verified without
an adapter, so say so in the report rather than reporting green.

- [x] **Step 3: Factor the placeholder texture out**

In `crates/voltra-assets/src/textures.rs`, add this free function below the
`impl Textures` block:

```rust
/// The magenta-and-black checker, uploaded.
///
/// A free function because both the store's seed and every failed path need
/// one, and the second of those runs on a `&mut self` that already exists.
fn placeholder_texture(device: &Device, queue: &Queue) -> Texture {
    Texture::from_rgba8(
        device,
        queue,
        "missing-texture",
        &placeholder::rgba(),
        placeholder::SIZE,
        placeholder::SIZE,
        // Nearest so the checks stay hard-edged. Filtering them into a
        // magenta smear makes the failure look like a design choice.
        Filter::Nearest,
    )
    .expect("the placeholder's pixel count matches its declared size")
}
```

Then replace the body of `Textures::new` between `let mut store = Assets::new();`
and `let mut bind_groups = HashMap::new();` with:

```rust
        let mut store = Assets::new();
        let texture = placeholder_texture(device, queue);
        let bind_group = texture.create_bind_group(device, layout);
        let placeholder = store.insert(texture);
```

- [x] **Step 4: Give each failure its own slot**

Add this method to `impl Textures`, directly after `load`:

```rust
    /// A private texture holding the placeholder's pixels, under its own
    /// handle.
    ///
    /// Not the shared [`Self::placeholder`] handle. A [`Sprite`] stores the
    /// handle it was given and nothing re-resolves it, so if every broken path
    /// shared one handle, repairing one of them would mean overwriting the
    /// texture all the others are also drawing — and hot reload could never fix
    /// a typo. Its own slot costs 256 bytes of pixels.
    ///
    /// [`Sprite`]: https://docs.rs/voltra-scene
    fn insert_broken(&mut self, device: &Device, queue: &Queue) -> Handle<Texture> {
        let texture = placeholder_texture(device, queue);
        let bind_group = texture.create_bind_group(device, &self.layout);
        let handle = self.store.insert(texture);
        self.bind_groups.insert(handle, bind_group);
        handle
    }
```

Change `load`'s error arm (currently `textures.rs:96-100`) to:

```rust
            Err(e) => {
                log::warn!("{e}; drawing the missing-texture checker instead");
                self.insert_broken(device, queue)
            }
```

- [x] **Step 5: Run the tests**

Run: `cargo test -p voltra-assets --test headless_textures`
Expected: PASS, eight tests.

Run: `cargo test -p voltra-scene --test headless_sprite_textures`
Expected: PASS, four tests.

- [x] **Step 6: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [x] **Step 7: Commit**

```sh
git add crates/voltra-assets/src/textures.rs crates/voltra-assets/tests/headless_textures.rs crates/voltra-scene/tests/headless_sprite_textures.rs
git commit -m "fix(assets): give each failed path its own checker"
```

---

### Task 2: `Textures::reload` swaps pixels under a stable handle

**Files:**
- Modify: `crates/voltra-testkit/src/lib.rs` (`write_png_rgba`)
- Modify: `crates/voltra-assets/src/textures.rs` (`reload`)
- Create: `crates/voltra-assets/tests/hot_reload.rs`
- Modify: `crates/voltra-scene/tests/headless_sprite_textures.rs` (one new test)

**Interfaces:**
- Consumes: Task 1's `insert_broken`, and `Assets::get_mut` (`store.rs:58`).
- Produces:
  - `voltra_assets::Textures::reload(&mut self, &Device, &Queue, &AssetPath) -> bool`
  - `voltra_assets::Textures::by_path_handle(&self, &AssetPath) -> Option<Handle<Texture>>`
  - `voltra_testkit::write_png_rgba(root: &Path, name: &str, width: u32, height: u32, rgba: [u8; 4])`

**Where each assertion lives, and why it is split.** `voltra_render::Texture`
keeps only the `wgpu::TextureView`, and `from_rgba8` creates its texture with
`TEXTURE_BINDING | COPY_DST` and no `COPY_SRC` (`texture.rs:112`) — so a
loaded texture cannot be copied back to the CPU, and adding the usage flag to
every texture in the engine to satisfy a test would be the tail wagging the dog.

So `voltra-assets` proves what it can prove without pixels: the handle is
unchanged, the store did not grow, the bind group still resolves, and the
texture's **dimensions** changed, which no cache hit could produce. The pixel
proof goes in `crates/voltra-scene/tests/headless_sprite_textures.rs`, where the
render-to-target harness already exists — that file already renders a `Sprite`
through the real pipeline and reads the frame back.

- [x] **Step 1: Add the colour-carrying PNG writer to the testkit**

In `crates/voltra-testkit/src/lib.rs`, replace the body of `write_png` and add
the new function above it:

```rust
/// Writes a real PNG of `width` x `height` in one flat colour at `root/name`.
///
/// Tests that tell two textures apart by what comes back need their own
/// colours; [`write_png`] is the red-by-default case.
///
/// Encodes through `PngEncoder` rather than `DynamicImage::save_with_format`.
/// The workspace pins `image` with `default-features = false, features =
/// ["png"]`, and the convenience `save*` helpers sit behind feature gates that
/// set does not necessarily turn on; the encoder is exactly what the `png`
/// feature provides.
pub fn write_png_rgba(root: &Path, name: &str, width: u32, height: u32, rgba: [u8; 4]) {
    use image::ImageEncoder;

    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("asset subdirectory");
    }

    let pixels: Vec<u8> = (0..width * height).flat_map(|_| rgba).collect();

    let file = std::fs::File::create(&path).expect("creating the test PNG");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .expect("encoding the test PNG");
}

/// Writes a real PNG of `width` x `height` opaque red at `root/name`.
pub fn write_png(root: &Path, name: &str, width: u32, height: u32) {
    write_png_rgba(root, name, width, height, [255, 0, 0, 255]);
}
```

- [x] **Step 2: Write the failing tests**

Create `crates/voltra-assets/tests/hot_reload.rs`:

```rust
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

    // The handle is what every Sprite in the world is holding. If reload
    // issued a new one instead of swapping in place, nothing on screen would
    // change and this whole subsystem would be pointless.
    assert_eq!(textures.by_path_handle(&path), Some(handle));
    assert_eq!(textures.get(handle).width(), 16);
    assert_eq!(textures.get(handle).height(), 16);

    // The bind group must have been replaced along with the texture, not
    // merely left in place: the old one names the old view.
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
    assert_ne!(handle, textures.placeholder(), "Task 1's own-slot rule");

    write_png_rgba(&root, "late.png", 32, 32, GREEN);
    assert!(textures.reload(&device, &queue, &path));

    // Same handle: the Sprite component was never told anything changed, and
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
```

- [x] **Step 3: Run them and watch them fail to compile**

Run: `cargo test -p voltra-assets --test hot_reload`
Expected: FAIL — `no method named 'reload' found`, and
`no method named 'by_path_handle' found`.

Do **not** solve this by adding `COPY_SRC` to `Texture::from_rgba8` or by
exposing the `wgpu::Texture`. Every texture in the engine would carry a usage
flag it does not need so that one test could read it back, and the pixel
question is answered in Step 6 where the render harness already is.

- [x] **Step 4: Implement `reload` and the lookup the tests need**

In `crates/voltra-assets/src/textures.rs`, add both to `impl Textures`, after
`insert_broken`:

```rust
    /// Re-reads `path` and swaps the new pixels in under the handle it already
    /// has. Returns whether anything changed.
    ///
    /// The handle is deliberately stable: every `Sprite` in the world stores
    /// the one it was given, and the scene file stores the path. Issuing a new
    /// handle would mean walking the world to repoint them, and would make the
    /// scene dirty for a change nobody made to it.
    ///
    /// A path that was never loaded is ignored. The watch is on the whole
    /// asset root, so most events name files no scene has asked for; loading
    /// them here would upload every PNG in the project the first time one of
    /// them was touched.
    pub fn reload(&mut self, device: &Device, queue: &Queue, path: &AssetPath) -> bool {
        let Some(&handle) = self.by_path.get(path) else {
            return false;
        };

        let texture = match self.read(device, queue, path) {
            Ok(texture) => texture,
            Err(e) => {
                // The previous pixels stay on screen. An image editor's save
                // leaves the file truncated for a few milliseconds and the
                // debounce window does not always cover it, so degrading to
                // the checker here would flash magenta on every save.
                log::warn!("{e}; keeping the previously loaded texture");
                return false;
            }
        };

        let bind_group = texture.create_bind_group(device, &self.layout);
        let slot = self
            .store
            .get_mut(handle)
            .expect("Textures never removes, so every handle it issued resolves");
        *slot = texture;
        // The old bind group still names the old texture view, so replacing
        // the texture without replacing this swaps nothing the GPU can see.
        self.bind_groups.insert(handle, bind_group);

        log::info!("reloaded {}", path.as_str());
        true
    }

    /// The handle `path` is cached to, if it has ever been loaded.
    ///
    /// Distinct from [`Self::load`], which loads on a miss. This asks whether
    /// the cache already knows the path, which is the question hot reload has.
    pub fn by_path_handle(&self, path: &AssetPath) -> Option<Handle<Texture>> {
        self.by_path.get(path).copied()
    }
```

- [x] **Step 5: Run the tests**

Run: `cargo test -p voltra-assets --test hot_reload`
Expected: PASS, six tests.

If `a_path_that_failed_at_load_recovers_under_its_original_handle` fails on the
first assertion, Task 1 is not in the tree — that test needs the per-path
checker slot, because a shared placeholder handle cannot be overwritten without
changing every other broken path too.

- [x] **Step 6: Prove it in pixels, where the render harness lives**

Append to `crates/voltra-scene/tests/headless_sprite_textures.rs`. It already
has `render_batch`, `at`, `close_camera`, `sprite_at` and `write_flat_png`; use
them as they are.

```rust
#[test]
fn a_reloaded_texture_changes_what_is_drawn() {
    // The dimension assertions in voltra-assets prove the slot was replaced.
    // This proves the replacement reaches the screen — which it only does if
    // the bind group was replaced too, because the old one names the old view.
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();
    write_flat_png(&root, "swap.png", [40, 200, 90, 255]);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let sprite = sprite_at(0.0, Some("swap.png"), &mut textures, &device, &queue);

    let mut batch = SpriteBatch::default();
    batch.push(&sprite.0, &sprite.1);

    let before = render_batch(&device, &queue, &batch, &textures, &close_camera());
    let centre = at(&before, SIZE / 2, SIZE / 2);
    assert!(centre.g > centre.b, "the green PNG first: {centre:?}");

    write_flat_png(&root, "swap.png", [50, 90, 220, 255]);
    let path = AssetPath::new("swap.png").expect("a valid asset path");
    assert!(textures.reload(&device, &queue, &path));

    // The same batch, the same sprite, the same handle. Nothing in the world
    // was touched — only the texture behind the handle.
    let after = render_batch(&device, &queue, &batch, &textures, &close_camera());
    let centre = at(&after, SIZE / 2, SIZE / 2);
    assert!(
        centre.b > centre.g,
        "the reload must reach the screen: {centre:?}"
    );
}
```

Run: `cargo test -p voltra-scene --test headless_sprite_textures`
Expected: PASS, five tests.

If the frame is unchanged after the reload while `voltra-assets`' tests pass,
the bind group is not being replaced — that is the one line in `reload` after
the `*slot = texture;`.

- [x] **Step 7: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [x] **Step 8: Commit**

```sh
git add crates/voltra-assets/src/textures.rs crates/voltra-assets/tests/hot_reload.rs crates/voltra-testkit/src/lib.rs crates/voltra-scene/tests/headless_sprite_textures.rs
git commit -m "feat(assets): reload a texture under its handle"
```

---

### Task 3: The watcher

Disjoint from Tasks 1 and 2 — it touches no file they touch and knows nothing
about textures.

**Files:**
- Create: `crates/voltra-assets/src/watch.rs`
- Create: `crates/voltra-assets/tests/watch.rs`
- Modify: `crates/voltra-assets/src/error.rs`
- Modify: `crates/voltra-assets/src/lib.rs`
- Modify: `Cargo.toml`, `crates/voltra-assets/Cargo.toml`

**Interfaces:**
- Produces:
  - `voltra_assets::watch::AssetWatcher::new(root: &Path) -> Result<AssetWatcher, AssetError>`
  - `voltra_assets::watch::AssetWatcher::drain(&mut self) -> Vec<AssetPath>`
  - `voltra_assets::watch::TEXTURE_EXTENSIONS: &[&str]`
  - re-exported as `voltra_assets::AssetWatcher`
  - `AssetError::Watch { root: PathBuf, source: notify_debouncer_full::notify::Error }`

- [x] **Step 1: Add the dependency**

In the root `Cargo.toml`, in `[workspace.dependencies]`, alphabetically:

```toml
notify-debouncer-full = "0.7"
```

Only this one. It re-exports the `notify` 8.2 it was built against as
`notify_debouncer_full::notify`; a separate `notify` entry could drift to a
different major on a `cargo update` and give the build two incompatible copies.

In `crates/voltra-assets/Cargo.toml`, under `[dependencies]`:

```toml
notify-debouncer-full.workspace = true
```

- [x] **Step 2: Add the error variant**

In `crates/voltra-assets/src/error.rs`, add to the enum after `Decode`:

```rust
    /// The filesystem watch could not be established.
    Watch {
        root: PathBuf,
        source: notify_debouncer_full::notify::Error,
    },
```

Add to `Display`:

```rust
            Self::Watch { root, source } => {
                write!(f, "could not watch {}: {source}", root.display())
            }
```

Add to `source()`'s first match arm list:

```rust
            Self::Watch { source, .. } => Some(source),
```

and leave `Absolute | EscapesRoot | Empty` returning `None`.

Update the enum's doc comment, which currently says "The last two are runtime
failures": it is now "`Read`, `Decode` and `Watch` are runtime failures".

- [x] **Step 3: Write the failing tests**

Create `crates/voltra-assets/tests/watch.rs`:

```rust
//! The watcher transport: filesystem events in, `AssetPath`s out.
//!
//! The only tests in the workspace that wait on the operating system. They poll
//! with a bounded deadline rather than sleeping a fixed time, because how long
//! a platform takes to deliver an event is not a number this repository gets to
//! choose — and a fixed sleep is either flaky or slow, usually both.
//!
//! The reload *policy* is in `tests/hot_reload.rs` and needs none of this.

use std::path::Path;
use std::time::{Duration, Instant};

use voltra_assets::{AssetPath, AssetWatcher};
use voltra_testkit::{scratch_root, write_png};

/// How long to wait for an event before calling it a failure.
///
/// Generous: this bounds a broken watcher, it does not measure latency. A
/// working one answers in well under a second.
const DEADLINE: Duration = Duration::from_secs(10);

/// Polls `drain` until `path` shows up, or the deadline passes.
fn wait_for(watcher: &mut AssetWatcher, path: &AssetPath) -> bool {
    let start = Instant::now();
    while start.elapsed() < DEADLINE {
        if watcher.drain().contains(path) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Drains for a fixed window and returns everything seen.
///
/// For the negative cases, where the answer is "nothing arrives" and the only
/// way to be sure is to wait out the debounce and then some.
fn drain_for(watcher: &mut AssetWatcher, window: Duration) -> Vec<AssetPath> {
    let start = Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < window {
        seen.extend(watcher.drain());
        std::thread::sleep(Duration::from_millis(50));
    }
    seen
}

#[test]
fn a_rewritten_png_arrives_as_an_asset_path() {
    let root = scratch_root();
    write_png(&root, "sprites/hero.png", 4, 4);

    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");
    write_png(&root, "sprites/hero.png", 8, 8);

    let expected = AssetPath::new("sprites/hero.png").expect("valid");
    assert!(
        wait_for(&mut watcher, &expected),
        "no event for the rewritten PNG within {DEADLINE:?}"
    );
}

#[test]
fn a_new_png_in_a_new_subdirectory_arrives() {
    // The watch is recursive, and a directory created after it started must be
    // covered too — that is where an artist's new folder of sprites lands.
    let root = scratch_root();
    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");

    write_png(&root, "sprites/new/villain.png", 4, 4);

    let expected = AssetPath::new("sprites/new/villain.png").expect("valid");
    assert!(
        wait_for(&mut watcher, &expected),
        "no event for a PNG in a directory created after the watch started"
    );
}

#[test]
fn the_scene_save_is_not_an_asset_event() {
    // `voltra-scene` writes `demo.ron.tmp` and renames it over `demo.ron`.
    // Neither has a texture extension, so the filter drops both without
    // needing a rule about our own temporary files.
    let root = scratch_root();
    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");

    std::fs::write(root.join("demo.ron.tmp"), b"(version: 1)").expect("tmp");
    std::fs::rename(root.join("demo.ron.tmp"), root.join("demo.ron")).expect("rename");

    let seen = drain_for(&mut watcher, Duration::from_secs(2));
    assert!(seen.is_empty(), "a scene save must be silent: {seen:?}");
}

#[test]
fn an_idle_watcher_drains_empty_without_blocking() {
    let root = scratch_root();
    let mut watcher = AssetWatcher::new(&root).expect("watching a scratch dir");

    let start = Instant::now();
    let drained = watcher.drain();

    assert!(drained.is_empty());
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "drain runs once per frame; it must never block"
    );
}

#[test]
fn a_root_that_does_not_exist_is_an_error_not_a_panic() {
    let missing = Path::new("voltra-no-such-root-anywhere");
    assert!(AssetWatcher::new(missing).is_err());
}
```

- [x] **Step 4: Run them and watch them fail to compile**

Run: `cargo test -p voltra-assets --test watch`
Expected: FAIL — `unresolved import 'voltra_assets::AssetWatcher'`.

- [x] **Step 5: Implement the watcher**

Create `crates/voltra-assets/src/watch.rs`:

```rust
//! Turning filesystem events into the [`AssetPath`]s that changed.
//!
//! One concept, and deliberately not two: this knows nothing about textures,
//! handles or the GPU. [`Textures::reload`](crate::textures::Textures::reload)
//! consumes what this produces, and the caller — `voltra_core::App` — is what
//! joins them. That seam is where a second kind of asset would attach.
//!
//! There is no subscriber registry, because there is one consumer and the shape
//! of the second is not known.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

use crate::error::AssetError;
use crate::path::AssetPath;

/// How long events are held before being delivered.
///
/// An image editor writing a PNG emits a burst — truncate, several writes,
/// close — and reloading on each one would upload a half-written file several
/// times per save. 200 ms merges the burst and is still imperceptible.
///
/// Not a parameter: neither Bevy nor Unreal exposes one, and there is no second
/// caller here to disagree with the value.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// File extensions that can become a texture, lowercase, without the dot.
///
/// The filter is by extension rather than by event kind, which is Unreal's
/// shape too. It also means the scene format's atomic save — `demo.ron.tmp`,
/// renamed over `demo.ron` — is ignored without a rule about our own temporary
/// files.
pub const TEXTURE_EXTENSIONS: &[&str] = &["png"];

/// Watches the asset root and reports which assets changed.
///
/// Dropping it stops the watch: the debouncer's own thread is joined by its
/// `Drop`.
pub struct AssetWatcher {
    /// Held for its `Drop`. Removing this field stops the watch immediately.
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    events: Receiver<DebounceEventResult>,
    /// The canonical form of the watched root.
    ///
    /// Canonical because the events arrive canonical, and on Windows that
    /// means an extended-length `\\?\` prefix that a root built from
    /// `CARGO_MANIFEST_DIR` does not have. Comparing the two forms is what
    /// crashed Bevy's watcher on Windows for a release (bevyengine/bevy#18342).
    root: PathBuf,
}

impl AssetWatcher {
    /// Starts a recursive watch on `root`.
    ///
    /// Recursive because an artist creates directories: a watch per known
    /// subdirectory would miss the folder of sprites added after startup.
    pub fn new(root: &Path) -> Result<Self, AssetError> {
        let canonical = root.canonicalize().map_err(|source| AssetError::Read {
            path: root.to_path_buf(),
            source,
        })?;

        let (tx, events) = channel();
        let mut debouncer =
            new_debouncer(DEBOUNCE, None, tx).map_err(|source| AssetError::Watch {
                root: canonical.clone(),
                source,
            })?;

        debouncer
            .watch(&canonical, RecursiveMode::Recursive)
            .map_err(|source| AssetError::Watch {
                root: canonical.clone(),
                source,
            })?;

        log::info!("watching {} for asset changes", canonical.display());

        Ok(Self {
            _debouncer: debouncer,
            events,
            root: canonical,
        })
    }

    /// Every asset path that changed since the last call, deduplicated.
    ///
    /// Never blocks: this runs once per frame, and a frame that waits on a
    /// filesystem notification is a dropped frame.
    ///
    /// Event *kinds* are deliberately not inspected. Which kind a save produces
    /// varies by platform and by the program doing the writing — an overwrite
    /// is `Create` on some, `Modify` on others, a rename pair on a third — and
    /// matching on them is what makes a watcher miss changes
    /// (bevyengine/bevy#10576). Any event on a path that could be a texture is
    /// a reload *attempt*; whether the file is readable is `Textures::reload`'s
    /// question, not this one's.
    pub fn drain(&mut self) -> Vec<AssetPath> {
        let mut seen = HashSet::new();
        let mut changed = Vec::new();

        loop {
            match self.events.try_recv() {
                Ok(Ok(events)) => {
                    for event in events {
                        for path in &event.paths {
                            if let Some(asset) = self.to_asset_path(path) {
                                if seen.insert(asset.clone()) {
                                    changed.push(asset);
                                }
                            }
                        }
                    }
                }
                Ok(Err(errors)) => {
                    for error in errors {
                        log::warn!("asset watcher: {error}");
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    // The debouncer thread is gone, so no further event will
                    // ever arrive. Worth one line, and nothing else: the editor
                    // keeps working, it just stops noticing file changes.
                    log::warn!("asset watcher stopped; file changes will no longer be noticed");
                    break;
                }
            }
        }

        changed
    }

    /// An absolute path from an event, relative to the root, if it is an asset.
    ///
    /// Returns `None` for anything that cannot be relativized rather than
    /// panicking. A path that will not relativize is an ordinary event, not a
    /// bug: a file deleted between the notification and this call, a junction,
    /// a path from a volume we do not own.
    fn to_asset_path(&self, path: &Path) -> Option<AssetPath> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        if !TEXTURE_EXTENSIONS.contains(&extension.as_str()) {
            return None;
        }

        // Canonicalizing fails for a path that has just been deleted, so fall
        // back to the path as delivered: the prefix may still strip, and a
        // delete is exactly the case `Textures::reload` handles by keeping the
        // pixels it already has.
        let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        let relative = absolute.strip_prefix(&self.root).ok().or_else(|| {
            log::debug!(
                "ignoring {} — not under {}",
                absolute.display(),
                self.root.display()
            );
            None
        })?;

        AssetPath::new(&relative.to_string_lossy()).ok()
    }
}
```

- [x] **Step 6: Export it**

In `crates/voltra-assets/src/lib.rs`, add `pub mod watch;` after `pub mod
textures;`, and `pub use watch::AssetWatcher;` after `pub use
textures::Textures;`.

- [x] **Step 7: Run the tests**

Run: `cargo test -p voltra-assets --test watch`
Expected: PASS, five tests. They take a few seconds — the negative cases wait
out a two-second window on purpose.

If `a_rewritten_png_arrives_as_an_asset_path` times out, do **not** raise
`DEADLINE`; ten seconds is already far past any real delivery latency, so a
timeout means the path never relativized. Run with
`RUST_LOG=debug cargo test -p voltra-assets --test watch -- --nocapture` and
read the "ignoring … not under …" lines: that is the Windows prefix problem,
and the fix is in `to_asset_path`, not in the deadline.

- [x] **Step 8: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [x] **Step 9: Commit**

```sh
git add Cargo.toml Cargo.lock crates/voltra-assets
git commit -m "feat(assets): watch the asset root for changes"
```

---

### Task 4: Wire it into the frame loop

**Files:**
- Modify: `crates/voltra-core/src/app.rs`
- Modify: `crates/voltra-editor/src/main.rs`

**Interfaces:**
- Consumes: `Textures::reload` (Task 2), `AssetWatcher::{new, drain}` (Task 3).
- Produces: `voltra_core::App::with_hot_reload(self) -> App`.

- [x] **Step 1: Add the field and the builder**

In `crates/voltra-core/src/app.rs`, add to `struct App`, directly under
`asset_root`:

```rust
    /// Whether to watch the asset root and reload textures as files change.
    ///
    /// Off by default. A shipped game has no reason to watch its own assets,
    /// and none of Unity, Unreal, Godot or Bevy leaves this on in a build.
    hot_reload: bool,
```

and to the `Option`-until-resumed group, beside `textures`:

```rust
    watcher: Option<AssetWatcher>,
```

Extend the `voltra_assets` import at the top of the file:

```rust
use voltra_assets::{AssetWatcher, Textures};
```

Add the builder beside `with_asset_root`:

```rust
    /// Watches the asset root and reloads textures as their files change.
    ///
    /// The editor wants this; a shipped game does not, which is why it is not
    /// the default. A failure to start the watch is logged and otherwise
    /// ignored — the app runs, it just does not notice file changes.
    pub fn with_hot_reload(mut self) -> Self {
        self.hot_reload = true;
        self
    }
```

- [x] **Step 2: Build the watcher in `resumed`**

In `resumed`, directly after the `Textures::new(...)` call and before the
`if self.ui.is_some()` block:

```rust
        if self.hot_reload {
            match AssetWatcher::new(&asset_root) {
                Ok(watcher) => self.watcher = Some(watcher),
                // Not fatal. Refusing to open the editor because a watch handle
                // could not be had would trade a working session for a feature
                // nobody has used yet this run.
                Err(e) => log::error!("hot reload disabled: {e}"),
            }
        }
```

`asset_root` is moved into `Textures::new` on the line above it, so change that
call's last argument to `asset_root.clone()`.

- [x] **Step 3: Drain once per frame**

Replace `App::update`'s body:

```rust
    fn update(&mut self) {
        self.clock.tick();
        self.reload_changed_assets();
    }

    /// Applies whatever the watcher saw since the last frame.
    ///
    /// Between the clock and the render, so a texture that changed this frame
    /// is on screen this frame rather than next. Costs one non-blocking
    /// `try_recv` when nothing changed, which is almost every frame.
    fn reload_changed_assets(&mut self) {
        let (Some(watcher), Some(textures), Some(renderer)) = (
            self.watcher.as_mut(),
            self.textures.as_mut(),
            self.renderer.as_ref(),
        ) else {
            return;
        };

        for path in watcher.drain() {
            textures.reload(
                renderer.context().device(),
                renderer.context().queue(),
                &path,
            );
        }
    }
```

- [x] **Step 4: The editor opts in**

In `crates/voltra-editor/src/main.rs`, change the final statement of `main`:

```rust
    let mut editor = Editor::default();
    app.with_ui(move |ui, frame| editor.ui(ui, frame))
        .with_hot_reload()
        .run();
```

- [x] **Step 5: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [x] **Step 6: Smoke it in the real editor**

The editor is a GUI app with an infinite event loop. Never run it in the
foreground.

Launch it detached with logging on, give it a few seconds, then — while it is
still running — overwrite `assets/sprites/checker.png` with a different image,
wait two seconds, and kill it. Then read the log.

Expected, in order: `asset root: …`, `watching … for asset changes`, and
`reloaded sprites/checker.png`. No warnings between them.

Restore the checker afterwards:

```sh
git checkout assets/sprites/checker.png
```

If the `reloaded` line never appears but `watching` did, the events are arriving
and being filtered out — that is `to_asset_path`, and `RUST_LOG=debug` names the
path it dropped.

- [x] **Step 7: Commit**

```sh
git add crates/voltra-core/src/app.rs crates/voltra-editor/src/main.rs
git commit -m "feat(core): reload changed assets each frame"
```

---

### Task 5: Record the decisions and close stage 12

**Files:**
- Modify: `docs/ARCHITECTURE.md`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-08-10-asset-hot-reload.md` (this file)

**Interfaces:** none.

- [x] **Step 1: Add the decisions**

In `docs/ARCHITECTURE.md`, under "Decisions", after
"Headless test scaffolding lives in `voltra-testkit`":

```markdown
### Hot reload swaps contents under a stable handle

**`Textures::reload` replaces the texture in the slot its handle already names
and replaces that handle's bind group with it.** The handle does not change,
so `Sprite`, `SpriteBatch`, the scene format and the renderer never learn that
anything happened — which is the only reason a reload can be a per-frame
operation rather than a walk of the world.

Replacing the bind group is not optional. It holds the *old* `TextureView`, so
swapping the texture alone changes nothing the GPU can see.

A reload that fails keeps the previous pixels and warns once. An image editor's
save leaves the file truncated for a few milliseconds and the debounce window
does not always cover it; degrading to the magenta checker on every save would
teach the reader to ignore the one colour that means "this path is broken".
Unreal and Unity both keep an imported asset when its source becomes unreadable.

A path that failed at load therefore gets **its own** slot holding the
placeholder's pixels rather than the shared placeholder handle. A `Sprite`
stores the handle it was given and nothing re-resolves it, so a shared handle
would make a broken path permanently broken: repairing one would mean
overwriting the texture every other broken path draws. Its own slot costs 256
bytes.

#### Rejected

- **Issuing a new handle and repointing the world.** Correct, and it makes the
  scene dirty for a change nobody made to it, plus a full world walk per save.
- **Degrading to the placeholder on a failed reload.** Flashes magenta on
  routine saves, which spends the signal that means a real broken path.

### The asset watcher is opt-in and filters by extension

**`App::with_hot_reload()` starts one recursive `notify` watch on the asset
root, debounced at 200 ms.** The editor opts in; a shipped game does not. All
four of Unity, Unreal, Godot and Bevy ship this switchable and none leaves it on
in a build.

Events are filtered by **file extension**, not by event kind. Which `EventKind`
an overwrite produces varies by platform and by the writing program, and
matching on them is what makes a watcher miss changes
([bevy#10576](https://github.com/bevyengine/bevy/issues/10576)). The extension
filter also makes the scene format's atomic save invisible for free — neither
`demo.ron.tmp` nor `demo.ron` is a texture — rather than as a rule about our own
temporary files.

Both the root and each event path are canonicalized before the prefix is
stripped, and an event that still will not relativize is logged at `debug` and
dropped. On Windows a canonical path carries the `\\?\` extended-length prefix
that a root built from `CARGO_MANIFEST_DIR` does not, and an `expect` on that
mismatch is exactly what crashed Bevy's watcher on Windows
([bevy#18342](https://github.com/bevyengine/bevy/issues/18342)).

`AssetWatcher` produces `AssetPath`s and nothing else — no textures, no handles,
no device. There is no subscriber registry: there is one consumer, `App`, and
the shape of a second is not known.

#### Rejected

- **Godot's rescan on window focus.** No new dependency, and less machinery.
  It works there because Godot keeps a `.import` database with timestamps; here
  the equivalent means walking the tree and `stat`ing every file, which is a
  filesystem index — a bigger thing than the watcher it avoids.
- **A cargo feature, as Bevy does it.** Keeps the dependency out of a release
  binary, at the price of `#[cfg]` through `Textures` and the frame loop. Code
  behind a disabled `cfg` is not compiled, which is the shape that let Bevy's
  Windows regression through.
- **Depending on `notify` directly as well as on the debouncer.** Two entries
  can drift to incompatible versions; the debouncer re-exports the one it was
  built against.
```

- [x] **Step 2: Fix the roadmap**

In `README.md`, replace the last two rows of the roadmap table:

```markdown
| 11 | Gizmos and physics | planned |
| 12a | Asset store: handles, paths, cache | done |
| 12b | Textures per sprite, batched by texture | done |
| 12c | Hot reload: watch, debounce, swap under a stable handle | done |
```

Stage 12 was split three ways in the 12a design and shipped that way; one
"planned" row hid two thirds of it being already merged.

- [x] **Step 3: Tick this plan**

Change every `- [x]` in this file to `- [x]`. Verify against
`git log --oneline main..HEAD` first — if a step describes something that is not
in the tree, leave it unticked and say so in the report.

- [x] **Step 4: Sanity check**

Run: `cargo test --workspace`
Expected: green. Documentation only; this confirms nothing else drifted.

- [x] **Step 5: Commit**

```sh
git add docs/ARCHITECTURE.md README.md docs/superpowers/plans/2026-08-10-asset-hot-reload.md
git commit -m "docs: record the hot reload decisions"
```

---

## Definition of done

- Overwriting a PNG while the editor is running changes what is on screen,
  without reopening the scene, and logs `reloaded <path>` once.
- A truncated or deleted file leaves the last good pixels on screen and warns
  once.
- A path that failed at load repairs itself when the file appears, under the
  handle the `Sprite` is already holding.
- A scene save produces no asset event.
- `App::with_hot_reload()` is opt-in; `voltra-editor` calls it and nothing else
  does. A watch that cannot start logs an error and the app still runs.
- The workspace depends on `notify-debouncer-full` and not on `notify`.
- `docs/ARCHITECTURE.md` carries both decisions with their rejected
  alternatives; the README roadmap shows 12a/12b/12c.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` are clean on the final tree.

## Spec coverage

| Spec section | Task |
| --- | --- |
| Opt-in on `App`, editor opts in | Task 4 |
| Watcher does not know what a texture is | Task 3 |
| Failed reload keeps the last good pixels | Task 2 |
| Every failed path gets its own placeholder slot | Task 1 |
| Only texture extensions are converted | Task 3 |
| One dependency, not two | Task 3 |
| Both paths canonicalized before stripping | Task 3 |
| Debounce is a documented constant | Task 3 |
| Data flow, drained in `App::update` | Task 4 |
| Policy tests | Task 2 |
| Transport tests | Task 3 |
| Decisions recorded | Task 5 |
