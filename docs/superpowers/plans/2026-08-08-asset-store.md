# Asset Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `voltra-assets`, a crate that turns a path in a scene file into a cached GPU texture, so two sprites naming the same PNG share one `voltra_render::Texture`.

**Architecture:** A generational arena (`Assets<T>`) holds textures and hands out `Handle<T>`, the same index-plus-generation shape `voltra_ecs::Entity` already uses. `AssetPath` is a validated, normalised, root-relative path that cannot name anything outside the asset root — the check lives in the constructor and `Deserialize` routes through it. `Textures` owns the root, the arena and a path→handle map, loads PNGs on a cache miss, and returns a magenta-checker placeholder for anything that fails.

**Tech Stack:** Rust 2021, `wgpu` 30 (via `voltra_render::wgpu`, never declared directly), `serde`, `log`, `image` as a dev-dependency only.

**Spec:** `docs/superpowers/specs/2026-08-08-asset-store-design.md`

## Global Constraints

- **Only `voltra-render` may depend on `wgpu`.** `voltra-assets` reaches `Device`, `Queue` and `TextureFormat` through `voltra_render::wgpu`. A `wgpu` line in `crates/voltra-assets/Cargo.toml` is a rejected change.
- **All versions live in the root `[workspace.dependencies]`.** Member crates write `dep.workspace = true`. Never pin a version inside a member crate.
- **No `unwrap()` outside tests.** `expect("why this cannot fail")` when the invariant is real. Tests use `.expect("…")` too, not `.unwrap()`.
- **Log through `log`, never `println!`.**
- **One concept per file.** No file in this crate should need the word "and" to describe it.
- **`cargo clippy --workspace --all-targets -- -D warnings` must be clean.** A warning is a failure. A genuinely wrong lint gets a targeted `#[allow(...)]` with a comment — never a crate-level blanket allow.
- **Every task ends with `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` passing before the commit.**
- **Conventional Commits**, scope is the crate without the `voltra-` prefix, subject imperative and ≤50 characters.
- This is a 2D engine. Nothing here gains a third dimension, a depth field or a Z.

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `Cargo.toml` (root) | Adds `voltra-assets` to `[workspace.dependencies]` | 1 |
| `crates/voltra-assets/Cargo.toml` | Member manifest | 1 |
| `crates/voltra-assets/src/lib.rs` | Module declarations and re-exports. No logic | 1 |
| `crates/voltra-assets/src/handle.rs` | `Handle<T>` — index plus generation | 1 |
| `crates/voltra-assets/src/store.rs` | `Assets<T>` — the generational arena | 2 |
| `crates/voltra-assets/src/error.rs` | `AssetError` | 3 |
| `crates/voltra-assets/src/path.rs` | `AssetPath` — validation, normalisation, wire format | 3 |
| `crates/voltra-assets/src/placeholder.rs` | The magenta checker's pixels | 4 |
| `crates/voltra-assets/src/textures.rs` | `Textures` — root, cache, loading | 5 |
| `crates/voltra-assets/tests/headless_textures.rs` | GPU tests, skipped without an adapter | 5 |
| `docs/ARCHITECTURE.md` | Crate table, layer diagram, the decision entry | 6 |

## One refinement against the spec

The spec lists `AssetError::NotFound(PathBuf)` beside `Read(io::Error)`. This plan drops `NotFound`: `io::Error` already reports `ErrorKind::NotFound`, and a separate variant would make a caller check two things to answer one question. `Read { path, source }` carries the path the spec wanted from `NotFound` and keeps the kind intact. Everything else follows the spec as approved.

---

### Task 1: The crate, and `Handle<T>`

**Files:**
- Modify: `Cargo.toml` (root, `[workspace.dependencies]`)
- Create: `crates/voltra-assets/Cargo.toml`
- Create: `crates/voltra-assets/src/lib.rs`
- Create: `crates/voltra-assets/src/handle.rs`
- Test: unit tests inside `crates/voltra-assets/src/handle.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `voltra_assets::Handle<T>`, with `Handle::new(index: u32, generation: u32) -> Self` (crate-visible), `handle.index() -> u32`, `handle.generation() -> u32`. `Copy + Clone + PartialEq + Eq + Hash + Debug` **for every `T`, including a `T` that is none of those.**

- [ ] **Step 1: Add the crate to the workspace manifest**

In root `Cargo.toml`, inside `[workspace.dependencies]`, keeping the existing alphabetical order of the `voltra-*` lines:

```toml
voltra-assets = { path = "crates/voltra-assets" }
```

It goes above `voltra-core`. Change nothing else in the file — the `members = ["crates/*"]` glob already picks the new directory up.

- [ ] **Step 2: Write the member manifest**

Create `crates/voltra-assets/Cargo.toml`:

```toml
[package]
name = "voltra-assets"
description = "Loading and caching the files a scene refers to."
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
voltra-render.workspace = true
log.workspace = true
serde.workspace = true

[dev-dependencies]
# Only the tests encode a PNG; the crate itself never does — `Texture::from_png`
# does the decoding, inside `voltra-render`.
image.workspace = true
pollster.workspace = true
ron.workspace = true
```

There is deliberately no `wgpu` line. See Global Constraints.

- [ ] **Step 3: Write the failing test**

Create `crates/voltra-assets/src/handle.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A type that is deliberately neither `Clone` nor `Debug`.
    ///
    /// `#[derive(Clone)]` on a generic struct generates `impl<T: Clone>`, even
    /// when the only `T` in the struct is behind a `PhantomData` that does not
    /// own one. A derived `Handle` would therefore stop being `Copy` the moment
    /// it named an asset type that is not, which is a trap worth a test rather
    /// than a comment.
    struct NotClone;

    #[test]
    fn a_handle_is_copy_whatever_it_points_at() {
        let handle: Handle<NotClone> = Handle::new(3, 7);
        let copy = handle;
        assert_eq!(handle, copy);
        assert_eq!(handle.index(), 3);
        assert_eq!(handle.generation(), 7);
    }

    #[test]
    fn handles_to_different_slots_differ() {
        let a: Handle<NotClone> = Handle::new(0, 0);
        let b: Handle<NotClone> = Handle::new(1, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn the_same_slot_at_different_generations_differs() {
        // The whole point of the generation. Two handles naming slot 0 are not
        // interchangeable if the slot was reused in between.
        let old: Handle<NotClone> = Handle::new(0, 0);
        let new: Handle<NotClone> = Handle::new(0, 1);
        assert_ne!(old, new);
    }

    #[test]
    fn a_handle_can_key_a_hash_map() {
        let mut map = std::collections::HashMap::new();
        map.insert(Handle::<NotClone>::new(2, 5), "hero");
        assert_eq!(map.get(&Handle::<NotClone>::new(2, 5)), Some(&"hero"));
        assert_eq!(map.get(&Handle::<NotClone>::new(2, 6)), None);
    }
}
```

Create `crates/voltra-assets/src/lib.rs`:

```rust
//! Loading and caching the files a scene refers to.
//!
//! A scene names a texture by path; this crate turns that path into a
//! `voltra_render::Texture` on the GPU, once, no matter how many entities name
//! it. Sits below `voltra-scene` and above `voltra-render`, and reaches wgpu
//! only through `voltra_render::wgpu`.

pub mod handle;

pub use handle::Handle;
```

- [ ] **Step 2b: Run the test to verify it fails**

Run: `cargo test -p voltra-assets`
Expected: FAIL to compile, `cannot find type Handle in this scope`.

- [ ] **Step 4: Write the implementation**

At the top of `crates/voltra-assets/src/handle.rs`, above the test module:

```rust
//! A typed reference to something in an [`Assets`](crate::store::Assets) store.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// A typed reference to a stored asset.
///
/// The generation is what makes this safe to keep around: a slot is reused
/// after its occupant is removed, and without a generation an old handle would
/// silently address whatever asset took the slot. Same shape as
/// `voltra_ecs::Entity`, so the engine has one idea of what a handle is.
pub struct Handle<T> {
    index: u32,
    generation: u32,
    /// `fn() -> T` rather than `T`: a handle does not own a `T`, so it should
    /// be `Send`, `Sync` and covariant no matter what `T` is.
    _marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    pub(crate) fn new(index: u32, generation: u32) -> Self {
        Self {
            index,
            generation,
            _marker: PhantomData,
        }
    }

    /// Slot this handle addresses.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// How many times that slot has been reused.
    pub fn generation(&self) -> u32 {
        self.generation
    }
}

// Every trait below is implemented by hand rather than derived. A derive on a
// generic struct emits `impl<T: Clone>` and friends, which would make a
// `Handle` stop being `Copy` as soon as it named an asset type that is not —
// even though the `T` never appears in a field.

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<T> {}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle({}v{})", self.index, self.generation)
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p voltra-assets`
Expected: PASS, 4 tests.

- [ ] **Step 6: Verify the whole workspace**

Run, in order:
```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: clippy prints no warnings; every test passes.

- [ ] **Step 7: Commit**

```sh
git add Cargo.toml Cargo.lock crates/voltra-assets/
git commit -m "feat(assets): add the crate and a typed handle"
```

---

### Task 2: `Assets<T>`, the generational arena

**Files:**
- Create: `crates/voltra-assets/src/store.rs`
- Modify: `crates/voltra-assets/src/lib.rs`
- Test: unit tests inside `crates/voltra-assets/src/store.rs`

**Interfaces:**
- Consumes: `crate::handle::Handle` and its crate-visible `Handle::new(index, generation)`.
- Produces: `voltra_assets::Assets<T>` with `Assets::new() -> Self`, `Default`, `insert(&mut self, T) -> Handle<T>`, `get(&self, Handle<T>) -> Option<&T>`, `get_mut(&mut self, Handle<T>) -> Option<&mut T>`, `remove(&mut self, Handle<T>) -> Option<T>`, `len(&self) -> usize`, `is_empty(&self) -> bool`.

- [ ] **Step 1: Write the failing test**

Create `crates/voltra-assets/src/store.rs` with only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_store_is_empty() {
        let store: Assets<u32> = Assets::new();
        assert_eq!(store.len(), 0);
        assert!(store.is_empty());
    }

    #[test]
    fn an_inserted_asset_reads_back() {
        let mut store = Assets::new();
        let handle = store.insert("hero");
        assert_eq!(store.get(handle), Some(&"hero"));
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn get_mut_hands_out_the_stored_value() {
        let mut store = Assets::new();
        let handle = store.insert(1u32);
        if let Some(value) = store.get_mut(handle) {
            *value = 2;
        }
        assert_eq!(store.get(handle), Some(&2));
    }

    #[test]
    fn removing_returns_the_asset_and_empties_the_slot() {
        let mut store = Assets::new();
        let handle = store.insert("hero");
        assert_eq!(store.remove(handle), Some("hero"));
        assert_eq!(store.get(handle), None);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn removing_twice_reports_the_second_as_gone() {
        let mut store = Assets::new();
        let handle = store.insert("hero");
        assert_eq!(store.remove(handle), Some("hero"));
        assert_eq!(store.remove(handle), None);
    }

    #[test]
    fn a_stale_handle_does_not_read_the_slot_that_replaced_it() {
        // The invariant the generation exists for. Without it, `stale` would
        // address "villain" — the bug this whole shape prevents.
        let mut store = Assets::new();
        let stale = store.insert("hero");
        store.remove(stale);
        let fresh = store.insert("villain");

        assert_eq!(fresh.index(), stale.index(), "the slot must be reused");
        assert_ne!(fresh.generation(), stale.generation());
        assert_eq!(store.get(stale), None);
        assert_eq!(store.get(fresh), Some(&"villain"));
    }

    #[test]
    fn a_stale_handle_cannot_remove_the_slot_that_replaced_it() {
        let mut store = Assets::new();
        let stale = store.insert("hero");
        store.remove(stale);
        let fresh = store.insert("villain");

        assert_eq!(store.remove(stale), None);
        assert_eq!(store.get(fresh), Some(&"villain"));
    }

    #[test]
    fn a_handle_from_another_store_does_not_resolve() {
        // Indices are per-store, so a handle from one is meaningless in
        // another. `get` must answer `None` rather than index out of bounds.
        let mut a = Assets::new();
        let from_a = a.insert("hero");
        let b: Assets<&str> = Assets::new();
        assert_eq!(b.get(from_a), None);
    }

    #[test]
    fn slots_are_reused_rather_than_growing_the_arena() {
        let mut store = Assets::new();
        for _ in 0..8 {
            let handle = store.insert(0u32);
            store.remove(handle);
        }
        let handle = store.insert(1u32);
        assert_eq!(handle.index(), 0, "one slot should have served all of them");
        assert_eq!(store.len(), 1);
    }
}
```

Add to `crates/voltra-assets/src/lib.rs`, keeping the module list alphabetical:

```rust
pub mod handle;
pub mod store;

pub use handle::Handle;
pub use store::Assets;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p voltra-assets`
Expected: FAIL to compile, `cannot find type Assets in this scope`.

- [ ] **Step 3: Write the implementation**

At the top of `crates/voltra-assets/src/store.rs`, above the test module:

```rust
//! A generational arena of assets of one type.

use crate::handle::Handle;

/// Stores assets of one type and hands out [`Handle`]s to them.
///
/// Knows nothing about paths, files or an asset root — that is
/// [`Textures`](crate::textures::Textures)' job, and the split is what lets
/// this be tested without a GPU.
#[derive(Debug)]
pub struct Assets<T> {
    slots: Vec<Option<T>>,
    /// How many times each slot has been reused. Indexed alongside `slots`.
    generations: Vec<u32>,
    /// Slots ready for reuse, so an arena that churns does not grow forever.
    free: Vec<u32>,
    live: usize,
}

impl<T> Default for Assets<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Assets<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            generations: Vec::new(),
            free: Vec::new(),
            live: 0,
        }
    }

    /// Stores `asset` and returns a handle to it.
    pub fn insert(&mut self, asset: T) -> Handle<T> {
        self.live += 1;

        if let Some(index) = self.free.pop() {
            let slot = index as usize;
            self.slots[slot] = Some(asset);
            return Handle::new(index, self.generations[slot]);
        }

        let index = self.slots.len() as u32;
        self.slots.push(Some(asset));
        self.generations.push(0);
        Handle::new(index, 0)
    }

    /// `None` if the handle is stale, already removed, or from another store.
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        self.slot(handle).and_then(|slot| self.slots[slot].as_ref())
    }

    /// `None` if the handle is stale, already removed, or from another store.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let slot = self.slot(handle)?;
        self.slots[slot].as_mut()
    }

    /// Takes the asset out and frees its slot for reuse.
    ///
    /// The slot is cleared *before* its generation is bumped. Reversing those
    /// two is the mistake `World::despawn` documents in ARCHITECTURE.md: bump
    /// first and the value stays in the arena with no handle able to reach it,
    /// which leaks one asset per removal forever.
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let slot = self.slot(handle)?;
        let asset = self.slots[slot].take()?;

        self.generations[slot] = self.generations[slot].wrapping_add(1);
        self.free.push(handle.index());
        self.live -= 1;
        Some(asset)
    }

    /// How many assets are stored.
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// The slot `handle` addresses, if it is in range and current.
    fn slot(&self, handle: Handle<T>) -> Option<usize> {
        let slot = handle.index() as usize;
        // The bounds check is what makes a handle from another store safe to
        // pass in: its index may name a slot this arena does not have.
        if self.generations.get(slot).copied()? != handle.generation() {
            return None;
        }
        Some(slot)
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p voltra-assets`
Expected: PASS, 13 tests.

- [ ] **Step 5: Verify the whole workspace**

Run, in order:
```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: clippy prints no warnings; every test passes.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-assets/src/
git commit -m "feat(assets): add a generational asset store"
```

---

### Task 3: `AssetPath` and `AssetError`

**Files:**
- Create: `crates/voltra-assets/src/error.rs`
- Create: `crates/voltra-assets/src/path.rs`
- Modify: `crates/voltra-assets/src/lib.rs`
- Test: unit tests inside `crates/voltra-assets/src/path.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `voltra_assets::AssetError`, an enum with variants `Absolute(String)`, `EscapesRoot(String)`, `Empty`, `Read { path: std::path::PathBuf, source: std::io::Error }`, `Decode { path: std::path::PathBuf, source: voltra_render::TextureError }`. Implements `Display` and `std::error::Error`.
  - `voltra_assets::AssetPath` with `AssetPath::new(raw: &str) -> Result<Self, AssetError>` and `as_str(&self) -> &str`. `Clone + PartialEq + Eq + Hash + Debug + Serialize + Deserialize`.

**This task is the security boundary of the crate.** A scene file is external input. Read the "The security constraint this type exists for" section of the spec before writing any of it.

- [ ] **Step 1: Write `AssetError`**

Create `crates/voltra-assets/src/error.rs`:

```rust
//! What can go wrong naming or loading an asset.

use std::fmt;
use std::path::PathBuf;

use voltra_render::TextureError;

/// A failure to name or load an asset.
///
/// The first three are rejections at construction and never reach a filesystem
/// call. The last two are runtime failures, which `Textures::load` turns into
/// a warning and a placeholder rather than propagating.
#[derive(Debug)]
pub enum AssetError {
    /// An absolute path, a volume prefix, or a UNC path.
    Absolute(String),
    /// A `..` component, which would name a file outside the asset root.
    EscapesRoot(String),
    /// A path with nothing left after normalisation.
    Empty,
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Decode {
        path: PathBuf,
        source: TextureError,
    },
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absolute(raw) => {
                write!(f, "`{raw}` is absolute; asset paths are relative to the asset root")
            }
            Self::EscapesRoot(raw) => {
                write!(f, "`{raw}` leaves the asset root")
            }
            Self::Empty => write!(f, "an asset path cannot be empty"),
            Self::Read { path, source } => {
                write!(f, "could not read {}: {source}", path.display())
            }
            Self::Decode { path, source } => {
                write!(f, "could not decode {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Decode { source, .. } => Some(source),
            Self::Absolute(_) | Self::EscapesRoot(_) | Self::Empty => None,
        }
    }
}
```

- [ ] **Step 2: Write the failing test**

Create `crates/voltra-assets/src/path.rs` with only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ok(raw: &str) -> String {
        AssetPath::new(raw)
            .expect("this path is valid")
            .as_str()
            .to_owned()
    }

    #[test]
    fn a_plain_relative_path_survives_unchanged() {
        assert_eq!(ok("sprites/hero.png"), "sprites/hero.png");
    }

    #[test]
    fn a_leading_dot_is_collapsed() {
        // Two spellings of one file must be one cache entry, or the same PNG
        // is uploaded to the GPU twice.
        assert_eq!(ok("./sprites/hero.png"), "sprites/hero.png");
        assert_eq!(ok("sprites/./hero.png"), "sprites/hero.png");
    }

    #[test]
    fn backslashes_normalise_to_forward_slashes() {
        // A path typed on Windows and a path typed on Linux must be the same
        // entry, and the file it names has to be the same on both.
        assert_eq!(ok(r"sprites\hero.png"), "sprites/hero.png");
    }

    #[test]
    fn repeated_separators_collapse() {
        assert_eq!(ok("sprites//hero.png"), "sprites/hero.png");
    }

    #[test]
    fn a_parent_component_is_rejected() {
        // The reason this type exists. A scene file is external input, and
        // `..` in it would read a file outside the project.
        assert!(matches!(
            AssetPath::new("../../etc/passwd"),
            Err(AssetError::EscapesRoot(_))
        ));
        assert!(matches!(
            AssetPath::new("sprites/../../secret"),
            Err(AssetError::EscapesRoot(_))
        ));
    }

    #[test]
    fn a_unix_absolute_path_is_rejected() {
        assert!(matches!(
            AssetPath::new("/etc/passwd"),
            Err(AssetError::Absolute(_))
        ));
    }

    #[test]
    fn a_windows_volume_path_is_rejected() {
        assert!(matches!(
            AssetPath::new(r"C:\Windows\System32\config\SAM"),
            Err(AssetError::Absolute(_))
        ));
    }

    #[test]
    fn a_unc_path_is_rejected() {
        assert!(matches!(
            AssetPath::new(r"\\server\share\file.png"),
            Err(AssetError::Absolute(_))
        ));
        assert!(matches!(
            AssetPath::new(r"\\?\C:\file.png"),
            Err(AssetError::Absolute(_))
        ));
    }

    #[test]
    fn an_alternate_data_stream_is_rejected() {
        // `hero.png:$DATA` names a different stream of the same file on NTFS.
        // The colon check that catches `C:` catches this too, deliberately.
        assert!(matches!(
            AssetPath::new("sprites/hero.png:$DATA"),
            Err(AssetError::Absolute(_))
        ));
    }

    #[test]
    fn an_empty_path_is_rejected() {
        assert!(matches!(AssetPath::new(""), Err(AssetError::Empty)));
        assert!(matches!(AssetPath::new("./"), Err(AssetError::Empty)));
    }

    #[test]
    fn two_spellings_of_one_path_are_the_same_value() {
        let a = AssetPath::new("./sprites/hero.png").expect("valid");
        let b = AssetPath::new(r"sprites\hero.png").expect("valid");
        assert_eq!(a, b);

        let mut map = std::collections::HashMap::new();
        map.insert(a, 1);
        assert_eq!(map.get(&b), Some(&1), "and therefore one cache entry");
    }

    #[test]
    fn it_round_trips_through_ron() {
        let path = AssetPath::new("sprites/hero.png").expect("valid");
        let text = ron::to_string(&path).expect("serializes");
        assert_eq!(text, r#"Path("sprites/hero.png")"#);
        let back: AssetPath = ron::from_str(&text).expect("deserializes");
        assert_eq!(back, path);
    }

    #[test]
    fn a_hostile_path_in_a_document_is_rejected_on_deserialize() {
        // The check has to be on the path a scene file actually takes. A
        // derived `Deserialize` would skip the constructor and let this
        // through, which is the whole reason it is written by hand.
        let hostile = r#"Path("../../../../Windows/System32/config/SAM")"#;
        assert!(ron::from_str::<AssetPath>(hostile).is_err());
    }

    #[test]
    fn a_normalising_path_normalises_on_deserialize_too() {
        let back: AssetPath =
            ron::from_str(r#"Path("./sprites/hero.png")"#).expect("deserializes");
        assert_eq!(back.as_str(), "sprites/hero.png");
    }
}
```

Add to `crates/voltra-assets/src/lib.rs`, keeping the lists alphabetical:

```rust
pub mod error;
pub mod handle;
pub mod path;
pub mod store;

pub use error::AssetError;
pub use handle::Handle;
pub use path::AssetPath;
pub use store::Assets;
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p voltra-assets`
Expected: FAIL to compile, `cannot find type AssetPath in this scope`.

- [ ] **Step 4: Write the implementation**

At the top of `crates/voltra-assets/src/path.rs`, above the test module:

```rust
//! The identity a scene file uses to name an asset.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::AssetError;

/// A validated, normalised path relative to the asset root.
///
/// **This type is a security boundary.** A scene file is external input — it
/// arrives in a pull request, from a collaborator, from the internet. A raw
/// string from one could name `../../../../Windows/System32/config/SAM`, and
/// merely opening the scene would read it; the PNG decode would fail, but the
/// read has already happened and the error distinguishes "missing" from
/// "malformed", which is a filesystem oracle driven by a `.ron` someone sent
/// you.
///
/// The invariant therefore lives in the constructor rather than in every
/// caller's memory: a value of this type cannot name anything outside the
/// root, so [`Textures`](crate::textures::Textures) needs no second check and
/// cannot forget one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetPath(String);

impl AssetPath {
    /// Validates and normalises `raw`.
    ///
    /// Rejects absolute paths, volume prefixes and any `..`. Normalises
    /// backslashes to forward slashes, collapses `.` and repeated separators.
    pub fn new(raw: &str) -> Result<Self, AssetError> {
        // Backslashes first, so a Windows-flavoured path is judged by the same
        // rules as a Unix one rather than arriving as a single component.
        let unified = raw.replace('\\', "/");

        if unified.starts_with('/') {
            return Err(AssetError::Absolute(raw.to_owned()));
        }
        // Catches the `C:` volume prefix and, on NTFS, `file.png:$DATA` — an
        // alternate data stream is a different file wearing the same name.
        if unified.contains(':') {
            return Err(AssetError::Absolute(raw.to_owned()));
        }

        let mut parts = Vec::new();
        for part in unified.split('/') {
            match part {
                "" | "." => continue,
                ".." => return Err(AssetError::EscapesRoot(raw.to_owned())),
                other => parts.push(other),
            }
        }

        if parts.is_empty() {
            return Err(AssetError::Empty);
        }

        Ok(Self(parts.join("/")))
    }

    /// The normalised path, relative to the asset root, forward slashes.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The wire form, kept separate from the type that carries the invariant.
///
/// An enum from the first file written, so a `Uuid` variant can be added later
/// without changing the scene format's `VERSION` — files already on disk say
/// which shape they use. Bevy chose a path as the canonical identity for the
/// same reason we do, and deferred UUIDs; Godot and Unity pay for a sidecar
/// per asset to survive a rename, which needs an editor that manages moves.
///
/// Private, so adding a variant is not a breaking change to this crate's API.
#[derive(Serialize, Deserialize)]
#[serde(rename = "AssetPath")]
enum Repr {
    Path(String),
}

impl Serialize for AssetPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Repr::Path(self.0.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for AssetPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Routed through `new` rather than derived. A derived impl would skip
        // the constructor, and with it every check above — on exactly the path
        // a scene file takes, which is the only path that matters here.
        let Repr::Path(raw) = Repr::deserialize(deserializer)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p voltra-assets`
Expected: PASS, 27 tests.

- [ ] **Step 6: Verify the whole workspace**

Run, in order:
```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: clippy prints no warnings; every test passes.

- [ ] **Step 7: Commit**

```sh
git add crates/voltra-assets/src/
git commit -F- <<'EOF'
feat(assets): add a root-relative asset path

A scene file is external input. `AssetPath::new` rejects absolute paths,
volume prefixes, UNC paths, NTFS stream names and any `..`, so a value of
the type cannot name a file outside the asset root. `Deserialize` is written
by hand to route through the same constructor — a derived one would skip
every check on the only path that matters.
EOF
```

---

### Task 4: The placeholder's pixels

**Files:**
- Create: `crates/voltra-assets/src/placeholder.rs`
- Modify: `crates/voltra-assets/src/lib.rs`
- Test: unit tests inside `crates/voltra-assets/src/placeholder.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `voltra_assets::placeholder::SIZE: u32` (value `8`) and `voltra_assets::placeholder::rgba() -> Vec<u8>`, returning `SIZE * SIZE * 4` bytes.

This module is pure pixel arithmetic, so it needs no GPU and its tests run everywhere.

- [ ] **Step 1: Write the failing test**

Create `crates/voltra-assets/src/placeholder.rs` with only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MAGENTA: [u8; 4] = [255, 0, 255, 255];
    const BLACK: [u8; 4] = [0, 0, 0, 255];

    fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * SIZE + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    }

    #[test]
    fn it_is_the_right_number_of_bytes() {
        assert_eq!(rgba().len(), (SIZE * SIZE * 4) as usize);
    }

    #[test]
    fn it_starts_magenta() {
        assert_eq!(pixel(&rgba(), 0, 0), MAGENTA);
    }

    #[test]
    fn the_next_cell_across_is_black() {
        // A checker, not a flat fill. A flat magenta square could be mistaken
        // for a sprite someone actually tinted magenta.
        let pixels = rgba();
        assert_eq!(pixel(&pixels, CELL, 0), BLACK);
        assert_eq!(pixel(&pixels, 0, CELL), BLACK);
        assert_eq!(pixel(&pixels, CELL, CELL), MAGENTA);
    }

    #[test]
    fn a_cell_is_one_colour_throughout() {
        let pixels = rgba();
        for y in 0..CELL {
            for x in 0..CELL {
                assert_eq!(pixel(&pixels, x, y), MAGENTA, "at {x},{y}");
            }
        }
    }

    #[test]
    fn every_pixel_is_opaque() {
        // A transparent placeholder is an invisible one, which defeats it.
        for chunk in rgba().chunks_exact(4) {
            assert_eq!(chunk[3], 255);
        }
    }

    #[test]
    fn both_colours_appear() {
        let pixels = rgba();
        let magenta = pixels.chunks_exact(4).filter(|c| *c == MAGENTA).count();
        let black = pixels.chunks_exact(4).filter(|c| *c == BLACK).count();
        assert_eq!(magenta, black, "the checker should be evenly split");
    }
}
```

Add `pub mod placeholder;` to `crates/voltra-assets/src/lib.rs`, alphabetically between `path` and `store`. It gets no `pub use` — it is reached as `placeholder::rgba`, because a bare `rgba` at the crate root says nothing.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p voltra-assets`
Expected: FAIL to compile, `cannot find value SIZE in this scope`.

- [ ] **Step 3: Write the implementation**

At the top of `crates/voltra-assets/src/placeholder.rs`, above the test module:

```rust
//! The texture drawn in place of one that could not be loaded.

/// Width and height of the placeholder, in pixels.
pub const SIZE: u32 = 8;

/// Side of one check, in pixels.
pub const CELL: u32 = 4;

/// Magenta-and-black checks, RGBA, row-major from the top-left.
///
/// Magenta because nothing in real art is this colour by accident, and checks
/// rather than a flat fill because a flat magenta square could pass for a
/// sprite someone deliberately tinted. The alternative already in the tree —
/// binding the 1x1 white texture — makes a broken path look exactly like a
/// sprite with no texture at all, which hides the failure everywhere but the
/// log.
pub fn rgba() -> Vec<u8> {
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let magenta = (x / CELL + y / CELL) % 2 == 0;
            pixels.extend_from_slice(if magenta {
                &[255, 0, 255, 255]
            } else {
                &[0, 0, 0, 255]
            });
        }
    }
    pixels
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p voltra-assets`
Expected: PASS, 33 tests.

- [ ] **Step 5: Verify the whole workspace**

Run, in order:
```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: clippy prints no warnings; every test passes.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-assets/src/
git commit -m "feat(assets): add the missing-texture checker"
```

---

### Task 5: `Textures` — the root, the cache, the loading

**Files:**
- Create: `crates/voltra-assets/src/textures.rs`
- Create: `crates/voltra-assets/tests/common/mod.rs`
- Create: `crates/voltra-assets/tests/headless_textures.rs`
- Modify: `crates/voltra-assets/src/lib.rs`

**Interfaces:**
- Consumes: `Assets<Texture>`, `Handle<Texture>`, `AssetPath`, `AssetError`, `placeholder::{SIZE, rgba}` — all as produced by Tasks 1–4.
- Produces: `voltra_assets::Textures` with:
  - `Textures::new(device: &wgpu::Device, queue: &wgpu::Queue, root: impl Into<PathBuf>) -> Self`
  - `load(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, path: &AssetPath) -> Handle<Texture>` — infallible
  - `get(&self, handle: Handle<Texture>) -> &Texture`
  - `placeholder(&self) -> Handle<Texture>`
  - `root(&self) -> &Path`
  - `len(&self) -> usize`, `is_empty(&self) -> bool`

- [ ] **Step 1: Write the shared test scaffolding**

Create `crates/voltra-assets/tests/common/mod.rs`. `headless_device` is copied from `voltra-render/tests/common/mod.rs` rather than shared, because integration test crates cannot depend on another crate's test modules:

```rust
//! Shared scaffolding for the headless GPU tests.
//!
//! Each integration test is its own binary, so anything used by only one of
//! them looks dead to the others. That is what the blanket allow is for.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use voltra_render::wgpu;

/// Returns `None` when the machine has no usable adapter, so a CI runner
/// without a GPU skips rather than fails.
pub fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;

    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("headless-test-device"),
        ..Default::default()
    }))
    .ok()
}

/// A fresh directory under the system temp dir, unique per call.
pub fn scratch_root() -> PathBuf {
    // Unique per call so tests running in parallel cannot see each other's
    // files. Nothing cleans these up; they are a few hundred bytes each.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("voltra-assets-{nanos}-{:?}", std::thread::current().id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Writes a real PNG of `width` x `height` opaque red at `root/name`.
///
/// Encodes through `PngEncoder` rather than `DynamicImage::save_with_format`.
/// The workspace pins `image` with `default-features = false, features =
/// ["png"]`, and the convenience `save*` helpers sit behind feature gates that
/// set does not necessarily turn on; the encoder is exactly what the `png`
/// feature provides.
pub fn write_png(root: &Path, name: &str, width: u32, height: u32) {
    use image::ImageEncoder;

    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("asset subdirectory");
    }

    let pixels: Vec<u8> = (0..width * height)
        .flat_map(|_| [255u8, 0, 0, 255])
        .collect();

    let file = std::fs::File::create(&path).expect("creating the test PNG");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .expect("encoding the test PNG");
}
```

- [ ] **Step 2: Write the failing test**

Create `crates/voltra-assets/tests/headless_textures.rs`:

```rust
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
    let handle = textures.load(&device, &queue, &AssetPath::new("broken.png").expect("valid"));

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
```

Add to `crates/voltra-assets/src/lib.rs`, keeping both lists alphabetical:

```rust
pub mod error;
pub mod handle;
pub mod path;
pub mod placeholder;
pub mod store;
pub mod textures;

pub use error::AssetError;
pub use handle::Handle;
pub use path::AssetPath;
pub use store::Assets;
pub use textures::Textures;
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p voltra-assets --test headless_textures`
Expected: FAIL to compile, `unresolved import voltra_assets::Textures`.

- [ ] **Step 4: Write the implementation**

Create `crates/voltra-assets/src/textures.rs`:

```rust
//! Textures, keyed by the path a scene file names them with.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voltra_render::wgpu::{Device, Queue};
use voltra_render::{Filter, Texture};

use crate::error::AssetError;
use crate::handle::Handle;
use crate::path::AssetPath;
use crate::placeholder;
use crate::store::Assets;

/// Loads textures from an asset root and hands out shared handles to them.
///
/// Two entities naming one PNG get one handle and therefore one GPU texture.
/// That is the whole point of the type.
pub struct Textures {
    root: PathBuf,
    store: Assets<Texture>,
    by_path: HashMap<AssetPath, Handle<Texture>>,
    placeholder: Handle<Texture>,
}

impl Textures {
    /// Builds a store rooted at `root`, with the placeholder already in it.
    pub fn new(device: &Device, queue: &Queue, root: impl Into<PathBuf>) -> Self {
        let mut store = Assets::new();
        let texture = Texture::from_rgba8(
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
        .expect("the placeholder's pixel count matches its declared size");

        let placeholder = store.insert(texture);

        Self {
            root: root.into(),
            store,
            by_path: HashMap::new(),
            placeholder,
        }
    }

    /// The handle for `path`, loading it if this is the first time.
    ///
    /// Infallible on purpose: a scene naming a texture that will not load must
    /// still open and still draw. A failure logs once and returns the
    /// placeholder, and that answer is cached like any other — otherwise a
    /// broken path re-reads the disk and warns on every frame it is drawn.
    pub fn load(&mut self, device: &Device, queue: &Queue, path: &AssetPath) -> Handle<Texture> {
        if let Some(handle) = self.by_path.get(path) {
            return *handle;
        }

        let handle = match self.read(device, queue, path) {
            Ok(texture) => self.store.insert(texture),
            Err(e) => {
                log::warn!("{e}; drawing the missing-texture checker instead");
                self.placeholder
            }
        };

        self.by_path.insert(path.clone(), handle);
        handle
    }

    /// The texture `handle` names.
    ///
    /// Every handle this type hands out is valid: the placeholder is inserted
    /// at construction and nothing here ever removes from the store. Only a
    /// handle forged from a different store can reach the `expect`.
    pub fn get(&self, handle: Handle<Texture>) -> &Texture {
        self.store
            .get(handle)
            .expect("Textures never removes, so every handle it issued resolves")
    }

    /// The checker drawn in place of a texture that would not load.
    pub fn placeholder(&self) -> Handle<Texture> {
        self.placeholder
    }

    /// The directory every [`AssetPath`] is resolved against.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// How many textures are stored, the placeholder included.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }

    /// Reads and uploads one file. The only place this type touches the disk.
    fn read(&self, device: &Device, queue: &Queue, path: &AssetPath) -> Result<Texture, AssetError> {
        // Safe to join because `AssetPath` has already refused anything that
        // could climb out of the root. That check lives in the constructor
        // precisely so it cannot be forgotten here.
        let full = self.root.join(path.as_str());

        let bytes = std::fs::read(&full).map_err(|source| AssetError::Read {
            path: full.clone(),
            source,
        })?;

        Texture::from_png(device, queue, path.as_str(), &bytes, Filter::Linear).map_err(|source| {
            AssetError::Decode {
                path: full,
                source,
            }
        })
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p voltra-assets`
Expected: PASS. The seven headless tests either pass or print `no GPU adapter; skipping`; the 33 unit tests pass either way.

- [ ] **Step 6: Verify the whole workspace**

Run, in order:
```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: clippy prints no warnings; every test passes.

- [ ] **Step 7: Commit**

```sh
git add crates/voltra-assets/
git commit -F- <<'EOF'
feat(assets): cache textures by asset path

Two entities naming one PNG now get one handle and one GPU texture. A path
that cannot be read or decoded logs once and resolves to the checker, and
that answer is cached like any other — a broken path must not re-read the
disk on every frame it is drawn.
EOF
```

---

### Task 6: Record it in ARCHITECTURE.md

**Files:**
- Modify: `docs/ARCHITECTURE.md` — the layer diagram, the "Current crates" table, the "Planned crates" table, and a new entry under `## Decisions`

**Interfaces:**
- Consumes: the finished crate from Tasks 1–5.
- Produces: nothing in code.

- [ ] **Step 1: Update the layer diagram**

Replace the ASCII diagram under `## Layers` with exactly this. The only change is the `voltra-assets` box and the arrow from it into `voltra-render`; every other box, label and alignment is as it was.

```
        ┌──────────────────┐
        │  voltra-editor   │  binary — wires everything together
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │   voltra-core    │  platform: event loop, window, input, time
        │  (owns winit)    │
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │   voltra-scene   │  components and the geometry they become
        └─┬──────────┬───┬─┘
          │          │   │
┌─────────▼────┐  ┌──▼───▼───────────┐  ┌──────────────────┐
│  voltra-ecs  │  │  voltra-render   │◄─┤  voltra-assets   │
│  (no deps)   │  │  (owns wgpu)     │  │  cache, loading  │
└──────────────┘  └──────────────────┘  └──────────────────┘
```

Then add this paragraph immediately after the existing one that begins "`voltra-scene` is the only crate that knows about both entities and vertices":

```
`voltra-assets` points into `voltra-render` because the thing it caches *is* a
GPU texture — caching decoded bytes and re-uploading per sprite would cache the
cheap half. It reaches `Device` and `Queue` through `voltra_render::wgpu` and
declares no `wgpu` of its own, so the one-crate-per-backend rule holds.
```

- [ ] **Step 2: Update the crate tables**

Add a row to the `### Current crates` table, in the existing order (it goes after `voltra-ecs`):

| Crate | Owns | Key types |
| --- | --- | --- |
| `voltra-assets` | Asset identity, the texture cache, loading from the asset root | `Handle`, `Assets`, `AssetPath`, `Textures` |

Delete the `voltra-assets` row from `### Planned crates` — it is no longer planned. Leave the `xtask` row alone.

- [ ] **Step 3: Add the decision entry**

Insert under `## Decisions`, immediately after "A scene save replaces the file or leaves it alone" and immediately before "### wgpu 30 API notes". Match the voice of the surrounding entries: bold lead-in sentences stating the decision, prose giving the reason, a `#### Rejected` subsection, named external precedent.

Heading: `### An asset is named by its path, and a bad name draws magenta`

It must carry:

- **A path is the identity, in an enum.** Bevy chose `AssetPath` as canonical deliberately and deferred UUIDs — *"everyone uses filesystems... to manage their asset source files"* ([bevyengine/bevy#8624](https://github.com/bevyengine/bevy/pull/8624)). The enum shape is what keeps a `Uuid` variant addable later without changing the scene format's `VERSION`, which matters because ARCHITECTURE.md already records that a file format is the one thing that cannot be refactored freely.
- **`AssetPath` is a security boundary, not a newtype for tidiness.** A scene file is external input; a raw string in one could name a file anywhere on the machine, and merely opening the scene would read it. The check lives in the constructor and `Deserialize` routes through it by hand, because a derived impl would skip it on the only path that matters.
- **The handle is an index and a generation, not a refcount.** Same shape as `voltra_ecs::Entity`, so the engine has one idea of a handle. Bevy and Godot refcount and get eviction for it; here an `Arc` in `Sprite` would cost its `Copy`, which `batch.rs` and `pick.rs` rely on in per-frame loops, and no measured memory problem is asking for eviction. `Assets::remove` exists so the generation is real and testable; when to call it is the part still deferred.
- **A failure draws a magenta checker and the scene still opens.** Same value the format already states twice — an unknown component is preserved, a failed Open changes nothing. A path is user data, not a build invariant, so a moved PNG must not make a scene unopenable. The 1×1 white texture already in the tree was rejected as the placeholder: it is indistinguishable from a sprite with no texture, which hides the failure everywhere but the log.
- **Loading is synchronous, and that is not a shortcut to undo.** There is no task system, and building one for this would be the subproject rather than the module. The placeholder is exactly what an async load must return while bytes are in flight, so the call site does not change shape when async arrives.
- A `#### Rejected` subsection covering: refcounted handles (above); a UUID sidecar per asset, Unity's `.meta` and Godot 4.4's `uid://`, which survive a rename but cost a sidecar and an import step this engine has no editor to manage, and whose failure mode is a silently dead reference when the sidecar is lost; and failing the scene load on a missing texture, which would contradict Open's rollback.

Wrap at 80 columns. Change nothing else in the file.

- [ ] **Step 4: Verify**

Run: `cargo test --workspace`
Expected: PASS. Nothing in code changed, but the doc references type names — confirm every name it mentions exists:

```sh
git grep -n "pub struct Textures\|pub struct AssetPath\|pub struct Handle\|pub struct Assets\|pub fn remove" -- crates/voltra-assets/src
```

- [ ] **Step 5: Commit**

```sh
git add docs/ARCHITECTURE.md
git commit -m "docs: record the asset store decisions"
```

---

## Definition of done

- `voltra-assets` exists with six modules, each describable without the word "and".
- `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- `cargo test --workspace` passes, and passes on a machine with no GPU adapter.
- No crate but `voltra-render` names `wgpu` in its `Cargo.toml`.
- `Sprite`, `batch.rs`, the renderer and the editor are untouched. Nothing in this branch is visible on screen — that is 12b.
