# Asset hot reload — design

Date: 2026-08-10
Status: approved
Stage: 12c of 12a/12b/12c — the last third

## What this closes

Stage 12 was split three ways in
[the asset store design](2026-08-08-asset-store-design.md): the store (12a), the
sprite that names a texture (12b), and hot reload (12c). The first two are
merged. This is the last one, described there in one line: *"watcher, debounce,
ignoring our own `.tmp`, swapping contents under a stable handle"*.

Saving a PNG over one the editor already drew must change what is on screen,
without reopening the scene and without the scene file knowing anything
happened.

## The state it starts from

- `Textures` caches `AssetPath → Handle<Texture>` in `by_path`, and holds a
  `BindGroup` per handle in `bind_groups` (`textures.rs:19`).
- `Assets<T>` has `get_mut`, so a slot's contents can be replaced without
  changing the handle that names it (`store.rs:58`).
- A load that fails logs once and caches **the shared placeholder handle** for
  that path (`textures.rs:98`). This design changes that; see below.
- `App` owns `Textures` and builds it in `resumed`; `App::update` runs once per
  frame and currently only ticks the clock (`app.rs:91`).
- Nothing anywhere watches the filesystem. `notify` is not a dependency.

## How the four engines do it

**Bevy.** A `file_watcher` cargo feature, off by default. `FileWatcher` wraps
`notify-debouncer-full` on one recursive watch of the asset root, per asset
source. The handle is stable and the contents are swapped, which is what makes
`AssetEvent::Modified` a useful signal rather than a re-resolve.

Its Windows bug is worth more than its architecture here.
[bevyengine/bevy#18342](https://github.com/bevyengine/bevy/issues/18342): a
canonicalized path on Windows carries the `\\?\` extended-length prefix, the
watched root did not, and `strip_prefix` failed — turning every hot reload on
Windows into a panic. This engine is developed on Windows and its asset root is
built from `CARGO_MANIFEST_DIR` or `current_exe`, so the same two paths meet the
same way.

**Godot.** No OS watcher at all. `EditorFileSystem::scan_changes()` runs when
the editor window regains focus. Cheap to write, and it rescans the whole tree —
which made the editor visibly laggy on large projects and eventually needed a
`filesystem/refresh_on_focus` setting to turn off.

**Unreal.** A `DirectoryWatcher` module with user-configured monitored
directories and an extension filter, under Editor Preferences → Loading &
Saving → Auto Reimport, disableable with one checkbox.

**Unity.** Auto Refresh, on by default, switchable in Asset Pipeline
preferences.

Three things are common to all four, and each one is a decision below:

1. It is an **editor** feature. None of them leave it on in a shipped build.
2. It is **switchable**. All four have the toggle.
3. The watch is **on the root**, recursive — but only assets that are *already
   loaded* get reloaded. A PNG nobody asked for changing on disk does nothing.

### Rejected: Godot's rescan on focus

Less machinery, and it needs no new dependency. Rejected because it would be
*more* code here, not less: Godot can rescan cheaply because it already keeps a
`.import` database with timestamps. We have no such database, so the equivalent
would mean walking the tree and `stat`ing every file against remembered mtimes —
which is a filesystem index, a bigger thing than the watcher it was meant to
avoid.

## Decisions

### Hot reload is opt-in on `App`, and the editor opts in

`App::with_hot_reload()`, beside `with_asset_root`. A shipped game watches
nothing unless it asks. `voltra-editor` asks.

Rejected: a cargo feature, the way Bevy does it. It would keep `notify` out of a
release binary, but it puts `#[cfg]` through `Textures` and the frame loop, and
code behind a disabled `cfg` is not compiled — which is exactly the shape that
let Bevy's Windows regression through. The dependency cost of an unused watcher
is one thread that is never spawned.

### The watcher does not know what a texture is

`crates/voltra-assets/src/watch.rs` holds one concept: turning filesystem events
into the `AssetPath`s that changed.

```rust
pub struct AssetWatcher { /* debouncer + receiver */ }

impl AssetWatcher {
    pub fn new(root: &Path) -> Result<Self, AssetError>;
    /// Every asset path that changed since the last call. Never blocks.
    pub fn drain(&mut self) -> Vec<AssetPath>;
}
```

It never sees a `Device`, a `Handle` or a `Texture`. `Textures::reload` consumes
what it produces, and the caller — `App` — is what joins them.

That seam is where a second consumer would attach, and it costs nothing today
because both halves have to exist anyway. What is deliberately *not* built is a
subscriber registry: there is one consumer, and the shape of the second is
unknown, so a dispatch table would be the speculative scaffolding the hard rules
forbid.

### A failed reload keeps the last good pixels

Not the placeholder. An image editor writing a PNG produces a truncated file for
a few milliseconds, and the debounce window does not always cover it. Degrading
to magenta on every save would flash the scene magenta as a matter of routine,
which teaches the reader to ignore the one colour that means "this path is
broken". Unreal and Unity both keep the imported asset when its source file
becomes unreadable or disappears.

So: `warn` once, leave the slot alone, and let the next event retry. A deleted
texture stays on screen until something valid replaces it.

### Every failed path gets its own placeholder slot

This changes 12a's code deliberately.

Today a path that fails to load is cached to the **shared** placeholder handle.
With hot reload that is a dead end: fixing the typo and dropping the PNG in
place repoints `by_path`, but every `Sprite` already spawned still carries
`texture_handle: Some(placeholder)` in its component, and nothing re-resolves
it. The texture would stay magenta until the scene was reopened — losing exactly
the workflow this stage exists for.

Instead, a failed load inserts its **own** texture holding the placeholder's
pixels, under its own handle, and caches that. `reload` then swaps it in place
like any other, and the sprite fixes itself. The cost is an 8×8 RGBA texture and
one bind group per broken path: 256 bytes of pixels.

This breaks an invariant an existing test pins.
`crates/voltra-scene/tests/headless_sprite_textures.rs`'s
`a_path_that_does_not_load_draws_the_placeholder_checker` asserts handle
identity against `textures.placeholder()`. It becomes an assertion about the
*contents* — a magenta texel and a dark one, which it already checks in the
rendered frame. `Textures::placeholder()` stays: it is still what an untextured
run binds and still what the store is seeded with.

### Only texture extensions are converted

An event whose path does not end in a known texture extension is dropped before
it becomes an `AssetPath`. Today that list is `png`, because
`Texture::from_png` is the only decoder.

This buys the `.tmp` rule for free rather than as a special case: the scene
save's atomic write produces `demo.ron.tmp` and then `demo.ron`, and neither has
a texture extension. It is also Unreal's filter, for the same reason.

### Both paths are canonicalized before the prefix is stripped

The watcher stores `root.canonicalize()`, and canonicalizes each event path
before `strip_prefix`. When either fails, or the strip fails, the event is
logged at `debug` and dropped.

Never `expect`. A path that will not relativize is a normal event — a file
deleted between the notification and the call, a junction, a path from a volume
we do not own — and Bevy's Windows crash was precisely an `expect` on that case.

### One dependency, not two

`notify-debouncer-full` 0.7 re-exports the `notify` 8.2 it was built against, as
`notify_debouncer_full::notify`. Depending on both would let the two versions
drift apart on a `cargo update` and produce the same class of failure as two
copies of `wgpu`. So the workspace declares the debouncer only, and `watch.rs`
reaches `RecursiveMode` and `EventKind` through the re-export — the same rule
this repo already applies to `voltra_render::wgpu`.

### Debounce is a documented constant

200 ms, in `watch.rs`. Long enough to merge the burst of writes an image editor
emits while saving, short enough that the swap feels immediate.

Not a parameter: neither Bevy nor Unreal exposes one, and there is no second
caller to disagree with the value. The hard rule is about values the engine will
need to vary, and this one has nobody to vary it for.

## Files

| File | Change | Concept |
| --- | --- | --- |
| `crates/voltra-assets/src/watch.rs` | new | `AssetWatcher` — events to `AssetPath`s |
| `crates/voltra-assets/src/error.rs` | modified | `AssetError::Watch { root, source }` |
| `crates/voltra-assets/src/textures.rs` | modified | `reload`, and per-path placeholder slots |
| `crates/voltra-assets/src/lib.rs` | modified | `pub mod watch;`, re-export `AssetWatcher` |
| `crates/voltra-core/src/app.rs` | modified | `hot_reload` flag, `with_hot_reload`, build in `resumed`, drain in `update` |
| `crates/voltra-editor/src/main.rs` | modified | calls `.with_hot_reload()` |
| `Cargo.toml`, `crates/voltra-assets/Cargo.toml` | modified | `notify-debouncer-full` |
| `crates/voltra-assets/tests/hot_reload.rs` | new | the policy, with a GPU |
| `crates/voltra-assets/tests/watch.rs` | new | the transport, without one |
| `crates/voltra-scene/tests/headless_sprite_textures.rs` | modified | the placeholder assertion above |

`textures.rs` is 168 lines and gains roughly 40. It stays one file and one
concept; if it passes 300 it splits then, not speculatively.

## Data flow

```
image editor writes assets/sprites/hero.png
  │
  ├─ notify (its own thread) → debouncer, 200 ms window
  │      └─ Sender<DebounceEventResult> → Receiver held by AssetWatcher
  │
  └─ App::update, next frame
       ├─ AssetWatcher::drain
       │    canonicalize, strip the root, filter by extension,
       │    AssetPath::new, dedupe          → Vec<AssetPath>
       │
       └─ for each: Textures::reload(device, queue, &path)
            ├─ not in by_path        → nothing happens
            ├─ read + decode fails   → warn once, slot untouched
            └─ ok  → *store.get_mut(handle) = texture
                     bind_groups.insert(handle, new bind group)
```

The second line of the success case is the one that is easy to miss: the old
`BindGroup` holds the old `TextureView`, so replacing the texture without
replacing the bind group swaps nothing that the GPU can see.

Nothing else moves. `Sprite`, `SpriteBatch`, the scene file and the renderer are
untouched, because the handle they hold does not change. That is the whole
reason the swap happens under a stable handle rather than by issuing a new one.

## The API `App` gains

```rust
/// Watches the asset root and reloads textures as their files change.
///
/// Off by default: a shipped game has no reason to watch its own assets, and
/// none of Unity, Unreal, Godot or Bevy leaves this on in a build.
pub fn with_hot_reload(mut self) -> Self;
```

The watcher is built in `resumed`, after `Textures`, from the same resolved
root. A failure to start it is a `log::error!` and nothing else — the editor
opens and works, it just does not notice file changes. Refusing to launch
because an inotify handle could not be had would be the wrong trade.

## Tests

Split by whether they need a GPU, as the repo already does, and by whether they
need the operating system to deliver an event — which is the split that keeps
the suite deterministic.

**Policy**, `crates/voltra-assets/tests/hot_reload.rs`, GPU, no watcher
anywhere. Every one of these calls `reload` directly, so nothing waits on
anything:

- a loaded path whose file changed reloads: the handle is unchanged, the pixels
  are the new colour
- a path that was never loaded is a no-op, and the store does not grow
- a corrupt file leaves the previous pixels in place
- a path that failed at load and then appears reloads under **its original
  handle**, so a sprite holding it stops being magenta — the recovery case that
  the per-path placeholder slot exists for
- reloading the shared `placeholder()` handle's own path is not a thing that can
  happen, but a forged handle must not corrupt the store

**Transport**, `crates/voltra-assets/tests/watch.rs`, no GPU:

- a PNG written under a watched root appears in `drain()`. Bounded retry loop,
  not a fixed sleep: poll `drain` until it yields or a few seconds elapse, and
  fail with the elapsed time
- writing `scene.ron.tmp` and `scene.ron` yields nothing
- a path that cannot be relativized against the root is dropped, not panicked on
- `drain` on an idle watcher returns empty and does not block

## Out of scope, stated so it is not quietly added

The open scene `.ron` is not watched — that is a different question, because it
has to decide what happens to unsaved edits, and it needs an editor-side prompt
this design does not have. Shaders are not watched; they are not loaded through
`AssetPath` yet. There is no eviction, no asynchronous loading, and no asset
status channel to the inspector. `Sprite`, `SpriteBatch` and the renderer are
not touched at all.
