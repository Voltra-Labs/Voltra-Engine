# Asset store — design

Date: 2026-08-08
Status: approved
Stage: 12a of 12a/12b/12c

## Why this is only a third of stage 12

Stage 12 was framed as "textures per sprite, cache, hot reload". Those are three
subsystems with different interfaces and different failure modes, and building
them in one branch would force exactly the shortcut the hard rules forbid: a
store designed around whatever the batching change happened to need that
afternoon.

| | Delivers | Visible on screen |
| --- | --- | --- |
| **12a**, this spec | `voltra-assets`: identity, handle, store, PNG loading, cache | No. Tests only |
| **12b** | `Sprite` carries the reference, `SpriteBatch` splits by texture, the renderer binds per sub-batch, the scene format round-trips it, the inspector edits it | Yes |
| **12c** | Hot reload: watcher, debounce, ignoring our own `.tmp`, swapping contents under a stable handle | Yes |

12a is deliberately invisible. Its only consumer today is its own test suite,
which is what makes it reviewable on its own: nothing in this branch changes a
pixel, so a reviewer only has to judge the data structures.

## The state it starts from

Worth stating, because it is barer than "stage 12" suggests:

- `Renderer` holds one `white_bind_group`, a 1×1 white pixel, and binds it
  unconditionally on every draw (`renderer.rs:19`, `renderer.rs:170`). There is
  no texture selection anywhere between the ECS and the GPU.
- `SpriteBatch::from_world` merges every sprite in the world into a single
  `Mesh` — one draw call per frame (`batch.rs:60`).
- `Texture::from_png(device, queue, label, bytes, filter)` exists and is called
  from nothing but tests. There is no path-based loading, no cache and no handle
  type in any crate.

## Where the crate sits

`voltra-assets` depends on `voltra-render`, because loading a PNG ends in a GPU
upload and the cached thing *is* the GPU texture. Caching decoded bytes and
re-uploading per sprite would cache the cheap half.

```
voltra-scene ──┬── voltra-ecs
               ├── voltra-render
               └── voltra-assets ──> voltra-render
```

The `Device` and `Queue` it needs come from the re-export,
`voltra_render::wgpu::Device`. `voltra-assets` never declares a `wgpu`
dependency of its own — exactly one crate does, and it is `voltra-render`.

`voltra-scene` gains its edge to `voltra-assets` in 12b, not here.

Bevy splits this in two — `Assets<Image>` on the CPU side and
`RenderAssets<GpuImage>` on the render side — because its render world extracts
from the main world each frame. We have no such extraction, so one store holding
`voltra_render::Texture` is the whole thing.

## Files

One concept each, per CONVENTIONS.md.

| File | Concept |
| --- | --- |
| `handle.rs` | `Handle<T>` — index plus generation, `Copy` |
| `path.rs` | `AssetPath` — the identity that reaches the file, and its normalisation |
| `store.rs` | `Assets<T>` — a generational arena. Knows nothing of paths or disk |
| `textures.rs` | `Textures` — the root, the path→handle cache, loading on a miss |
| `placeholder.rs` | The magenta checker |
| `error.rs` | `AssetError` |

`lib.rs` declares the modules and re-exports the public surface, with no logic
in it.

`Handle<T>` and `Assets<T>` are generic because it costs a `PhantomData` and a
`Vec<T>`. There will be **no `AssetLoader` trait**: there is one implementor and
no second one is planned, so a trait would be speculative scaffolding of exactly
the kind the empty-crates rule exists to prevent.

## `Handle<T>`

```rust
pub struct Handle<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<fn() -> T>,
}
```

`Copy`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Debug`. The same shape
`voltra_ecs::Entity` already uses, so the engine has one idea of what a handle
is rather than two. `PhantomData<fn() -> T>` rather than `PhantomData<T>` so the
handle is `Send`/`Sync` and covariant regardless of what `T` is — it does not
own a `T`.

The generation is checked on every access, for the reason ARCHITECTURE.md
already records for the ECS: a slot is reused, so a stale handle would otherwise
read whichever asset took its place.

### Rejected: reference-counted strong/weak handles

What Bevy and Godot do, and it solves eviction properly. Rejected for now on two
grounds. `Sprite` is `Copy` and derives `serde`; an `Arc` inside it removes
`Copy`, and `batch.rs:60` and `pick.rs:45` copy sprites by value in per-frame
loops. And there is no measured memory problem asking for eviction — an editor
rarely evicts a texture, and eviction is a decision that should be pushed by a
profiler, not by symmetry with Bevy. The handle's shape does not change when
eviction arrives.

## `AssetPath`

### The security constraint this type exists for

A scene file is external input. It arrives in a pull request, from a
collaborator, from the internet. If `AssetPath` accepted any string, then

```ron
texture: Some(Path("../../../../Windows/System32/config/SAM")),
```

would make opening a scene read an arbitrary file outside the project. The PNG
decode would fail, but the read has already happened, and the resulting
`AssetError` distinguishes "not found" from "decoded badly" — a filesystem
oracle driven by a `.ron` someone sent you.

The type carries the invariant instead of asking every caller to remember it:

```rust
pub fn new(raw: &str) -> Result<Self, AssetError>
```

Rejects absolute paths, Windows volume prefixes (`C:`, `\\?\`, UNC) and any
`..` component. Normalises to forward slashes and collapses `.`. A value of this
type cannot name anything outside the asset root, so `Textures` does not need a
second check and cannot forget one.

`Deserialize` routes through the same constructor rather than through the
derive, which would skip it. That is the whole point: the check has to be on the
path a scene file actually takes.

### On-disk form

```rust
#[derive(Serialize, Deserialize)]
#[non_exhaustive]
pub enum AssetPath {
    Path(String),
}
```

A path, in an enum, from the first file written. Bevy chose the path as the
canonical identity deliberately and deferred UUIDs — *"everyone uses filesystems
to manage their asset source files"*
([bevyengine/bevy#8624](https://github.com/bevyengine/bevy/pull/8624)). Godot
4.4 writes a `uid://` alongside the path, and Unity writes a GUID into a `.meta`
sidecar; both survive a rename that a bare path does not, and both cost a
sidecar file and an import step that this engine has no editor to manage.

The enum is what makes that reversible. A `Uuid` variant can be added later
without changing `VERSION`, because files written today already say which shape
they are using. ARCHITECTURE.md's own warning is the reason: a file format is
the one thing that cannot be refactored freely, because files in the old shape
already exist.

Normalisation buys the obvious thing too — `sprites/hero.png` and
`./sprites/hero.png` are one `AssetPath`, so one cache entry, so one GPU
texture.

## `Assets<T>`

A generational arena: `insert(T) -> Handle<T>`, `get(Handle<T>) -> Option<&T>`,
`get_mut`, `remove(Handle<T>) -> Option<T>`, `len`, `is_empty`. Removing pushes
the slot onto a free list and bumps its generation, so the next `insert` reuses
the index under a new generation and every handle to the old occupant stops
resolving.

`remove` exists even though `Textures` never calls it, and that is not the
speculative scaffolding the rules forbid. An arena without `remove` is a `Vec`,
and its generation field is decorative — there is no way to make a slot go
stale, so the check that makes the type safe cannot be written or tested.
Including it is what completes the data structure; eviction is a separate
decision about *when* to call it, and that decision is still deferred.

The ECS records the ordering that this gets wrong easily: clear the slot before
bumping the generation, never after. ARCHITECTURE.md has the same note against
`World::despawn`, where reversing the two leaks a component per dead entity
forever.

It knows nothing about paths, files or the asset root. That is `Textures`' job,
and the split is what lets the arena be tested without a GPU.

## `Textures`

```rust
pub fn load(&mut self, device: &Device, queue: &Queue, path: &AssetPath) -> Handle<Texture>
```

Infallible on purpose — it always returns a usable handle.

- **Cache hit** → the same handle. Two sprites sharing a PNG share one GPU
  texture. That sentence is what 12a is for.
- **Cache miss** → `std::fs::read(root.join(path))` → `Texture::from_png` →
  insert → record in the map.
- **Failure** — missing, corrupt, unreadable — → `log::warn!` **once**, and the
  path is mapped to the placeholder's handle.

Failures are cached, not just successes. Without that, a sprite with a broken
path retries the read and logs every frame. The scene format already set this
precedent: an unrecognised component is named once, not once per save.

`get(&self, Handle<Texture>) -> &Texture` returns a reference, not an `Option`,
because the placeholder is inserted at construction and every handle this type
hands out is therefore valid. It resolves through `Assets::get` and
`expect`s — the invariant is real, and `Textures` never calls `Assets::remove`,
so no handle it issued can go stale. A handle forged from another store is the
one way to reach that `expect`, and a panic naming the invariant is the right
answer to it.

### Loading is synchronous

Bevy is asynchronous. We have no task system, and building one for this would be
the subproject rather than the module. Opening a scene stalls in proportion to
its PNGs.

This is not a shortcut that has to be undone: the placeholder is exactly what an
asynchronous load has to return while bytes are in flight, so the call site does
not change shape when async arrives. When the stall is measured and matters,
`load` gains a variant; the handle and the store do not move.

### The placeholder

8×8 magenta-and-black checks, `Filter::Nearest` so it stays crisp and stays
obviously wrong rather than being blurred into something that could pass for
art. It is the industry's answer for a reason — a 1×1 white pixel is
indistinguishable from a sprite with no texture, which makes a broken path
invisible outside the log.

Inserted into the store when `Textures` is constructed, so its handle is always
valid.

## Errors

```rust
pub enum AssetError {
    NotFound(PathBuf),
    Read(io::Error),
    Decode(voltra_render::TextureError),
    EscapesRoot(String),
    Absolute(String),
}
```

Separate variants because they call for different responses, the same reasoning
`SceneError` records. `EscapesRoot` and `Absolute` are rejections at
construction and never reach a filesystem call; the other three are runtime
failures that `load` turns into a warning and a placeholder.

`AssetError` is returned by `AssetPath::new` and is reachable from `load`'s
logging path. `load` itself does not return it — see above.

## Tests

Split by whether they need a GPU, as the repo already does.

**No GPU**, unit tests in the file they test:

- a stale handle is rejected after its slot is reused
- `..`, an absolute path and `C:\` are each rejected
- `sprites/hero.png` and `./sprites/hero.png` normalise equal
- backslashes normalise to forward slashes
- an `AssetPath` round-trips through serde, and a malicious string in a RON
  document is rejected on deserialize rather than accepted
- an empty store reports `is_empty`

**GPU**, `crates/voltra-assets/tests/headless_textures.rs`, gated exactly as
`voltra-render/tests/headless_render.rs` is so CI without an adapter still
passes:

- the same path loaded twice returns the same handle, and the store holds one
  texture beyond the placeholder
- two different paths return different handles
- a missing path returns the placeholder's handle
- a corrupt PNG returns the placeholder's handle
- the placeholder's handle resolves to an 8×8 texture

## Out of scope, stated so it is not quietly added

`Sprite` is not touched. `batch.rs` is not touched. The renderer keeps binding
its 1×1 white texture. There is no file watcher. Nothing in this branch is
visible on screen.
