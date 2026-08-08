# Sprite Textures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make sprites name a texture by `AssetPath`, share GPU textures through `Textures`, and draw in painter's order with one bind group per contiguous texture run.

**Architecture:** `Sprite` stores a serialised `Option<AssetPath>` and a runtime `Option<Handle<Texture>>` (serde-skipped). `App` owns `Textures`, resolves handles on Open and path edits, and never loads inside `SpriteBatch::from_world`. The batch still sorts by `draw_key`, then records index ranges per contiguous handle; the renderer clears once and draws each range with the matching bind group. `voltra-render` never depends on `voltra-assets` — it only sees `&BindGroup` and index ranges.

**Tech Stack:** Rust 2021, `wgpu` 30 via `voltra_render::wgpu`, `voltra-assets` from 12a, `serde`/`ron`, `egui` 0.35 for the inspector field.

**Spec:** `docs/superpowers/specs/2026-08-08-sprite-textures-design.md`

## Global Constraints

- **Only `voltra-render` may depend on `wgpu`.** Other crates use `voltra_render::wgpu`.
- **All versions live in the root `[workspace.dependencies]`.** Member crates write `dep.workspace = true`.
- **No `unwrap()` outside tests.** `expect("why this cannot fail")` when the invariant is real. Tests use `.expect("…")` too.
- **Log through `log`, never `println!`.**
- **One concept per file.** Prefer `foo.rs` + `foo/` over `foo/mod.rs`. Split before a file needs "and".
- **`cargo clippy --workspace --all-targets -- -D warnings` must be clean.**
- **Every task ends with `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` before the commit.**
- **Conventional Commits**, scope = crate without `voltra-`, subject imperative ≤50 chars.
- **2D only.** No depth, no Z axis, no 3D scaffolding.
- **Bind group layouts must be the same object the pipeline was built with.** `Textures` must receive `Renderer`'s texture layout (or an equivalent shared reference) — creating a second layout with the same descriptor will not bind against the pipeline.
- **Painter's order wins over texture batching.** Never reorder sprites solely to merge textures.

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/voltra-scene/Cargo.toml` | Add `voltra-assets` dep | 1 |
| `crates/voltra-scene/src/sprite.rs` | Path + handle fields, `set_texture` | 1 |
| `crates/voltra-assets/src/textures.rs` | Cache bind groups; `bind_group` / `bind_group_layout` wiring | 2 |
| `crates/voltra-assets/tests/headless_textures.rs` | Update `Textures::new` call sites | 2 |
| `crates/voltra-scene/src/batch.rs` | Contiguous texture ranges | 3 |
| `crates/voltra-render/src/mesh.rs` | `draw_range` for indexed subranges | 4 |
| `crates/voltra-render/src/pass.rs` | Multi-bind draw after one clear | 4 |
| `crates/voltra-render/src/renderer.rs` | Expose layout + white bind group; multi-draw APIs | 4 |
| `crates/voltra-core/Cargo.toml` | Add `voltra-assets` dep | 5 |
| `crates/voltra-core/src/app.rs` | Own `Textures`, resolve, build draws | 5 |
| `crates/voltra-editor/Cargo.toml` | Add `voltra-assets` if needed for types in panels | 6 |
| `crates/voltra-editor/src/panels/inspector.rs` | Path text field + Clear | 6 |
| `crates/voltra-editor/src/panels/menu_bar.rs` | Resolve textures after Open | 6 |
| `docs/ARCHITECTURE.md` | Decision entry + pick note if needed | 7 |

## Resolve helper (shared by Tasks 1, 5, 6)

Put this on `Sprite` in Task 1. Later tasks call it; they do not reinvent it.

```rust
/// Sets or clears the texture path and refreshes the runtime handle.
///
/// `None` clears both fields so the renderer uses its white bind group.
/// `Some(path)` stores the path and loads through `textures` (which may
/// return the placeholder handle on failure).
pub fn set_texture(
    &mut self,
    path: Option<AssetPath>,
    textures: &mut Textures,
    device: &voltra_render::wgpu::Device,
    queue: &voltra_render::wgpu::Queue,
) {
    match path {
        None => {
            self.texture = None;
            self.texture_handle = None;
        }
        Some(path) => {
            let handle = textures.load(device, queue, &path);
            self.texture = Some(path);
            self.texture_handle = Some(handle);
        }
    }
}
```

World-wide resolve after Open (Task 5/6):

```rust
fn resolve_world_textures(
    world: &mut World,
    textures: &mut Textures,
    device: &Device,
    queue: &Queue,
) {
    let entities: Vec<_> = world
        .query::<Sprite>()
        .map(|(e, sprite)| (e, sprite.texture.clone()))
        .collect();
    for (entity, path) in entities {
        if let Some(sprite) = world.get_mut::<Sprite>(entity) {
            sprite.set_texture(path, textures, device, queue);
        }
    }
}
```

Note: cloning `Option<AssetPath>` then calling `set_texture` re-loads even when the handle was already set. That is intentional on Open (handles from a previous session are meaningless). Do not call this every frame.

---

### Task 1: `Sprite` carries path + handle

**Files:**
- Modify: `crates/voltra-scene/Cargo.toml`
- Modify: `crates/voltra-scene/src/sprite.rs`
- Modify: any `Sprite` construction / `Copy` assumptions in `batch.rs`, `pick.rs`, format tests that break
- Test: unit tests inside `sprite.rs`

**Interfaces:**
- Consumes: `voltra_assets::{AssetPath, Handle, Textures}`, `voltra_render::Texture`
- Produces: `Sprite { color, sort_order, texture, texture_handle }`, `Sprite::set_texture(...)`, `Default`/`new` leave both texture fields `None`. **No longer `Copy`.** Keep `Clone`.

- [x] **Step 1: Add the dependency**

In `crates/voltra-scene/Cargo.toml`:

```toml
voltra-assets.workspace = true
```

Keep alphabetical order among `voltra-*` lines (`voltra-assets` above `voltra-ecs`).

- [x] **Step 2: Write the failing tests**

Add to `sprite.rs` (or extend an existing test module) — tests only first, fields not yet present:

```rust
#[cfg(test)]
mod texture_tests {
    use super::*;
    use voltra_assets::AssetPath;

    #[test]
    fn default_sprite_has_no_texture() {
        let sprite = Sprite::default();
        assert!(sprite.texture.is_none());
        assert!(sprite.texture_handle.is_none());
    }

    #[test]
    fn texture_path_round_trips_through_ron_without_the_handle() {
        let mut sprite = Sprite::default();
        sprite.texture = Some(AssetPath::new("sprites/hero.png").expect("valid"));
        // Pretend a handle was resolved — must not appear on the wire.
        sprite.texture_handle = Some(voltra_assets::Handle::new(0, 0));

        let text = ron::to_string(&sprite).expect("serialize");
        assert!(
            !text.contains("texture_handle") && !text.contains("Handle"),
            "runtime handle leaked into RON: {text}"
        );
        let back: Sprite = ron::from_str(&text).expect("deserialize");
        assert_eq!(
            back.texture.as_ref().map(AssetPath::as_str),
            Some("sprites/hero.png")
        );
        assert!(back.texture_handle.is_none(), "handle must not deserialize");
    }

    #[test]
    fn old_ron_without_texture_field_still_loads() {
        let text = "(color:(1.0,1.0,1.0,1.0),sort_order:0)";
        let sprite: Sprite = ron::from_str(text).expect("old scene shape");
        assert!(sprite.texture.is_none());
        assert!(sprite.texture_handle.is_none());
    }

    #[test]
    fn hostile_texture_path_is_rejected_on_deserialize() {
        let hostile =
            r#"(color:(1.0,1.0,1.0,1.0),sort_order:0,texture:Some(Path("../../etc/passwd")))"#;
        assert!(ron::from_str::<Sprite>(hostile).is_err());
    }
}
```

`Handle::new` is `pub(crate)` today. Either:
- make a test-only constructor via `#[cfg(test)] pub fn new_for_test` on `Handle` in `voltra-assets`, **or**
- drop the line that sets `texture_handle` in the round-trip test and only assert the RON omits any handle-like payload after a real `set_texture` in a later GPU test.

Prefer: change the round-trip test to not forge a handle — set only `texture`, serialise, assert no `texture_handle` key and deserialized handle is `None`. Simpler, no API hole.

- [x] **Step 3: Run tests — expect compile failure** (`texture` field missing)

- [x] **Step 4: Implement**

Update `Sprite`:

```rust
use voltra_assets::{AssetPath, Handle, Textures};
use voltra_render::Texture;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sprite {
    pub color: [f32; 4],
    pub sort_order: i32,
    #[serde(default)]
    pub texture: Option<AssetPath>,
    #[serde(skip)]
    pub texture_handle: Option<Handle<Texture>>,
}
```

Remove `Copy` from the derive. Update `Default` / `new` / `with_sort_order` to leave texture fields `None`. Add `set_texture` as in the shared helper above.

Fix compile breaks: anywhere that moved a `Sprite` by `Copy` must clone or use references. Grep for `Sprite` in the workspace and fix.

- [x] **Step 5: Tests pass; fmt / clippy / workspace test**

- [x] **Step 6: Commit**

```bash
git add crates/voltra-scene/ crates/voltra-assets/src/handle.rs
git commit -m "feat(scene): give Sprite a texture path and handle"
```

---

### Task 2: Cache bind groups on `Textures`

**Files:**
- Modify: `crates/voltra-assets/src/textures.rs`
- Modify: `crates/voltra-assets/tests/headless_textures.rs`
- Modify: `crates/voltra-assets/tests/common/mod.rs` only if helpers need the layout

**Interfaces:**
- Consumes: `voltra_render::texture::bind_group_layout` pattern — caller passes `&BindGroupLayout`
- Produces:
  - `Textures::new(device, queue, layout: &BindGroupLayout, root) -> Self`
  - `Textures::bind_group(&self, handle: Handle<Texture>) -> &BindGroup`
  - Existing `load` / `get` / `placeholder` unchanged in behaviour

**Why the layout is an argument:** wgpu bind groups are tied to the layout object the pipeline was created with. A second `bind_group_layout(device)` call produces a distinct layout; groups from it will not bind to the flat-color pipeline.

- [x] **Step 1: Write a failing unit/integration expectation**

In `headless_textures.rs`, after constructing `Textures`, assert `textures.bind_group(textures.placeholder())` can be obtained (method missing → fail compile).

- [x] **Step 2: Implement**

Store:

```rust
layout: BindGroupLayout, // cloned/owned — BindGroupLayout is cloneable via Arc internally in wgpu; keep a owned layout
bind_groups: HashMap<Handle<Texture>, BindGroup>,
```

Actually `BindGroupLayout` is not `Clone` in all versions — hold it owned from `layout.clone()` if available, or store the layout the caller gave by creating groups only and requiring the caller to keep the layout alive. **Preferred:** take `&BindGroupLayout` at `new` and at every `load`, and store only the `HashMap<Handle, BindGroup>`. Do not store the layout if that forces a lifetime on `Textures`. Creating bind groups needs `&BindGroupLayout` on each insert:

```rust
pub fn new(
    device: &Device,
    queue: &Queue,
    layout: &BindGroupLayout,
    root: impl Into<PathBuf>,
) -> Self
```

On placeholder insert and on every successful `load` insert, call `texture.create_bind_group(device, layout)`.

**Problem:** later `load` needs the layout again. Options:
1. Store `BindGroupLayout` on `Textures` — wgpu 30's `BindGroupLayout` is `Clone` (reference-counted). Verify against vendored source before coding; if `Clone`, store it.
2. Pass `layout` into every `load` — ugly API.

**Plan requirement:** read `wgpu-30` `BindGroupLayout` docs/source. If `Clone`, store on `Textures`. If not, pass layout into `load` as well and update all call sites. Do not guess.

```rust
pub fn bind_group(&self, handle: Handle<Texture>) -> &BindGroup {
    self.bind_groups
        .get(&handle)
        .expect("Textures inserts a bind group with every texture")
}
```

Failed loads map to the placeholder handle — that bind group already exists; do not insert a duplicate map entry for the path beyond what `by_path` already does.

- [x] **Step 3: Update every `Textures::new` call** (headless tests) to pass a layout from `voltra_render::texture::bind_group_layout(&device)`.

- [x] **Step 4: fmt / clippy / test**

- [x] **Step 5: Commit**

```bash
git commit -m "feat(assets): cache texture bind groups"
```

---

### Task 3: `SpriteBatch` texture runs

**Files:**
- Modify: `crates/voltra-scene/src/batch.rs`
- Test: unit tests in `batch.rs`

**Interfaces:**
- Consumes: `sprite.texture_handle: Option<Handle<Texture>>`
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteRange {
    /// `None` → renderer white bind group.
    pub texture: Option<Handle<Texture>>,
    /// Index range into `SpriteBatch::indices` (and thus the uploaded mesh).
    pub indices: std::ops::Range<u32>,
}

pub struct SpriteBatch {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub ranges: Vec<SpriteRange>,
}
```

`from_world` sorts by `draw_key`, pushes quads, and builds `ranges` as contiguous runs of equal `texture_handle`.

- [x] **Step 1: Failing tests**

```rust
#[test]
fn contiguous_same_handles_merge_into_one_range() {
    // Two sprites, same Some(handle), same sort_order — one range covering 12 indices.
}

#[test]
fn interleaved_handles_split_ranges() {
    // Handles A, B, A in draw order → three ranges.
}

#[test]
fn none_and_some_do_not_merge() {
    // None then Some → two ranges.
}

#[test]
fn draw_order_still_follows_sort_order() {
    // Existing sort_order test still passes; ranges follow the same order.
}
```

For handles in unit tests without GPU: `Handle` needs a way to build values. Add to `voltra-assets` `handle.rs`:

```rust
#[cfg(test)]
impl<T> Handle<T> {
    pub fn from_raw(index: u32, generation: u32) -> Self {
        Self::new(index, generation)
    }
}
```

Only behind `cfg(test)` on the **consumer** does not work across crates. Options:
- `pub fn from_raw` documented as test/forged — rejected (forges are dangerous).
- Keep `pub(crate) new` and in scene tests only use `None` handles and distinguish by... can't distinguish two Nones.

**Better approach for batch tests without forging handles:** don't put `Handle` in unit tests of scene. Instead test range merging with a small pure function:

```rust
pub(crate) fn runs_from_handles(
    handles: &[Option<Handle<Texture>>],
    indices_per_sprite: u32, // 6
) -> Vec<SpriteRange>
```

And in `voltra-assets`, make `Handle::new` usable from integration tests via:

```rust
impl<T> Handle<T> {
    /// Forged handle for tests. Not issued by a store.
    #[doc(hidden)]
    pub fn forge(index: u32, generation: u32) -> Self {
        Self::new(index, generation)
    }
}
```

Use sparingly in batch tests. Document that `Textures::get` / `bind_group` will panic on forged handles — tests must not call those.

Alternatively export `Handle::new` as `pub` with docs "prefer store-issued handles". The ECS already has a similar visibility for Entity. Check `Entity` — if Entity construction is public for tests, match that.

- [x] **Step 2: Implement range building inside `from_world` / `push`**

Algorithm after sorted push loop (or during):

```rust
fn push_ranges(handles_in_order: &[Option<Handle<Texture>>]) -> Vec<SpriteRange> {
    let mut ranges = Vec::new();
    let per = QUAD_INDICES.len() as u32;
    let mut i = 0u32;
    while (i as usize) < handles_in_order.len() {
        let tex = handles_in_order[i as usize];
        let start = i * per;
        i += 1;
        while (i as usize) < handles_in_order.len() && handles_in_order[i as usize] == tex {
            i += 1;
        }
        ranges.push(SpriteRange {
            texture: tex,
            indices: start..(i * per),
        });
    }
    ranges
}
```

Empty world → empty ranges.

- [x] **Step 3: fmt / clippy / test**

- [x] **Step 4: Commit**

```bash
git commit -m "feat(scene): split sprite batches by texture run"
```

---

### Task 4: Multi-bind draw in `voltra-render`

**Files:**
- Modify: `crates/voltra-render/src/mesh.rs`
- Modify: `crates/voltra-render/src/pass.rs`
- Modify: `crates/voltra-render/src/renderer.rs`
- Keep existing `draw_mesh` working for headless tests that pass a single texture

**Interfaces:**
- Produces:

```rust
// mesh.rs
impl Mesh {
    /// Indexed draw of `indices` only. Panics if this mesh is not indexed.
    pub fn draw_range(&self, pass: &mut wgpu::RenderPass<'_>, indices: Range<u32>);
}

// pass.rs
pub struct MeshDraw<'a> {
    pub texture: &'a wgpu::BindGroup,
    pub indices: Range<u32>,
}

pub fn draw_mesh_batches(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    camera: &wgpu::BindGroup,
    mesh: Option<&Mesh>,
    draws: &[MeshDraw<'_>],
    clear: wgpu::Color,
);
```

`draw_mesh_batches`: begin pass with clear; if mesh is `None` or `draws` empty, return after clear; else set pipeline + camera once, then for each draw set bind group 1 and `mesh.draw_range`.

```rust
// renderer.rs
impl Renderer {
    pub fn texture_layout(&self) -> &wgpu::BindGroupLayout;
    pub fn white_bind_group(&self) -> &wgpu::BindGroup;

    pub fn render_mesh(&mut self, mesh: Option<&Mesh>, draws: &[MeshDraw<'_>]);
    pub fn render_scene(
        &mut self,
        target: &RenderTarget,
        mesh: Option<&Mesh>,
        draws: &[MeshDraw<'_>],
    );
}
```

Store `texture_layout` on `Renderer` (already created in `new` — keep it in a field instead of dropping).

- [x] **Step 1: Unit test for `draw_range` bounds** — if hard without GPU, rely on headless: one existing test still uses `draw_mesh`; add a headless test that draws two ranges with different bind groups (white vs a coloured texture) into one target and checks a pixel from each half. Skip without adapter.

- [x] **Step 2: Implement mesh / pass / renderer**

- [x] **Step 3: Update `app.rs` temporarily?** Prefer Task 5 for App. Until then, workspace must compile — update `app.rs` in this task to pass a single full-range draw with `white_bind_group` so the tree stays green:

```rust
let batch = SpriteBatch::from_world(&self.world);
let mesh = batch.upload(...);
let draws: Vec<_> = batch
    .ranges
    .iter()
    .map(|r| MeshDraw {
        texture: renderer.white_bind_group(), // temporary until Textures wired
        indices: r.indices.clone(),
    })
    .collect();
// If ranges empty but mesh some — shouldn't happen; if both empty, draws empty.
renderer.render_scene(target, mesh.as_ref(), &draws);
```

Actually after Task 3, ranges exist; using white for all keeps visuals identical to today until Task 5. Good incremental step — do that App stub in Task 4 so compile works, Task 5 replaces white lookup with real bind groups.

- [x] **Step 4: fmt / clippy / test**

- [x] **Step 5: Commit**

```bash
git commit -m "feat(render): draw mesh ranges with per-run binds"
```

---

### Task 5: `App` owns `Textures` and resolves draws

**Files:**
- Modify: `crates/voltra-core/Cargo.toml` — `voltra-assets.workspace = true`
- Modify: `crates/voltra-core/src/app.rs`
- Possibly `crates/voltra-core/src/lib.rs` re-exports if editor needs `Textures` through core

**Interfaces:**
- `App` gains `textures: Option<Textures>` (built in `resumed` with `renderer.texture_layout()`, root `PathBuf::from("assets")`).
- `UiFrame` gains `pub textures: &'a mut Textures` plus device/queue access for panels:

```rust
pub struct UiFrame<'a> {
    pub world: &'a mut World,
    pub camera: &'a mut Camera2D,
    pub textures: &'a mut Textures,
    pub device: &'a wgpu::Device, // or clone Arc as today
    pub queue: &'a wgpu::Queue,
    // ... existing viewport fields
}
```

Prefer cloning `device`/`queue` as `App` already does for egui — store clones on the frame or pass references from the redraw locals.

- Rebuild draws properly:

```rust
fn mesh_draws<'a>(
    batch: &SpriteBatch,
    textures: &'a Textures,
    white: &'a wgpu::BindGroup,
) -> Vec<MeshDraw<'a>> {
    batch
        .ranges
        .iter()
        .map(|range| MeshDraw {
            texture: match range.texture {
                Some(handle) => textures.bind_group(handle),
                None => white,
            },
            indices: range.indices.clone(),
        })
        .collect()
}
```

- Expose `pub fn resolve_textures(&mut self)` on `App` or a free function used after Open — needs `&mut world`, `&mut textures`, device, queue. If Open happens inside UI callback, resolve via `UiFrame` methods:

```rust
impl UiFrame<'_> {
    pub fn resolve_sprite_textures(&mut self) { ... }
}
```

- [x] **Step 1: Implement ownership + draw wiring + `UiFrame` fields**

- [x] **Step 2: Manual/editor smoke later; for now `cargo test --workspace`**

- [x] **Step 3: Commit**

```bash
git commit -m "feat(core): wire Textures into the frame loop"
```

---

### Task 6: Inspector path + Open resolve

**Files:**
- Modify: `crates/voltra-editor/Cargo.toml` — add `voltra-assets.workspace = true` if panels import `AssetPath` directly
- Modify: `crates/voltra-editor/src/panels/inspector.rs`
- Modify: `crates/voltra-editor/src/panels/menu_bar.rs`

**Inspector:**
- Local `String` buffer is wrong across selection changes — salt by entity (already `push_id`). Keep path edit as egui `TextEdit` bound to a temporary: on lost focus / Enter, parse with `AssetPath::new`; on `Ok`, `sprite.set_texture(Some(path), frame.textures, device, queue)`; on `Err`, log and leave previous path.
- Clear button → `set_texture(None, ...)`.
- Show current `sprite.texture.as_ref().map(AssetPath::as_str).unwrap_or("")`.

Simplest robust UX for 12b: each frame show `TextEdit` on a `String` cloned from the path; if the edit differs from the stored path and the widget signals changed+lost_focus, commit. Follow existing egui patterns in the file; verify against egui 0.35 via Context7 or vendored source before writing.

**Open:** after successful `load` and despawn of `previous`, call `frame.resolve_sprite_textures()`.

- [x] **Step 1: Implement inspector + Open**

- [x] **Step 2: fmt / clippy / test**

- [x] **Step 3: Optional detached editor smoke** — launch `cargo run -p voltra-editor` detached, confirm log clean, kill. Do not block on GUI interaction in CI.

- [x] **Step 4: Commit**

```bash
git commit -m "feat(editor): edit and resolve sprite textures"
```

---

### Task 7: Record the decision in `ARCHITECTURE.md`

**Files:**
- Modify: `docs/ARCHITECTURE.md`

**Content:**
- Under `## Decisions`, after the asset-store entry, add `### Sprites carry a path and a handle, and batch by contiguous runs`.
- Cover: path+handle split; resolve outside `from_world`; sort then split runs (Unity/Godot); `None` → white, bad path → magenta; no `VERSION` bump; bind groups cached on `Textures` with the pipeline's layout; `voltra-render` stays free of `voltra-assets`.
- Update the picking decision paragraph that says "the renderer binds one texture for the whole batch" — that sentence is now false. Replace with: sprites may carry textures; picking is still AABB until pixel-perfect lands.
- Wrap at 80 columns. Match surrounding voice. `#### Rejected` for texture-first sorting and for loading inside `from_world`.

- [x] **Step 1: Edit the doc**

- [x] **Step 2: `cargo test --workspace`** (sanity)

- [x] **Step 3: Commit**

```bash
git commit -m "docs: record sprite texture decisions"
```

---

## Definition of done

- `Sprite` round-trips `texture: Option<AssetPath>`; handle never on disk.
- Old scenes without the field open as untextured white quads.
- Contiguous same-handle sprites → one draw; interleaved → multiple; order matches `sort_order`.
- Two sprites, same path → one `Textures` entry / one GPU texture.
- Bad path → magenta checker; scene still opens.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace` passes (GPU tests skip without adapter).
- No crate but `voltra-render` names `wgpu` in `Cargo.toml`.
- Hot reload, atlases, file picker, pixel picking: **not** in this branch.

## Spec coverage check

| Spec item | Task |
| --- | --- |
| Path + handle on Sprite, serde skip | 1 |
| `set_texture` / None → white | 1, 5 |
| Failed path → placeholder | 2 (existing load) + 5 |
| Textures owned by app, resolve on Open/edit | 5, 6 |
| Sort then contiguous runs | 3 |
| Renderer multi-bind, no assets dep | 4 |
| Bind groups cached on Textures with pipeline layout | 2 |
| Inspector text + Clear | 6 |
| No VERSION bump | 1 (`#[serde(default)]`) |
| ARCHITECTURE decision | 7 |
| Out of scope items not implemented | — |
