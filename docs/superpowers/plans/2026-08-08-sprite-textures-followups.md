# Sprite Textures Follow-ups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the seven review findings left open on `feature/sprite-textures`
after stage 12b landed, so the branch can be pushed and reviewed as finished
work.

**Architecture:** Four code changes and three housekeeping ones. The asset root
stops being a string literal in `App` and becomes a resolver in `voltra-assets`
plus a builder method. Sprite indices widen from `u16` to `u32`, removing the
silent 16 384-sprite ceiling the new `u32` ranges already implied. The headless
test scaffolding, currently copy-pasted in two crates and about to be needed in
a third, moves into one dev-only crate. On top of that scaffolding, one new test
renders the full path → handle → bind group → pixels chain that 12b's design
asked for and did not get. The repository finally ships a texture and a demo
scene so 12b is visible without the reader supplying their own PNG.

**Tech Stack:** Rust 2021, `wgpu` 30, `winit` 0.30, `egui` 0.35, `ron` 0.12,
`image` 0.25 (PNG only), `pollster` 1.0.

## Research this plan is built on

Read before deciding anything differs from what is written here.

- **Index width.** Bevy's `bevy_sprite_render` binds its sprite index buffer as
  `IndexFormat::Uint32` and pushes six indices per quad, and it opens a new
  batch whenever the image handle changes — the same split 12b already
  implements. `u32` is what an engine that batches by texture uses.
- **Asset root.** No engine resolves assets from the process working directory.
  Bevy: `BEVY_ASSET_ROOT`, else `CARGO_MANIFEST_DIR`, else the executable's
  parent. Unreal: everything hangs off the executable's `BaseDir`. Unity:
  `Application.dataPath` is `<project>/Assets` in the editor and
  `<exe>_Data` in a player build. Godot: `res://` is the project directory in
  the editor and the PCK beside the executable once exported. Task 1 copies
  that shape, with the cwd kept only as a last resort.
- **Shipping sample assets in the engine repo.** Bevy commits
  `assets/branding/bevy_bird_dark.png` and its examples name it. Task 5 does
  the same with a generated checker.

## Global Constraints

Copied from `CLAUDE.md`, `docs/ARCHITECTURE.md` and `docs/CONVENTIONS.md`.
Every task's requirements implicitly include this section.

- The engine is **2D only**. No depth buffer, no z-axis, no 3D scaffolding.
- Only `voltra-core` may depend on `winit`. Only `voltra-render` may depend on
  `wgpu`. Everything else goes through `voltra_render::wgpu`.
- `voltra-render` must **not** depend on `voltra-assets`. Dependency direction
  is `voltra-scene → voltra-assets → voltra-render`.
- All versions live in the root `[workspace.dependencies]`; member crates write
  `dep.workspace = true`. Never pin a version inside a member crate.
- No `unwrap()` outside tests. `expect("why this cannot fail")` when the
  invariant is real. Log through `log`, never `println!`.
- One concept per file. Split a module into a directory past roughly 300 lines
  or a second concept, `foo.rs` + `foo/`, never `foo/mod.rs` — except a test
  crate's `tests/common/mod.rs`, which cargo requires in that shape.
- Acceptance for every task: `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings` clean, then
  `cargo test --workspace` green. All three, every task, before the commit.
- Conventional Commits, scope = crate without the `voltra-` prefix, imperative
  subject ≤50 chars.
- **Subagents never rewrite history.** `git commit` and `git commit --amend` on
  your own last commit are allowed. `rebase`, `reset --hard`, `push --force`,
  `hash-object`, `write-tree`, `commit-tree`, `update-ref` are not. If an amend
  cannot be done with plain `git commit --amend`, stop and report.
- Branch: `feature/sprite-textures`. Do not push; the dispatching session does
  that in Task 8.

## File Structure

**Created**

- `crates/voltra-assets/src/root.rs` — resolving the directory `AssetPath`s are
  joined onto. Pure resolution rules plus one thin `default_root()` that reads
  the environment.
- `crates/voltra-testkit/Cargo.toml`, `crates/voltra-testkit/src/lib.rs` —
  dev-only helpers shared by every crate's headless GPU tests: adapter
  acquisition, texture readback, scratch directories, PNG writing.
- `crates/voltra-scene/tests/demo_scene.rs` — regenerates and then guards
  `assets/scenes/demo.ron`.
- `crates/voltra-scene/tests/headless_sprite_textures.rs` — the full
  path → handle → bind group → pixels chain.
- `assets/sprites/checker.png`, `assets/scenes/demo.ron` — the sample content.

**Modified**

- `crates/voltra-assets/src/lib.rs` — export the root resolver.
- `crates/voltra-core/src/app.rs` — `asset_root` field, `with_asset_root`,
  resolved root in `resumed`.
- `crates/voltra-render/src/mesh.rs` — `u32` indices.
- `crates/voltra-render/src/pass.rs` — `draw_mesh` doc only.
- `crates/voltra-scene/src/batch.rs` — `u32` indices.
- `crates/voltra-render/tests/*`, `crates/voltra-assets/tests/*` — migrated onto
  `voltra-testkit`.
- `Cargo.toml`, `crates/*/Cargo.toml` — new workspace member and dev-deps.
- `docs/ARCHITECTURE.md`, `docs/superpowers/plans/2026-08-08-sprite-textures.md`
  — decisions and bookkeeping.

## Execution waves

Tasks inside a wave touch disjoint files and can be dispatched in parallel.
Waves are sequential.

| Wave | Tasks | Why they cannot move |
| --- | --- | --- |
| 1 | Task 1, Task 2 | Task 2 rewrites `headless_render.rs`; Task 4 moves it. |
| 2 | Task 3, Task 4 | Task 4 must see Task 2's `u32` edits already in the file. |
| 3 | Task 5, Task 6 | Both need Task 4's crate; Task 6 needs Task 2's `u32`. |
| 4 | Task 7 | Documents what every earlier task decided. |
| 5 | Task 8 | Dispatching session only. |

---

### Task 1: The asset root stops being a literal

Finding 1. `crates/voltra-core/src/app.rs:284` hardcodes `PathBuf::from("assets")`,
which resolves against the process working directory. A shipped binary, or
`cargo run` from anywhere but the workspace root, finds nothing.

**Files:**
- Create: `crates/voltra-assets/src/root.rs`
- Modify: `crates/voltra-assets/src/lib.rs`
- Modify: `crates/voltra-core/src/app.rs` (the `App` struct, `impl App`, and
  `resumed`'s `Textures::new` call)
- Test: unit tests inside `crates/voltra-assets/src/root.rs`

**Interfaces:**
- Consumes: `Textures::new(&Device, &Queue, &BindGroupLayout, impl Into<PathBuf>)`,
  which already exists and does not change.
- Produces:
  - `voltra_assets::root::resolve_root(env_override: Option<PathBuf>, manifest_dir: Option<PathBuf>, exe_dir: Option<PathBuf>, cwd: &Path) -> PathBuf`
  - `voltra_assets::root::default_root() -> PathBuf`
  - `voltra_assets::root::ROOT_ENV: &str`
  - re-exported as `voltra_assets::{default_root, ROOT_ENV}`
  - `voltra_core::App::with_asset_root(impl Into<PathBuf>) -> App`

- [ ] **Step 1: Write the failing tests**

Create `crates/voltra-assets/src/root.rs` containing only this test module for
now, plus the `use` lines it needs. `resolve_root` takes its inputs as
arguments rather than reading the environment itself: `std::env::set_var` is
process-global and cargo runs unit tests on many threads, so an
environment-reading function under test would flake against its neighbours.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch tree with an `assets` directory `depth` levels above `start`.
    ///
    /// Returns `(start_dir, expected_assets_dir)`.
    fn tree_with_assets_above(depth: usize) -> (PathBuf, PathBuf) {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the clock is after 1970")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "voltra-root-{nanos}-{:?}",
            std::thread::current().id()
        ));
        let assets = base.join("assets");
        std::fs::create_dir_all(&assets).expect("assets dir");

        let mut start = base.clone();
        for level in 0..depth {
            start = start.join(format!("level{level}"));
        }
        std::fs::create_dir_all(&start).expect("start dir");

        (start, assets)
    }

    #[test]
    fn the_environment_override_wins_over_everything() {
        let (start, assets) = tree_with_assets_above(1);
        let forced = start.join("somewhere-else");

        let root = resolve_root(
            Some(forced.clone()),
            Some(start.clone()),
            Some(start),
            Path::new("."),
        );

        assert_eq!(root, forced);
        assert_ne!(root, assets, "the override must not be second-guessed");
    }

    #[test]
    fn the_manifest_directory_is_searched_before_the_executable() {
        // `cargo run -p voltra-editor` sets CARGO_MANIFEST_DIR to the *member*
        // crate, not the workspace root, so the walk upwards is the whole
        // point: `crates/voltra-editor` has no `assets` and the root does.
        let (manifest, assets) = tree_with_assets_above(2);
        let (exe, other_assets) = tree_with_assets_above(1);

        let root = resolve_root(None, Some(manifest), Some(exe), Path::new("."));

        assert_eq!(root, assets);
        assert_ne!(root, other_assets);
    }

    #[test]
    fn the_executable_directory_is_used_when_there_is_no_manifest() {
        let (exe, assets) = tree_with_assets_above(0);

        let root = resolve_root(None, None, Some(exe), Path::new("."));

        assert_eq!(root, assets);
    }

    #[test]
    fn a_directory_with_no_assets_anywhere_falls_back_to_the_cwd() {
        let (start, _) = tree_with_assets_above(0);
        let barren = start.join("no-assets-here");
        std::fs::create_dir_all(&barren).expect("barren dir");
        let cwd = Path::new("/some/working/dir");

        // `barren`'s parent does hold an `assets`, so walk from a tree that
        // has none at all: a fresh temp dir with nothing above it we control.
        let lonely = std::env::temp_dir().join("voltra-root-lonely-nonexistent");
        let root = resolve_root(None, None, Some(lonely), cwd);

        assert_eq!(root, cwd.join("assets"));
    }

    #[test]
    fn the_walk_upwards_is_bounded() {
        // MAX_ASCENT levels up is found; one more is not. An unbounded walk
        // from a temp directory would happily adopt an `assets` sitting near
        // the drive root and resolve every path against a stranger's files.
        let (deep, assets) = tree_with_assets_above(MAX_ASCENT);
        assert_eq!(
            resolve_root(None, None, Some(deep), Path::new("/cwd")),
            assets
        );

        let (too_deep, _) = tree_with_assets_above(MAX_ASCENT + 1);
        assert_eq!(
            resolve_root(None, None, Some(too_deep), Path::new("/cwd")),
            Path::new("/cwd").join("assets"),
        );
    }

    #[test]
    fn the_default_root_is_absolute_in_this_workspace() {
        // Unit tests run with CARGO_MANIFEST_DIR set to voltra-assets, whose
        // grandparent holds the repository's `assets`.
        let root = default_root();
        assert!(root.is_absolute(), "got {root:?}");
        assert!(root.ends_with("assets"), "got {root:?}");
    }
}
```

- [ ] **Step 2: Run the tests and watch them fail to compile**

Run: `cargo test -p voltra-assets root::`
Expected: FAIL — `cannot find function 'resolve_root' in this scope`.

- [ ] **Step 3: Implement the resolver**

Put this above the test module in `crates/voltra-assets/src/root.rs`:

```rust
//! Where an [`AssetPath`](crate::AssetPath) is resolved from.
//!
//! Nothing here reads a file's contents; this decides which directory the
//! paths in a scene are relative to, once, at startup.
//!
//! No engine resolves this against the process working directory, and neither
//! does this one: a working directory is whatever shell or launcher started
//! the process, and it changes the meaning of every path in every scene file.
//! Bevy resolves `BEVY_ASSET_ROOT`, then `CARGO_MANIFEST_DIR`, then the
//! executable's parent; Unreal hangs everything off the executable's base
//! directory; Unity's `Application.dataPath` is `<project>/Assets` in the
//! editor and `<exe>_Data` in a build; Godot's `res://` is the project
//! directory in the editor and the PCK beside the executable once exported.
//! The order below is that shape, with the working directory kept only as the
//! answer of last resort — something has to be returned, and a wrong root
//! surfaces as "texture failed to load", which is a logged, recoverable state
//! rather than a panic.

use std::path::{Path, PathBuf};

/// Environment variable that overrides every other rule.
///
/// Named for this engine rather than reusing anyone else's: a shell that
/// already exports `BEVY_ASSET_ROOT` for another project must not reach into
/// ours.
pub const ROOT_ENV: &str = "VOLTRA_ASSET_ROOT";

/// The directory name looked for while walking upwards.
const ASSETS_DIR: &str = "assets";

/// How many levels above the starting directory the walk may look.
///
/// Bounded rather than "up to the filesystem root": an unbounded walk from a
/// temporary directory would adopt any `assets` directory sitting near the
/// drive root and silently resolve every scene against a stranger's files.
/// Six covers `target/debug/deps` under a workspace member, which is the
/// deepest layout this repository produces.
const MAX_ASCENT: usize = 6;

/// The root every [`AssetPath`](crate::AssetPath) is joined onto, for a caller
/// that has not been told one.
///
/// [`App::with_asset_root`](../../voltra_core/struct.App.html) overrides this;
/// this is what it falls back to.
pub fn default_root() -> PathBuf {
    resolve_root(
        std::env::var_os(ROOT_ENV).map(PathBuf::from),
        std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from),
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf)),
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    )
}

/// The resolution rules, with every input passed in.
///
/// Separated from [`default_root`] so the rules can be tested without touching
/// process-global state: `std::env::set_var` is visible to every thread, and
/// cargo runs unit tests on many.
///
/// - `env_override` is taken verbatim, existing or not. Someone who set it
///   meant it, and silently ignoring a typo would be worse than failing to
///   load the textures under it.
/// - `manifest_dir` is set by `cargo run` and `cargo test`. It names the
///   *member* crate in a workspace, which is why the search walks upwards
///   rather than joining `assets` onto it.
/// - `exe_dir` is the shipped-binary case, and gets the same upward walk so a
///   `target/debug/voltra-editor.exe` still finds the repository's assets.
/// - `cwd` is the last resort.
pub fn resolve_root(
    env_override: Option<PathBuf>,
    manifest_dir: Option<PathBuf>,
    exe_dir: Option<PathBuf>,
    cwd: &Path,
) -> PathBuf {
    if let Some(root) = env_override {
        return root;
    }

    manifest_dir
        .as_deref()
        .and_then(ascend_to_assets)
        .or_else(|| exe_dir.as_deref().and_then(ascend_to_assets))
        .unwrap_or_else(|| cwd.join(ASSETS_DIR))
}

/// The nearest `assets` directory at or above `start`, within [`MAX_ASCENT`].
fn ascend_to_assets(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .take(MAX_ASCENT + 1)
        .map(|dir| dir.join(ASSETS_DIR))
        .find(|candidate| candidate.is_dir())
}
```

- [ ] **Step 4: Export it**

In `crates/voltra-assets/src/lib.rs`, add `pub mod root;` beside the other
module declarations and `pub use root::{default_root, ROOT_ENV};` beside the
other re-exports, both in the file's existing alphabetical position.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p voltra-assets root::`
Expected: PASS, six tests.

- [ ] **Step 6: Give `App` the parameter**

In `crates/voltra-core/src/app.rs`:

Add the field to `struct App`, directly under `config`:

```rust
    /// Where [`AssetPath`]s resolve from, when the caller has an opinion.
    ///
    /// `None` means [`voltra_assets::default_root`] decides at `resumed` time.
    /// A game that ships its assets somewhere unusual sets this; the editor
    /// does not need to.
    ///
    /// [`AssetPath`]: voltra_assets::AssetPath
    asset_root: Option<PathBuf>,
```

Add the builder beside `with_ui`:

```rust
    /// Sets the directory every texture path resolves against.
    ///
    /// Without this, [`voltra_assets::default_root`] resolves one at startup.
    pub fn with_asset_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.asset_root = Some(root.into());
        self
    }
```

Replace the `Textures::new` call in `resumed` (the `PathBuf::from("assets")`
argument) with:

```rust
        let asset_root = self
            .asset_root
            .clone()
            .unwrap_or_else(voltra_assets::default_root);
        log::info!("asset root: {}", asset_root.display());

        // The same layout object the sprite pipeline was built with —
        // `Renderer` owns both, so `texture_layout()` is guaranteed to be it.
        let textures = Textures::new(
            renderer.context().device(),
            renderer.context().queue(),
            renderer.texture_layout(),
            asset_root,
        );
```

- [ ] **Step 7: fmt, clippy, test**

Run, in order:
```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: no output from fmt, clippy clean, every test green.

- [ ] **Step 8: Commit**

```sh
git add crates/voltra-assets/src/root.rs crates/voltra-assets/src/lib.rs crates/voltra-core/src/app.rs
git commit -m "feat(assets): resolve the asset root instead of assuming it"
```

---

### Task 2: Sprite indices become `u32`

Finding 3. `crates/voltra-scene/src/batch.rs:102` casts the vertex count to
`u16`, so the 16 385th sprite in a batch wraps to index 0 and draws over the
first one — silently, in release, with no validation error. The ranges added in
12b are already `u32`.

**Files:**
- Modify: `crates/voltra-render/src/mesh.rs` (`indexed`, `draw`, `draw_range`,
  `QUAD_INDICES`)
- Modify: `crates/voltra-scene/src/batch.rs` (`QUAD_INDICES`, the `indices`
  field, `push`, one existing test's cast)
- Test: `crates/voltra-scene/src/batch.rs` unit tests,
  `crates/voltra-render/tests/headless_render.rs:334`

**Interfaces:**
- Produces:
  - `voltra_render::Mesh::indexed(&Device, &str, &[Vertex], &[u32]) -> Mesh`
  - `voltra_render::mesh::QUAD_INDICES: [u32; 6]`
  - `voltra_scene::SpriteBatch::indices: Vec<u32>`
- Consumes: `SpriteRange { texture, indices: Range<u32> }`, unchanged.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module at the bottom of `crates/voltra-scene/src/batch.rs`:

```rust
    #[test]
    fn a_batch_past_sixty_five_thousand_vertices_still_indexes_its_own_quads() {
        // 16 384 sprites is 65 536 vertices, exactly where a `u16` base
        // wraps. The 16 385th sprite's indices must point at its own four
        // vertices; with a `u16` cast they point at the first sprite's, and
        // nothing — not wgpu, not a validation layer — reports it.
        const SPRITES: u32 = 16_385;

        let mut batch = SpriteBatch::default();
        for _ in 0..SPRITES {
            batch.push(&Transform::default(), &Sprite::default());
        }

        let last = batch
            .indices
            .last()
            .copied()
            .expect("a pushed batch has indices");
        assert!(
            last > u16::MAX as u32,
            "the last index wrapped: got {last}, expected past {}",
            u16::MAX
        );
        assert_eq!(batch.vertices.len() as u32, SPRITES * 4);
        assert!(batch.indices.iter().all(|&i| i < batch.vertices.len() as u32));
    }
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p voltra-scene a_batch_past_sixty_five_thousand`
Expected: FAIL — a type error on `u16::MAX as u32` against a `Vec<u16>`, or the
assertion itself once that is coerced. Either is the failure being fixed.

- [ ] **Step 3: Widen the render side**

In `crates/voltra-render/src/mesh.rs`, replace the `indexed` doc comment and
signature:

```rust
    /// Uploads vertices plus a `u32` index buffer.
    ///
    /// `u32` rather than `u16`: sprite batches are split by texture into
    /// ranges over one mesh, so a batch is not free to stop at 65 536 vertices
    /// the way a single-object mesh would be, and a wrapped index draws the
    /// wrong geometry without any validation error. Bevy binds its sprite
    /// indices as `Uint32` for the same reason. The cost is four bytes per
    /// index instead of two.
    pub fn indexed(
        device: &wgpu::Device,
        label: &str,
        vertices: &[Vertex],
        indices: &[u32],
    ) -> Self {
```

Change both `wgpu::IndexFormat::Uint16` occurrences (in `draw` and in
`draw_range`) to `wgpu::IndexFormat::Uint32`, and change the constant:

```rust
pub const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];
```

- [ ] **Step 4: Widen the scene side**

In `crates/voltra-scene/src/batch.rs`:

```rust
const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];
```

```rust
    pub indices: Vec<u32>,
```

In `push`, the base cast and the extend:

```rust
        // Every quad's indices are relative to its own first vertex; without
        // this offset each sprite would redraw the first one. `u32` because a
        // batch holds every sprite in the world, not one object's geometry.
        let base = self.vertices.len() as u32;
```

The `extend` line needs no change beyond the type flowing through it; confirm
it still reads:

```rust
        self.indices
            .extend(QUAD_INDICES.iter().map(|offset| base + offset));
```

In the existing `indices_never_point_past_the_vertices` test, change the cast:

```rust
        let count = batch.vertices.len() as u32;
```

- [ ] **Step 5: Widen the headless test constant**

In `crates/voltra-render/tests/headless_render.rs:334`:

```rust
const TWO_HALVES_INDICES: [u32; 12] = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
```

- [ ] **Step 6: Run the tests**

Run: `cargo test --workspace`
Expected: PASS, including the new test. If any other `[u16; N]` literal is
still fed to `Mesh::indexed`, the compiler names the file and line — widen it
the same way; do not cast at the call site.

- [ ] **Step 7: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 8: Commit**

```sh
git add crates/voltra-render/src/mesh.rs crates/voltra-scene/src/batch.rs crates/voltra-render/tests/headless_render.rs
git commit -m "fix(render): widen mesh indices to u32"
```

---

### Task 3: `draw_mesh` is documented as the non-indexed path

Finding 5. `pass::draw_mesh` has no caller in the engine since 12b routed the
renderer through `draw_mesh_batches`; only the headless tests use it. Deleting
it was considered and **rejected**: `draw_mesh_batches` reaches the GPU through
`Mesh::draw_range`, which requires an index buffer, so it cannot draw the
non-indexed meshes `Mesh::new` produces (`mesh::TRIANGLE`, and any future
geometry that has no indices). `draw_mesh` is the whole-mesh path, not dead
code — but nothing in the file says so, which is why it read as dead.

**Files:**
- Modify: `crates/voltra-render/src/pass.rs` (the `draw_mesh` doc comment only)

**Interfaces:**
- Produces: no signature changes. `pass::draw_mesh` and `pass::draw_mesh_batches`
  both stay public.

- [ ] **Step 1: Extend the doc comment**

In `crates/voltra-render/src/pass.rs`, replace the first line of `draw_mesh`'s
doc comment ("Records a clear followed by `mesh`, if there is one.") with:

```rust
/// Records a clear followed by all of `mesh`, if there is one.
///
/// The whole-mesh path, and the only one that can draw geometry with no index
/// buffer: [`draw_mesh_batches`] goes through [`Mesh::draw_range`], which is
/// indexed by construction. Sprites take the batched path because they are
/// split into per-texture ranges over one buffer; anything drawn in a single
/// call against a single bind group belongs here.
```

Leave the rest of the comment and the body untouched.

- [ ] **Step 2: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: unchanged behaviour, everything green.

- [ ] **Step 3: Commit**

```sh
git add crates/voltra-render/src/pass.rs
git commit -m "docs(render): say what draw_mesh is still for"
```

---

### Task 4: One test-support crate instead of three copies

Enabler for Task 6, and a cleanup on its own. `crates/voltra-render/tests/common/mod.rs`
and `crates/voltra-assets/tests/common/mod.rs` are near-identical copies of the
same headless scaffolding, and Task 6 needs a third copy in `voltra-scene`. Move
it once.

This is the one task that adds a workspace member. It is allowed under
"new crates only when there is code for them" because the code exists twice
already; the crate is `publish = false` and is only ever a `dev-dependency`.

**Files:**
- Create: `crates/voltra-testkit/Cargo.toml`
- Create: `crates/voltra-testkit/src/lib.rs`
- Modify: `Cargo.toml` (workspace dependency entry)
- Modify: `crates/voltra-render/Cargo.toml`, `crates/voltra-assets/Cargo.toml`
  (dev-dependency)
- Delete: `crates/voltra-render/tests/common/mod.rs`,
  `crates/voltra-assets/tests/common/mod.rs`
- Modify: every file under `crates/voltra-render/tests/` and
  `crates/voltra-assets/tests/` that declares `mod common;`

**Interfaces:**
- Produces, all from `voltra_testkit`:
  - `Rgba { r: u8, g: u8, b: u8, a: u8 }` with `is_clear_ish(&self) -> bool`
  - `CLEAR: wgpu::Color`
  - `headless_device() -> Option<(wgpu::Device, wgpu::Queue)>`
  - `read_texture(&Device, &Queue, &wgpu::Texture, width: u32, height: u32) -> Vec<Rgba>`
  - `scratch_root() -> PathBuf`
  - `write_png(root: &Path, name: &str, width: u32, height: u32)`

- [ ] **Step 1: Create the crate manifest**

`crates/voltra-testkit/Cargo.toml`:

```toml
[package]
name = "voltra-testkit"
description = "Headless GPU scaffolding shared by the workspace's integration tests."
publish = false
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
voltra-render.workspace = true
image.workspace = true
pollster.workspace = true
```

- [ ] **Step 2: Move the code into it**

`crates/voltra-testkit/src/lib.rs` is the union of the two existing
`tests/common/mod.rs` files, verbatim, with this header replacing theirs. There
is no `#![allow(dead_code)]`: a library crate's public items are not dead.

```rust
//! Headless GPU scaffolding shared by every crate's integration tests.
//!
//! A dev-dependency, never shipped: `publish = false`, and no member crate
//! depends on it outside `[dev-dependencies]`. It exists because each
//! integration test is its own binary and each crate its own `tests/` tree, so
//! the alternative is the same adapter-acquisition and readback code copied
//! once per crate — which is what it was until three copies were due.
//!
//! It depends on `voltra-render` for `wgpu`, like every other non-render
//! crate, and declares no `wgpu` of its own.
```

Copy, in this order: `Rgba` and its `is_clear_ish`, `CLEAR`, `headless_device`,
`read_texture` (from the `voltra-render` copy), then `scratch_root` and
`write_png` (from the `voltra-assets` copy). Keep every doc comment as written.
The `use` lines the union needs:

```rust
use std::path::{Path, PathBuf};

use voltra_render::wgpu;
```

- [ ] **Step 3: Register it in the workspace**

In the root `Cargo.toml`, in `[workspace.dependencies]`, alphabetically among
the `voltra-*` entries:

```toml
voltra-testkit = { path = "crates/voltra-testkit" }
```

`members = ["crates/*"]` already picks the directory up; do not add a members
entry.

- [ ] **Step 4: Point the two test trees at it**

In `crates/voltra-render/Cargo.toml` and `crates/voltra-assets/Cargo.toml`, add
to `[dev-dependencies]` (create the section if the crate has none):

```toml
voltra-testkit.workspace = true
```

Delete both `tests/common/mod.rs` files. In every test file that had
`mod common;`, delete that line and rewrite the import. For example, in
`crates/voltra-render/tests/headless_render.rs`:

```rust
use voltra_testkit::{headless_device, read_texture, Rgba, CLEAR};
```

and in `crates/voltra-assets/tests/headless_textures.rs`:

```rust
use voltra_testkit::{headless_device, scratch_root, write_png};
```

Import only what each file actually uses — without the blanket
`#![allow(dead_code)]` the copies carried, an unused import is now a clippy
error, which is the point.

- [ ] **Step 5: Confirm nothing moved but the code**

Run: `cargo test --workspace`
Expected: exactly the same test names, counts and results as before this task.
This is a move-only change; a behaviour difference here means something was
retyped rather than moved.

- [ ] **Step 6: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 7: Commit**

```sh
git add Cargo.toml Cargo.lock crates/voltra-testkit crates/voltra-render crates/voltra-assets
git commit -m "refactor(testkit): share the headless test scaffolding"
```

---

### Task 5: The repository ships a texture and a demo scene

Finding 2. `assets/` holds two `.gitkeep` files. Stage 12b is the stage whose
whole point is something visible on screen, and nobody can see it without
supplying their own PNG and typing its path into the inspector.

Bevy commits `assets/branding/bevy_bird_dark.png` and names it from its
examples; this does the same with a generated checker. The checker is
deliberately blue-and-white, not magenta: magenta-and-black is the
missing-texture signal and a sample asset must not look like a failure.

**Files:**
- Create: `assets/sprites/checker.png` (generated, committed)
- Create: `assets/scenes/demo.ron` (generated, committed)
- Create: `crates/voltra-scene/tests/demo_scene.rs`
- Modify: `crates/voltra-scene/Cargo.toml` (dev-dependencies)
- Delete: `assets/sprites/.gitkeep` if one appears; leave
  `assets/scenes/.gitkeep` alone.

**Interfaces:**
- Consumes, with these exact signatures — note the argument orders differ:
  - `voltra_scene::format::save(world: &World, registry: &ComponentRegistry, path: &Path) -> Result<(), SceneError>`
  - `voltra_scene::format::load(path: &Path, registry: &ComponentRegistry, world: &mut World) -> Result<(), SceneError>`
  - `voltra_scene::format::VERSION: u32`
  - `ComponentRegistry::with_defaults()`, `SceneId::new()`, `Sprite`,
    `Transform`, `voltra_assets::AssetPath`.
- Produces: `assets/sprites/checker.png` (64×64 RGBA), `assets/scenes/demo.ron`
  (three entities: two naming the checker, one untextured and tinted).

- [ ] **Step 1: Add the dev-dependencies**

In `crates/voltra-scene/Cargo.toml`, add a `[dev-dependencies]` section (or
extend the existing one). Both are needed by this task and Task 6, so both go in
now:

```toml
[dev-dependencies]
voltra-testkit.workspace = true
image.workspace = true
```

- [ ] **Step 2: Write the generator and the guard**

Create `crates/voltra-scene/tests/demo_scene.rs`:

```rust
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
```

- [ ] **Step 3: Generate the assets**

Run: `cargo test -p voltra-scene --test demo_scene -- --ignored regenerate_the_demo_assets`
Expected: PASS, and `git status` now shows `assets/sprites/checker.png` and
`assets/scenes/demo.ron` as new files.

- [ ] **Step 4: Run the guards**

Run: `cargo test -p voltra-scene --test demo_scene`
Expected: PASS, three tests (the generator stays ignored).

If `the_committed_demo_scene_loads` fails on the sprite count, the `save` path
only writes entities carrying a `SceneId` — check all three got one.

- [ ] **Step 5: Look at what was generated**

Read `assets/scenes/demo.ron`. It must contain `version: 1`, three entities,
and the string `sprites/checker.png` twice. If a `texture_handle` appears
anywhere in it, `#[serde(skip)]` has been lost and this task stops here and
reports rather than committing.

- [ ] **Step 6: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 7: Commit**

```sh
git add assets crates/voltra-scene/Cargo.toml crates/voltra-scene/tests/demo_scene.rs Cargo.lock
git commit -m "feat(scene): ship a demo scene and its texture"
```

---

### Task 6: The resolve-to-pixels chain gets its headless test

Finding 4. 12b's design asked for a headless test proving that two sprites
naming one path sample one texture, that a bad path draws the checker, and that
an untextured sprite still tints through white. What landed tests the range
splitting (`voltra-render`) and the handle sharing (`voltra-assets`)
separately; nothing renders `Sprite` → `Textures` → `MeshDraw` → pixels end to
end, which is where a wrong bind group or a lost range would actually show.

`voltra-scene` is the only crate that can host it: it is the one that depends on
both `voltra-assets` and `voltra-render`.

**Files:**
- Create: `crates/voltra-scene/tests/headless_sprite_textures.rs`
- Test: itself

**Interfaces:**
- Consumes: `voltra_testkit::{headless_device, read_texture, scratch_root, Rgba, CLEAR}`
  (Task 4), `SpriteBatch { vertices, indices: Vec<u32>, ranges }` (Task 2),
  `Textures::{new, load, bind_group, placeholder}`,
  `voltra_render::{pass::{draw_mesh_batches, MeshDraw}, pipeline, texture, camera::{Camera2D, CameraBinding}}`.
- Produces: nothing other code consumes.

- [ ] **Step 1: Write the test file**

Create `crates/voltra-scene/tests/headless_sprite_textures.rs`:

```rust
//! The whole chain, in pixels: a `Sprite`'s path becomes a handle, the handle
//! becomes a bind group, the batch's ranges become draw calls, and the right
//! texels land in the right half of the frame.
//!
//! Every link in that chain is already unit-tested on its own. This exists
//! because the links are what break: a bind group built against the wrong
//! layout, a range off by one quad, or a `None` run drawn against the
//! placeholder instead of white are all invisible to those tests and obvious
//! here.
//!
//! Skips itself when no GPU adapter is available.

use std::path::Path;

use voltra_assets::{AssetPath, Textures};
use voltra_render::camera::{Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{pipeline, texture, wgpu, Texture};
use voltra_scene::{Sprite, SpriteBatch, Transform};
use voltra_testkit::{headless_device, read_texture, scratch_root, Rgba, CLEAR};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

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

/// Writes an opaque single-colour PNG at `root/name`.
///
/// A flat colour rather than `voltra_testkit::write_png`'s red: these tests
/// tell two textures apart by the colour that comes back, so each needs its
/// own.
fn write_flat_png(root: &Path, name: &str, rgba: [u8; 4]) {
    use image::ImageEncoder;

    let path = root.join(name);
    std::fs::create_dir_all(path.parent().expect("the PNG has a parent")).expect("asset subdir");

    let pixels: Vec<u8> = (0..16 * 16).flat_map(|_| rgba).collect();
    let file = std::fs::File::create(&path).expect("creating the PNG");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(&pixels, 16, 16, image::ExtendedColorType::Rgba8)
        .expect("encoding the PNG");
}

/// Draws a batch exactly the way `App::redraw_*` does, and reads the frame back.
///
/// The mapping from `SpriteRange` to `MeshDraw` is deliberately the same shape
/// as `voltra_core::app::mesh_draws`: this test is worthless if it draws the
/// batch differently from the engine.
fn render_batch(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &SpriteBatch,
    textures: &Textures,
    camera: &Camera2D,
) -> Vec<Rgba> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless-sprite-target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let camera_binding = CameraBinding::new(device);
    camera_binding.upload(queue, camera);
    let layout = texture::bind_group_layout(device);
    let white = Texture::white(device, queue).create_bind_group(device, &layout);
    let render_pipeline =
        pipeline::create_flat_color(device, FORMAT, camera_binding.layout(), &layout);

    let mesh = batch.upload(device);
    let draws: Vec<MeshDraw> = batch
        .ranges
        .iter()
        .map(|range| MeshDraw {
            texture: match range.texture {
                Some(handle) => textures.bind_group(handle),
                None => &white,
            },
            indices: range.indices.clone(),
        })
        .collect();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-sprite-encoder"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &render_pipeline,
        camera_binding.bind_group(),
        mesh.as_ref(),
        &draws,
        CLEAR,
    );
    queue.submit(Some(encoder.finish()));

    read_texture(device, queue, &target, SIZE, SIZE)
}

fn at(pixels: &[Rgba], x: u32, y: u32) -> Rgba {
    pixels[(y * SIZE + x) as usize]
}

/// A camera wide enough that two sprites a unit apart land in opposite halves.
///
/// Built through `Camera2D::new`: `zoom` is a private field with a clamping
/// setter, precisely so nobody writes a zero into it.
fn wide_camera() -> Camera2D {
    Camera2D::new(Vec2::ZERO, 0.25, 1.0)
}

/// Tight enough that one sprite at the origin fills most of the frame.
fn close_camera() -> Camera2D {
    Camera2D::new(Vec2::ZERO, 0.5, 1.0)
}

/// A sprite at `x`, optionally naming `path`, resolved through `textures`.
fn sprite_at(
    x: f32,
    path: Option<&str>,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (Transform, Sprite) {
    let mut sprite = Sprite::default();
    if let Some(path) = path {
        sprite.set_texture(
            Some(AssetPath::new(path).expect("a valid asset path")),
            textures,
            device,
            queue,
        );
    }
    (Transform::from_translation(Vec2::new(x, 0.0)), sprite)
}

#[test]
fn two_sprites_naming_one_path_sample_the_same_texture() {
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();
    write_flat_png(&root, "sprites/green.png", [40, 200, 90, 255]);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let left = sprite_at(-1.0, Some("sprites/green.png"), &mut textures, &device, &queue);
    let right = sprite_at(1.0, Some("sprites/green.png"), &mut textures, &device, &queue);

    assert_eq!(
        left.1.texture_handle, right.1.texture_handle,
        "one path must resolve to one handle"
    );

    let mut batch = SpriteBatch::default();
    batch.push(&left.0, &left.1);
    batch.push(&right.0, &right.1);
    assert_eq!(batch.ranges.len(), 1, "one texture must be one run");

    let pixels = render_batch(&device, &queue, &batch, &textures, &wide_camera());
    let left_px = at(&pixels, SIZE / 4, SIZE / 2);
    let right_px = at(&pixels, SIZE * 3 / 4, SIZE / 2);

    assert!(!left_px.is_clear_ish(), "nothing drawn on the left");
    assert!(!right_px.is_clear_ish(), "nothing drawn on the right");
    assert!(
        left_px.g > left_px.r && left_px.g > left_px.b,
        "left should sample the green PNG, got {left_px:?}"
    );
    assert_eq!(
        left_px, right_px,
        "both sprites sample one texture, so both halves must match"
    );
}

#[test]
fn two_paths_reach_their_own_textures_in_one_frame() {
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();
    write_flat_png(&root, "green.png", [40, 200, 90, 255]);
    write_flat_png(&root, "blue.png", [50, 90, 220, 255]);

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let left = sprite_at(-1.0, Some("green.png"), &mut textures, &device, &queue);
    let right = sprite_at(1.0, Some("blue.png"), &mut textures, &device, &queue);

    let mut batch = SpriteBatch::default();
    batch.push(&left.0, &left.1);
    batch.push(&right.0, &right.1);
    assert_eq!(batch.ranges.len(), 2, "two textures must be two runs");

    let pixels = render_batch(&device, &queue, &batch, &textures, &wide_camera());
    let left_px = at(&pixels, SIZE / 4, SIZE / 2);
    let right_px = at(&pixels, SIZE * 3 / 4, SIZE / 2);

    assert!(
        left_px.g > left_px.b,
        "left half must be the green PNG, got {left_px:?}"
    );
    assert!(
        right_px.b > right_px.g,
        "right half must be the blue PNG, got {right_px:?}"
    );
}

#[test]
fn a_path_that_does_not_load_draws_the_placeholder_checker() {
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();

    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let missing = sprite_at(0.0, Some("absent.png"), &mut textures, &device, &queue);
    assert_eq!(
        missing.1.texture_handle,
        Some(textures.placeholder()),
        "a failed load must resolve to the placeholder handle"
    );

    let mut batch = SpriteBatch::default();
    batch.push(&missing.0, &missing.1);

    let pixels = render_batch(&device, &queue, &batch, &textures, &close_camera());

    // The placeholder is an 8x8 magenta-and-black checker, so the drawn area
    // holds both a strongly magenta pixel and a near-black one. A single
    // sample cannot tell a checker from a flat fill; two must disagree.
    let drawn: Vec<Rgba> = pixels
        .iter()
        .copied()
        .filter(|px| !px.is_clear_ish())
        .collect();
    assert!(!drawn.is_empty(), "the missing sprite drew nothing");
    assert!(
        drawn.iter().any(|px| px.r > 180 && px.b > 180 && px.g < 100),
        "no magenta texel in the drawn area: {:?}",
        &drawn[..drawn.len().min(8)]
    );
    assert!(
        drawn.iter().any(|px| px.r < 80 && px.g < 80 && px.b < 80),
        "no dark texel in the drawn area, so it is not a checker"
    );
}

#[test]
fn an_untextured_sprite_still_tints_through_white() {
    let (device, queue) = device_or_skip!();
    let layout = texture::bind_group_layout(&device);
    let root = scratch_root();

    let textures = Textures::new(&device, &queue, &layout, &root);
    let sprite = Sprite::new([0.9, 0.2, 0.2, 1.0]);
    assert!(sprite.texture_handle.is_none());

    let mut batch = SpriteBatch::default();
    batch.push(&Transform::default(), &sprite);
    assert_eq!(batch.ranges.len(), 1);
    assert!(
        batch.ranges[0].texture.is_none(),
        "an untextured sprite must produce a None run, not the placeholder"
    );

    let pixels = render_batch(&device, &queue, &batch, &textures, &close_camera());
    let centre = at(&pixels, SIZE / 2, SIZE / 2);

    assert!(
        centre.r > centre.g && centre.r > centre.b,
        "the sprite colour must survive the white texture, got {centre:?}"
    );
    assert!(!centre.is_clear_ish(), "nothing was drawn");
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p voltra-scene --test headless_sprite_textures`
Expected: PASS, four tests, on a machine with an adapter.

If a pixel assertion misses because a sprite lands off-frame or fills the whole
frame, adjust the zoom passed to `Camera2D::new` in `wide_camera` /
`close_camera` — that is the test's business. Do not change engine code to make
an assertion pass: the requirement is only that two sprites at x = ±1 land in
opposite halves of a 64×64 frame, and that one sprite at the origin covers its
centre.

- [ ] **Step 3: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 4: Commit**

```sh
git add crates/voltra-scene/tests/headless_sprite_textures.rs
git commit -m "test(scene): render the texture chain to pixels"
```

---

### Task 7: Record the decisions and close the 12b plan

Finding 6, plus the bookkeeping every earlier task generated. The 12b plan's
seven tasks are all committed with every checkbox still unticked, and three
decisions made in this plan are nowhere in `ARCHITECTURE.md`.

**Files:**
- Modify: `docs/superpowers/plans/2026-08-08-sprite-textures.md`
- Modify: `docs/ARCHITECTURE.md`

**Interfaces:** none.

- [ ] **Step 1: Tick the 12b plan**

In `docs/superpowers/plans/2026-08-08-sprite-textures.md`, change every `- [ ]`
to `- [x]`. All seven tasks landed: commits `b977be2`, `4d5cba2`, `929db80`,
`ed46d65`, `1fd2a12`, `de0d08b`, `2186850`. Verify with
`git log --oneline main..HEAD` before ticking; if a step describes something
that is not in the tree, leave that one unticked and say so in the report.

- [ ] **Step 2: Add the decisions**

In `docs/ARCHITECTURE.md`, under "Decisions", after the
"Sprites carry a path and a handle" section added in 12b, add:

```markdown
### The asset root is resolved, never assumed

**The directory `AssetPath`s are joined onto comes from
`voltra_assets::default_root()`, or from `App::with_asset_root`.** The
resolution order is `VOLTRA_ASSET_ROOT`, then the nearest `assets` directory at
or above `CARGO_MANIFEST_DIR`, then the nearest one at or above the
executable's directory, then `<cwd>/assets`.

No engine resolves assets against the process working directory, and this one
should not either: the working directory is set by whatever shell or launcher
started the process, and it silently changes what every path in every scene
file means. Bevy resolves `BEVY_ASSET_ROOT`, then `CARGO_MANIFEST_DIR`, then
the executable's parent. Unreal hangs every path off the executable's base
directory. Unity's `Application.dataPath` is `<project>/Assets` in the editor
and `<exe>_Data` in a player build. Godot's `res://` is the project directory
in the editor and the PCK beside the executable once exported.

The walk *upwards* is the one place this differs from Bevy: `cargo run -p
voltra-editor` sets `CARGO_MANIFEST_DIR` to `crates/voltra-editor`, not the
workspace root, so joining `assets` onto it directly would resolve to a
directory that does not exist. The walk is bounded at six levels, because an
unbounded one started in a temp directory would adopt any `assets` sitting near
the drive root.

#### Rejected

- **`<cwd>/assets`, which is what stage 12b shipped.** Works only when the
  process is started from the workspace root, which no shipped binary is.
- **The executable's directory alone.** Correct for a shipped game, wrong
  during development, where the binary is under `target/debug` and the assets
  are not.

### Sprite indices are `u32`

**`Mesh::indexed` takes `&[u32]` and binds `IndexFormat::Uint32`.** A sprite
batch is not one object's geometry: it holds every sprite in the world in one
buffer, split into per-texture ranges, so it cannot be "split before it gets
large" the way a mesh with its own draw call can. At 16 384 sprites a `u16`
base index wraps and the next sprite silently draws over the first, with no
validation error anywhere. Bevy binds its sprite index buffer as `Uint32` for
the same reason. The cost is two extra bytes per index.

### Headless test scaffolding lives in `voltra-testkit`

**Adapter acquisition, texture readback, scratch directories and PNG writing
are one dev-only crate, not one copy per crate's `tests/` tree.** Each
integration test is its own binary and each crate its own tree, so the
alternative is the same 120 lines copied per crate — it was already at two
copies with a third due. `voltra-testkit` is `publish = false` and appears only
under `[dev-dependencies]`, so it is not part of the shipped dependency graph.
```

- [ ] **Step 3: Sanity check**

Run: `cargo test --workspace`
Expected: green. Documentation only, but the doc links in `ARCHITECTURE.md` are
prose, not doctests — this is confirming nothing else drifted.

- [ ] **Step 4: Commit**

```sh
git add docs/ARCHITECTURE.md docs/superpowers/plans/2026-08-08-sprite-textures.md
git commit -m "docs: record the sprite texture follow-up decisions"
```

---

### Task 8: Push the branch and hand over the PR link

Finding 7. **Dispatching session only — not a subagent.** Subagents do not
push.

- [ ] **Step 1: Verify the branch**

```sh
git log --oneline main..HEAD
git status --short
```
Expected: the 12b commits plus this plan's, and a clean working tree.

- [ ] **Step 2: Full verification, once, on the final tree**

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 3: Editor smoke**

Launch `cargo run -p voltra-editor` detached, give it a few seconds, open
`assets/scenes/demo.ron` if driving it is possible, check the log for the
`asset root:` line and for warnings, then kill it. Never run it in the
foreground.

- [ ] **Step 4: Push**

```sh
git push -u origin feature/sprite-textures
```

- [ ] **Step 5: Hand over the PR link**

`gh` is not installed. Give the user the compare URL:
`https://github.com/Voltra-Labs/Voltra-Engine/compare/main...feature/sprite-textures`

---

## Definition of done

- `assets/sprites/checker.png` and `assets/scenes/demo.ron` are committed, and
  opening the demo scene in the editor shows two checkered sprites and one
  orange untextured one.
- The asset root is a resolved value, logged at startup, overridable by
  `App::with_asset_root` and by `VOLTRA_ASSET_ROOT`.
- A batch of 16 385 sprites indexes its own geometry.
- `pass::draw_mesh` says why it still exists.
- One copy of the headless scaffolding, in `voltra-testkit`.
- Four new headless tests cover path → handle → bind group → pixels, including
  the placeholder and the untextured cases.
- `docs/ARCHITECTURE.md` carries three new decisions with their rejected
  alternatives; the 12b plan is ticked.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` are all clean on the final tree.
- The branch is pushed and the compare link handed over.

## Finding coverage check

| Finding | Task |
| --- | --- |
| 1 — asset root hardcoded | Task 1 |
| 2 — `assets/` empty, 12b invisible | Task 5 |
| 3 — `u16` index overflow | Task 2 |
| 4 — no resolve-to-pixels test | Task 6 (enabled by Task 4) |
| 5 — `draw_mesh` reads as dead | Task 3 |
| 6 — 12b plan unticked, decisions unrecorded | Task 7 |
| 7 — branch never pushed | Task 8 |
