# Scene Serialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Save a world to a readable, diffable `.ron` file and load it back, without ever destroying data the running build does not understand.

**Architecture:** A `ComponentRegistry` maps a component's name to save and load functions specialised for its type, so serialization walks the registry rather than the ECS's type-erased storages — `voltra-ecs` keeps its zero dependencies and gains no new API. Entities persist through a `SceneId(Uuid)` component; unknown components survive as `ron::value::RawValue`s — the RON text itself — and are written back untouched.

**Tech Stack:** Rust, `serde` 1.0.229, `ron` 0.12.2, `uuid` 1.24.0 (v7 + serde).

Spec: [docs/superpowers/specs/2026-08-07-scene-serialization-design.md](../specs/2026-08-07-scene-serialization-design.md)

## Global Constraints

- Branch is `feature/scene-serialization`, already created off `main`. Never commit to `main`.
- **This is 2D.** Per CLAUDE.md's scope section: a 2D transform is written as two floats, never three. No z-axis, no 3D scaffolding, nothing named for an axis it is not.
- **`voltra-ecs` is not touched by this plan.** It has zero dependencies by design and ARCHITECTURE.md records that as worth keeping. If a task seems to need a change there, stop and report — it means the registry design has a hole.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all pass before any commit. Clippy warnings are errors.
- No `unwrap()` outside `#[cfg(test)]`. `expect("why this cannot fail")` only where the invariant is real. Errors propagate as `Result`.
- Log through `log`, never `println!`.
- **All dependency versions live in the root `[workspace.dependencies]`.** Member crates write `dep.workspace = true`. Never a literal version in a member crate.
- One concept per file. Split a module into a directory past roughly 300 lines or a second concept, preferring `foo.rs` + `foo/` over `foo/mod.rs`.
- Unit tests live in the file they test, in `#[cfg(test)] mod tests`.
- Conventional Commits, scope = crate minus the `voltra-` prefix, imperative subject **50 characters or fewer** — count them.
- Every commit ends with the trailer `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- **Git: one new commit per task, plain porcelain only.** No amend, no rebase, no `reset --hard`, and never the plumbing — no `hash-object`, `write-tree`, `commit-tree`, `update-ref`. If a git operation will not work with porcelain, stop and report.
- The editor is a GUI app with an infinite loop — never run it in the foreground. Launch it detached, wait, read the log, kill it with `taskkill` (`pkill` is not available here).

### Read this before trusting any code in this plan

The plan's author could not read `ron` or `uuid`: neither was a dependency when it
was written, so there was no vendored source on disk. The `ron` snippets here came
from a documentation lookup that may describe an **older minor version than the
0.12.2 this plan pins.**

Task 1 adds the dependencies. **From Task 2 onward, verify every `ron` and `uuid`
call against the vendored source before relying on this plan's spelling of it:**

```
~/.cargo/registry/src/index.crates.io-*/ron-0.12.*/src/
~/.cargo/registry/src/index.crates.io-*/uuid-1.*/src/
```

If a signature differs, use what the source says and **report the difference** —
do not silently work around it. This plan's author has a track record on this
exact branch of hand-writing code into plans that did not match reality.

### Facts about the existing code every task depends on

- `World::query::<T>()` yields `(Entity, &T)`; `World::query2::<A, B>()` yields `(Entity, &A, &B)`.
- `World::get::<T>(entity) -> Option<&T>`, `World::insert::<T>(entity, value) -> Option<T>`, `World::spawn() -> Entity`.
- `Entity::index() -> u32`, `Entity::generation() -> u32`. Both are recycled at runtime and must never reach a file.
- `Transform` is `translation: Vec2`, `rotation: f32`, `scale: Vec2`. `Sprite` is `color: [f32; 4]`, `sort_order: i32`.
- `voltra-scene` already re-exports `voltra_render::glam`. `glam` is in the workspace with the `bytemuck` feature; **`serde` support for `Vec2` needs glam's `serde` feature adding.**

---

### Task 1: Dependencies, `serde` derives, and the identity components

**Files:**
- Modify: `Cargo.toml` (root — `[workspace.dependencies]`)
- Modify: `crates/voltra-scene/Cargo.toml`
- Modify: `crates/voltra-scene/src/transform.rs`
- Modify: `crates/voltra-scene/src/sprite.rs`
- Create: `crates/voltra-scene/src/scene_id.rs`
- Modify: `crates/voltra-scene/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `Transform` and `Sprite` both `#[derive(Serialize, Deserialize)]`
  - `pub struct SceneId(pub Uuid)`, with `SceneId::new() -> Self` using UUID **v7**
  - `pub struct UnknownComponents(pub BTreeMap<String, Box<ron::value::RawValue>>)`, `Default`
  - All three re-exported from `voltra_scene`

- [ ] **Step 1: Add the dependencies to the root manifest**

In the root `Cargo.toml`, inside `[workspace.dependencies]`:

```toml
serde = { version = "1.0.229", features = ["derive"] }
# Scene files are authoring artefacts: read in a pull request, merged in git.
ron = "0.12.2"
# v7 rather than v4: the timestamp in the high bits means sorting by id sorts by
# creation, so a scene file is deterministic *and* appends new entities at the end.
uuid = { version = "1.24.0", features = ["v7", "serde"] }
```

`glam` is already there but without serde. Add the feature to the existing line, keeping `bytemuck`:

```toml
glam = { version = "0.33.3", features = ["bytemuck", "serde"] }
```

- [ ] **Step 2: Add them to `voltra-scene`**

In `crates/voltra-scene/Cargo.toml`, under `[dependencies]`:

```toml
serde.workspace = true
ron.workspace = true
uuid.workspace = true
```

- [ ] **Step 3: Confirm the dependency tree still has one `wgpu` and one `glam`**

```sh
cargo tree --workspace -d
```

Expected: no duplicate `wgpu` and no duplicate `glam`. A duplicate `glam` would mean two incompatible `Vec2` types and is a hard failure — stop and report rather than proceeding.

- [ ] **Step 4: Derive `Serialize` and `Deserialize` on the two components**

In `crates/voltra-scene/src/transform.rs`, extend the existing derive:

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Transform {
```

In `crates/voltra-scene/src/sprite.rs`, the same on `Sprite`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sprite {
```

Do not change any field, doc comment or default. This step adds two derives and nothing else.

- [ ] **Step 5: Write the failing tests for the identity components**

Create `crates/voltra-scene/src/scene_id.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = SceneId::new();
        let b = SceneId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn ids_sort_by_creation_order() {
        // The whole reason for v7 over v4. Sorting a scene file by id has to be
        // the same as sorting it by when each entity was made, or new entities
        // land in the middle of the diff instead of at the end.
        let mut ids: Vec<SceneId> = (0..64).map(|_| SceneId::new()).collect();
        let created = ids.clone();
        ids.sort();
        assert_eq!(ids, created, "v7 ids must already be in creation order");
    }

    #[test]
    fn unknown_components_start_empty() {
        assert!(UnknownComponents::default().0.is_empty());
    }
}
```

- [ ] **Step 6: Run them and confirm they fail**

```sh
cargo test -p voltra-scene --lib scene_id
```

Expected: compile error — `cannot find type 'SceneId' in this scope`, and the same for `UnknownComponents`.

- [ ] **Step 7: Write the implementation**

Above that test module in `crates/voltra-scene/src/scene_id.rs`:

```rust
//! What identifies an entity in a scene file, and what to do with the parts of
//! that file this build does not understand.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A stable identity that survives saving, loading and reordering a file.
///
/// `Entity` is an index and a generation, both recycled by the allocator at
/// runtime — allocator bookkeeping, not identity, and it must never reach a
/// file. Only entities carrying a `SceneId` are saved, so a transient runtime
/// spawn opts out simply by not having one, and no exclusion list has to exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SceneId(pub Uuid);

impl SceneId {
    /// A fresh identity.
    ///
    /// UUID **v7**, which carries a timestamp in its high bits, so ordering by
    /// id is ordering by creation. That is what lets a scene file be both
    /// deterministic and append-only in a diff; v4 forces a choice between the
    /// two.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for SceneId {
    fn default() -> Self {
        Self::new()
    }
}

/// Components read from a file that no registered type claimed.
///
/// Held unparsed and written straight back out on save. A build that does not
/// know what `Physics` is can still open a scene, move a sprite and save without
/// deleting it — which is the failure mode this exists to prevent, and the one
/// Unity is criticised for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UnknownComponents(pub BTreeMap<String, Box<ron::value::RawValue>>);
```

Verify `Uuid::now_v7()` exists with that name in the vendored `uuid` source before relying on it, and report if it differs.

- [ ] **Step 8: Declare and re-export**

In `crates/voltra-scene/src/lib.rs`, add the module in alphabetical order and re-export both types alongside the existing ones:

```rust
pub mod scene_id;
```

```rust
pub use scene_id::{SceneId, UnknownComponents};
```

- [ ] **Step 9: Run the tests and the workspace**

```sh
cargo test -p voltra-scene --lib
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all pass, no warnings.

- [ ] **Step 10: Commit**

```sh
git add Cargo.toml Cargo.lock crates/voltra-scene
git commit -m "feat(scene): add scene identity and serde derives

SceneId is UUIDv7 so ordering by id is ordering by creation, which keeps a
scene file deterministic without scattering new entities through the diff.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: The component registry

**Files:**
- Create: `crates/voltra-scene/src/format.rs`
- Create: `crates/voltra-scene/src/format/error.rs`
- Create: `crates/voltra-scene/src/format/registry.rs`
- Modify: `crates/voltra-scene/src/lib.rs`

**Interfaces:**
- Consumes from Task 1: `SceneId`, `UnknownComponents`, the `serde` derives on `Transform` and `Sprite`.
- Produces:
  - `pub enum SceneError` with variants `Io(std::io::Error)`, `Parse(ron::error::SpannedError)`, `Serialize(ron::Error)`, `UnsupportedVersion { found: u32, supported: u32 }`, `Component { name: String, source: ron::Error }`
  - `pub struct ComponentRegistry` with `new()`, `with_defaults()`, `register::<T: Serialize + DeserializeOwned + 'static>(&mut self, name: &'static str)`, `names(&self) -> impl Iterator<Item = &'static str>`
  - `pub(crate) fn save_one(&self, world: &World, entity: Entity, name: &str) -> Option<Result<Box<ron::value::RawValue>, SceneError>>`
  - `pub(crate) fn load_one(&self, world: &mut World, entity: Entity, name: &str, value: &ron::value::RawValue) -> Option<Result<(), SceneError>>`

Both `*_one` return `None` when the name is not registered — that is how the caller distinguishes "unknown, preserve it" from "known, and it failed".

- [ ] **Step 1: Write the error type**

Create `crates/voltra-scene/src/format/error.rs`:

```rust
//! What can go wrong saving or loading a scene.

use std::fmt;

/// A failure while reading or writing a scene file.
///
/// The variants are separate because they call for different responses. A
/// missing file is the user's problem; malformed RON is the file's; a component
/// that fails to deserialize is *ours*, because the name being registered says
/// this build claims to understand it.
#[derive(Debug)]
pub enum SceneError {
    Io(std::io::Error),
    Parse(ron::error::SpannedError),
    Serialize(ron::Error),
    UnsupportedVersion { found: u32, supported: u32 },
    /// A registered component whose stored data does not fit its type.
    ///
    /// Deliberately not treated as an unknown component. Unknown means "not
    /// mine, do not touch"; this means "mine, and broken", and swallowing it
    /// would silently drop data on the next save.
    Component { name: String, source: ron::Error },
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "scene file i/o failed: {e}"),
            Self::Parse(e) => write!(f, "scene file is not valid RON: {e}"),
            Self::Serialize(e) => write!(f, "could not write the scene: {e}"),
            Self::UnsupportedVersion { found, supported } => write!(
                f,
                "scene file is version {found}, this build supports {supported}"
            ),
            Self::Component { name, source } => {
                write!(f, "component `{name}` could not be read: {source}")
            }
        }
    }
}

impl std::error::Error for SceneError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::Serialize(e) | Self::Component { source: e, .. } => Some(e),
            Self::UnsupportedVersion { .. } => None,
        }
    }
}

impl From<std::io::Error> for SceneError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
```

`ron::error::SpannedError` and `ron::Error` are the plan author's best guess at
0.12's names. **Check the vendored source and correct them if they differ**, then
report what you found.

- [ ] **Step 2: Write the failing registry tests**

Create `crates/voltra-scene/src/format/registry.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Sprite, Transform};
    use voltra_render::glam::Vec2;

    fn world_with_one_sprite() -> (World, Entity) {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Transform::from_translation(Vec2::new(1.0, 2.0)));
        world.insert(e, Sprite::default().with_sort_order(3));
        (world, e)
    }

    #[test]
    fn the_defaults_cover_the_built_in_components() {
        let registry = ComponentRegistry::with_defaults();
        let names: Vec<_> = registry.names().collect();
        assert!(names.contains(&"Transform"), "got {names:?}");
        assert!(names.contains(&"Sprite"), "got {names:?}");
    }

    #[test]
    fn an_unregistered_name_is_not_an_error() {
        // The caller has to be able to tell "I do not know this" from "I know it
        // and it broke", because the first is preserved and the second is not.
        let (world, entity) = world_with_one_sprite();
        let registry = ComponentRegistry::with_defaults();
        assert!(registry.save_one(&world, entity, "Physics").is_none());
    }

    #[test]
    fn a_registered_component_round_trips_through_a_value() {
        let (world, entity) = world_with_one_sprite();
        let registry = ComponentRegistry::with_defaults();

        let value = registry
            .save_one(&world, entity, "Sprite")
            .expect("Sprite is registered")
            .expect("saving a valid Sprite cannot fail");

        let mut target = World::new();
        let copy = target.spawn();
        registry
            .load_one(&mut target, copy, "Sprite", &value)
            .expect("Sprite is registered")
            .expect("the value came from a Sprite, so it must load as one");

        assert_eq!(target.get::<Sprite>(copy), world.get::<Sprite>(entity));
    }

    #[test]
    fn an_entity_without_the_component_saves_nothing() {
        // Registered, but this entity does not have one. Distinct from both
        // "unknown name" and "failed" — the map simply has no entry.
        let mut world = World::new();
        let bare = world.spawn();
        let registry = ComponentRegistry::with_defaults();
        assert!(registry.save_one(&world, bare, "Sprite").is_none());
    }

    #[test]
    fn a_registered_component_with_wrong_data_is_an_error() {
        let mut world = World::new();
        let entity = world.spawn();
        let registry = ComponentRegistry::with_defaults();

        // A string where a Sprite struct belongs.
        let nonsense: ron::Value = ron::from_str("\"not a sprite\"").expect("valid RON");

        let result = registry
            .load_one(&mut world, entity, "Sprite", &nonsense)
            .expect("Sprite is registered");
        assert!(
            matches!(result, Err(SceneError::Component { .. })),
            "expected a Component error, got {result:?}"
        );
    }
}
```

- [ ] **Step 3: Run them and confirm they fail**

```sh
cargo test -p voltra-scene --lib format::registry
```

Expected: compile error — `cannot find type 'ComponentRegistry' in this scope`.

- [ ] **Step 4: Write the registry**

Above the test module in `crates/voltra-scene/src/format/registry.rs`:

```rust
//! Which component types a scene file can carry, and how each converts.
//!
//! The registry is what lets serialization stay out of `voltra-ecs`. That crate
//! stores components type-erased behind a `TypeId`, with no way to enumerate the
//! types or reach one without knowing it at compile time — so the list of types
//! has to come from somewhere else. This is that somewhere: registering a type
//! captures functions that already know `T`, and saving then walks the registry
//! rather than the storages.

use serde::de::DeserializeOwned;
use serde::Serialize;
use voltra_ecs::{Entity, World};

use super::error::SceneError;
use crate::{Sprite, Transform};

/// One component type's name and the two conversions that go with it.
struct Entry {
    name: &'static str,
    save: fn(&World, Entity) -> Option<Result<Box<ron::value::RawValue>, SceneError>>,
    load: fn(&mut World, Entity, &ron::value::RawValue) -> Result<(), SceneError>,
}

/// The component types a scene file may contain.
///
/// Registration is explicit. A type that is not registered is not persisted,
/// which is a property worth choosing rather than inheriting.
pub struct ComponentRegistry {
    entries: Vec<Entry>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Every component type this crate defines.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register::<Transform>("Transform");
        registry.register::<Sprite>("Sprite");
        registry
    }

    /// Adds a component type under `name`, which is what appears in the file.
    ///
    /// The name is chosen rather than derived from the Rust path, so renaming a
    /// type in code does not silently invalidate every scene on disk.
    pub fn register<T>(&mut self, name: &'static str)
    where
        T: Serialize + DeserializeOwned + 'static,
    {
        self.entries.push(Entry {
            name,
            save: |world, entity| {
                let component = world.get::<T>(entity)?;
                Some(to_value(component))
            },
            load: |world, entity, value| {
                let component: T = value
                    .clone()
                    .into_rust()
                    .map_err(|source| SceneError::Component {
                        name: std::any::type_name::<T>().to_owned(),
                        source,
                    })?;
                world.insert(entity, component);
                Ok(())
            },
        });
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|e| e.name)
    }

    /// The entity's value for `name`, or `None` when the name is unregistered
    /// **or** the entity simply has no such component.
    pub(crate) fn save_one(
        &self,
        world: &World,
        entity: Entity,
        name: &str,
    ) -> Option<Result<Box<ron::value::RawValue>, SceneError>> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        (entry.save)(world, entity)
    }

    /// Inserts `value` as `name`, or `None` when the name is unregistered.
    ///
    /// The outer `Option` is the caller's signal to preserve the value untouched
    /// instead of failing.
    pub(crate) fn load_one(
        &self,
        world: &mut World,
        entity: Entity,
        name: &str,
        value: &ron::value::RawValue,
    ) -> Option<Result<(), SceneError>> {
        let entry = self.entries.iter().find(|e| e.name == name)?;
        Some((entry.load)(world, entity, value))
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// A component as a `ron::Value`, via RON text.
///
/// `Value` is the one representation both known and unknown components share,
/// which is what lets a single code path write both.
fn to_value<T: Serialize>(component: &T) -> Result<ron::Value, SceneError> {
    let text = ron::to_string(component).map_err(SceneError::Serialize)?;
    ron::from_str(&text).map_err(|e| SceneError::Parse(e))
}
```

**Two things in here are the plan author's guess and must be checked against the
vendored `ron` 0.12 source before you trust them:** that `ron::Value` has
`into_rust::<T>()` returning a `Result` whose error is `ron::Error`, and that
`ron::to_string` / `ron::from_str` have those names and error types. If the real
API differs, use it and report the difference. There may also be a direct
`T -> Value` conversion that avoids the text round trip — if there is, prefer it
and say so.

- [ ] **Step 5: Wire up the module tree**

Create `crates/voltra-scene/src/format.rs`:

```rust
//! Reading and writing a scene as a file.

pub mod error;
pub mod registry;

pub use error::SceneError;
pub use registry::ComponentRegistry;
```

And in `crates/voltra-scene/src/lib.rs`, add the module in alphabetical order and re-export:

```rust
pub mod format;
```

```rust
pub use format::{ComponentRegistry, SceneError};
```

- [ ] **Step 6: Run the tests and the workspace**

```sh
cargo test -p voltra-scene --lib format
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all pass, no warnings.

- [ ] **Step 7: Commit**

```sh
git add crates/voltra-scene/src
git commit -m "feat(scene): add the component registry

voltra-ecs stores components type-erased with no way to enumerate them, so the
list of serializable types has to come from somewhere that knows T. Registering
captures functions that already do.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: The file model and saving

**Files:**
- Create: `crates/voltra-scene/src/format/save.rs`
- Modify: `crates/voltra-scene/src/format.rs`

**Interfaces:**
- Consumes from Tasks 1 and 2: `SceneId`, `UnknownComponents`, `ComponentRegistry`, `SceneError`.
- Produces:
  - `pub const VERSION: u32 = 1;`
  - `pub struct SceneFile { pub version: u32, pub entities: Vec<EntityRecord> }`, `Serialize + Deserialize`
  - `pub struct EntityRecord { pub id: SceneId, pub components: BTreeMap<String, Box<ron::value::RawValue>> }`, `Serialize + Deserialize`
  - `pub fn to_scene_file(world: &World, registry: &ComponentRegistry) -> Result<SceneFile, SceneError>`
  - `pub fn save(world: &World, registry: &ComponentRegistry, path: &Path) -> Result<(), SceneError>`

- [ ] **Step 1: Write the failing tests**

Create `crates/voltra-scene/src/format/save.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Sprite, Transform};
    use voltra_render::glam::Vec2;

    fn world_with(count: usize) -> World {
        let mut world = World::new();
        for i in 0..count {
            let e = world.spawn();
            world.insert(e, SceneId::new());
            world.insert(e, Transform::from_translation(Vec2::new(i as f32, 0.0)));
            world.insert(e, Sprite::default());
        }
        world
    }

    #[test]
    fn an_empty_world_saves_an_empty_scene() {
        let file = to_scene_file(&World::new(), &ComponentRegistry::with_defaults())
            .expect("an empty world cannot fail to save");
        assert_eq!(file.version, VERSION);
        assert!(file.entities.is_empty());
    }

    #[test]
    fn an_entity_without_a_scene_id_is_skipped() {
        // Opting out of persistence is the absence of a SceneId, so no exclusion
        // list has to be kept anywhere.
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Transform::default());
        world.insert(e, Sprite::default());

        let file = to_scene_file(&world, &ComponentRegistry::with_defaults())
            .expect("saving cannot fail here");
        assert!(file.entities.is_empty(), "got {:?}", file.entities);
    }

    #[test]
    fn entities_are_ordered_by_id() {
        let world = world_with(8);
        let file = to_scene_file(&world, &ComponentRegistry::with_defaults())
            .expect("saving cannot fail here");

        let ids: Vec<_> = file.entities.iter().map(|e| e.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "entities must be written in SceneId order");
    }

    #[test]
    fn a_saved_entity_carries_its_registered_components() {
        let world = world_with(1);
        let file = to_scene_file(&world, &ComponentRegistry::with_defaults())
            .expect("saving cannot fail here");

        let keys: Vec<_> = file.entities[0].components.keys().cloned().collect();
        assert_eq!(keys, vec!["Sprite".to_owned(), "Transform".to_owned()]);
    }

    #[test]
    fn unknown_components_are_written_back() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, SceneId::new());
        world.insert(e, Sprite::default());

        let physics: ron::Value = ron::from_str("(mass: 5.0)").expect("valid RON");
        let mut unknown = UnknownComponents::default();
        unknown.0.insert("Physics".to_owned(), physics.clone());
        world.insert(e, unknown);

        let file = to_scene_file(&world, &ComponentRegistry::with_defaults())
            .expect("saving cannot fail here");

        assert_eq!(
            file.entities[0].components.get("Physics"),
            Some(&physics),
            "an unrecognised component must survive a save untouched"
        );
    }

    #[test]
    fn saving_the_same_world_twice_is_identical() {
        // The property the whole format rests on: a diff shows changes, never
        // the serializer changing its mind about ordering or spacing.
        let world = world_with(4);
        let registry = ComponentRegistry::with_defaults();
        let dir = std::env::temp_dir().join("voltra-scene-save-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let a = dir.join("a.ron");
        let b = dir.join("b.ron");

        save(&world, &registry, &a).expect("saving cannot fail here");
        save(&world, &registry, &b).expect("saving cannot fail here");

        assert_eq!(
            std::fs::read_to_string(&a).expect("written"),
            std::fs::read_to_string(&b).expect("written"),
        );
    }
}
```

- [ ] **Step 2: Run them and confirm they fail**

```sh
cargo test -p voltra-scene --lib format::save
```

Expected: compile error — `cannot find function 'to_scene_file' in this scope`.

- [ ] **Step 3: Write the implementation**

Above that test module:

```rust
//! Turning a world into a scene file.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use voltra_ecs::World;

use super::error::SceneError;
use super::registry::ComponentRegistry;
use crate::scene_id::{SceneId, UnknownComponents};

/// The only scene format version this build writes or reads.
///
/// Written from the first release. Adding a version field later means guessing
/// what a file without one meant.
pub const VERSION: u32 = 1;

/// A scene as it appears on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneFile {
    pub version: u32,
    pub entities: Vec<EntityRecord>,
}

/// One entity: its stable identity and every component it carries.
///
/// `BTreeMap` rather than `HashMap` so component names come out alphabetically
/// and two saves of the same world are byte-identical. A `HashMap` would reorder
/// between runs and fill the diff with noise that is not a change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityRecord {
    pub id: SceneId,
    pub components: BTreeMap<String, Box<ron::value::RawValue>>,
}

/// The formatting every save uses.
///
/// Defined once, on purpose. Two saves of the same world can only be identical
/// if there is exactly one configuration and nobody passes a variant of it.
/// `struct_names` is off: `SceneFile(` at the top of the file tells a reader
/// nothing they need.
fn pretty() -> ron::ser::PrettyConfig {
    ron::ser::PrettyConfig::new()
        .struct_names(false)
        .indentor("    ")
        .new_line("\n")
}

/// Every entity holding a [`SceneId`], in id order.
pub fn to_scene_file(
    world: &World,
    registry: &ComponentRegistry,
) -> Result<SceneFile, SceneError> {
    let mut entities: Vec<EntityRecord> = Vec::new();

    for (entity, id) in world.query::<SceneId>() {
        let mut components = BTreeMap::new();

        for name in registry.names() {
            if let Some(value) = registry.save_one(world, entity, name) {
                components.insert(name.to_owned(), value?);
            }
        }

        // Merged after the known ones so a component that has since become
        // registered wins over the stale copy kept from an older load.
        if let Some(unknown) = world.get::<UnknownComponents>(entity) {
            for (name, value) in &unknown.0 {
                components
                    .entry(name.clone())
                    .or_insert_with(|| value.clone());
            }
        }

        entities.push(EntityRecord {
            id: *id,
            components,
        });
    }

    // Ordering by id is ordering by creation, because the ids are UUIDv7. That
    // makes the file deterministic and puts new entities at the end rather than
    // in the middle of someone's diff.
    entities.sort_by_key(|record| record.id);

    Ok(SceneFile {
        version: VERSION,
        entities,
    })
}

/// Writes the world to `path`.
pub fn save(
    world: &World,
    registry: &ComponentRegistry,
    path: &Path,
) -> Result<(), SceneError> {
    let file = to_scene_file(world, registry)?;
    let text =
        ron::ser::to_string_pretty(&file, pretty()).map_err(SceneError::Serialize)?;
    std::fs::write(path, text)?;
    log::info!("saved {} entities to {}", file.entities.len(), path.display());
    Ok(())
}
```

**Check against the vendored `ron` 0.12 source before trusting:** that
`PrettyConfig` has `struct_names`, `indentor` and `new_line` with those names,
and that `ron::ser::to_string_pretty` takes `(&value, PrettyConfig)` and returns
`Result<String, ron::Error>`. Report any difference.

Also confirm `ron::Value` implements `Serialize` and `Deserialize` — the whole
`EntityRecord` depends on it. If it needs a feature flag on the `ron` dependency,
add it to the root manifest and say so.

- [ ] **Step 4: Export the new items**

In `crates/voltra-scene/src/format.rs`:

```rust
pub mod save;
```

```rust
pub use save::{save, to_scene_file, EntityRecord, SceneFile, VERSION};
```

- [ ] **Step 5: Run the tests and the workspace**

```sh
cargo test -p voltra-scene --lib format
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-scene/src
git commit -m "feat(scene): write a world to a scene file

Entities sort by SceneId, which is creation order because the ids are v7, and
component names sort alphabetically — so two saves of one world are identical
and a diff only ever shows a real change.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Loading, and preserving what is not understood

**Files:**
- Create: `crates/voltra-scene/src/format/load.rs`
- Modify: `crates/voltra-scene/src/format.rs`

**Interfaces:**
- Consumes from Tasks 1-3: everything above.
- Produces:
  - `pub fn from_scene_file(file: &SceneFile, registry: &ComponentRegistry, world: &mut World) -> Result<(), SceneError>`
  - `pub fn load(path: &Path, registry: &ComponentRegistry, world: &mut World) -> Result<(), SceneError>`

Both add to the world given rather than clearing it. Clearing is the caller's decision, and the editor's menu is where it belongs.

- [ ] **Step 1: Write the failing tests**

Create `crates/voltra-scene/src/format/load.rs` with only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::save::{save, to_scene_file};
    use crate::{Sprite, Transform};
    use voltra_render::glam::Vec2;

    fn source_world() -> World {
        let mut world = World::new();
        for i in 0..3 {
            let e = world.spawn();
            world.insert(e, SceneId::new());
            world.insert(
                e,
                Transform::from_translation(Vec2::new(i as f32, -1.0))
                    .with_scale(Vec2::splat(0.5)),
            );
            world.insert(e, Sprite::new([1.0, 0.0, 0.0, 1.0]).with_sort_order(i as i32));
        }
        world
    }

    #[test]
    fn a_world_survives_a_round_trip() {
        let registry = ComponentRegistry::with_defaults();
        let original = source_world();
        let file = to_scene_file(&original, &registry).expect("saving cannot fail here");

        let mut loaded = World::new();
        from_scene_file(&file, &registry, &mut loaded).expect("loading cannot fail here");

        let mut before: Vec<_> = original
            .query::<SceneId>()
            .map(|(e, id)| {
                (
                    *id,
                    original.get::<Transform>(e).copied(),
                    original.get::<Sprite>(e).copied(),
                )
            })
            .collect();
        let mut after: Vec<_> = loaded
            .query::<SceneId>()
            .map(|(e, id)| {
                (
                    *id,
                    loaded.get::<Transform>(e).copied(),
                    loaded.get::<Sprite>(e).copied(),
                )
            })
            .collect();
        before.sort_by_key(|(id, _, _)| *id);
        after.sort_by_key(|(id, _, _)| *id);

        assert_eq!(before, after);
    }

    #[test]
    fn an_unknown_component_survives_a_round_trip() {
        // The test the preservation promise rests on. Compared as parsed values
        // rather than as text, because formatting is the serializer's choice and
        // byte equality against a hand-written file is not achievable.
        let registry = ComponentRegistry::with_defaults();
        let text = r#"(
            version: 1,
            entities: [
                (
                    id: "018f3a2b-7c41-7000-8000-2b1d4e5f6a70",
                    components: {
                        "Physics": (mass: 5.0, friction: 0.25),
                        "Sprite": (color: (1.0, 1.0, 1.0, 1.0), sort_order: 0),
                    },
                ),
            ],
        )"#;

        let file: SceneFile = ron::from_str(text).expect("the fixture is valid RON");
        let expected = file.entities[0]
            .components
            .get("Physics")
            .expect("the fixture has a Physics")
            .clone();

        let mut world = World::new();
        from_scene_file(&file, &registry, &mut world).expect("loading cannot fail here");

        let saved = to_scene_file(&world, &registry).expect("saving cannot fail here");
        assert_eq!(saved.entities[0].components.get("Physics"), Some(&expected));
    }

    #[test]
    fn a_wrong_version_is_refused() {
        let registry = ComponentRegistry::with_defaults();
        let file = SceneFile {
            version: VERSION + 1,
            entities: Vec::new(),
        };

        let mut world = World::new();
        let result = from_scene_file(&file, &registry, &mut world);
        assert!(
            matches!(result, Err(SceneError::UnsupportedVersion { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn a_registered_component_with_wrong_data_fails_the_load() {
        // Not preserved as unknown: the name is registered, so this build claims
        // to understand it, and data it cannot read is a real error.
        let registry = ComponentRegistry::with_defaults();
        let text = r#"(
            version: 1,
            entities: [
                (
                    id: "018f3a2b-7c41-7000-8000-2b1d4e5f6a70",
                    components: { "Sprite": "not a sprite" },
                ),
            ],
        )"#;
        let file: SceneFile = ron::from_str(text).expect("the fixture is valid RON");

        let mut world = World::new();
        let result = from_scene_file(&file, &registry, &mut world);
        assert!(
            matches!(result, Err(SceneError::Component { .. })),
            "got {result:?}"
        );
    }

    #[test]
    fn a_missing_file_is_an_io_error() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let missing = std::env::temp_dir().join("voltra-no-such-scene.ron");
        let _ = std::fs::remove_file(&missing);

        let result = load(&missing, &registry, &mut world);
        assert!(matches!(result, Err(SceneError::Io(_))), "got {result:?}");
    }

    #[test]
    fn malformed_ron_is_a_parse_error() {
        let registry = ComponentRegistry::with_defaults();
        let mut world = World::new();
        let dir = std::env::temp_dir().join("voltra-scene-load-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("broken.ron");
        std::fs::write(&path, "this is not ron at all {{{").expect("write");

        let result = load(&path, &registry, &mut world);
        assert!(matches!(result, Err(SceneError::Parse(_))), "got {result:?}");
    }

    #[test]
    fn saving_is_idempotent_through_a_load() {
        let registry = ComponentRegistry::with_defaults();
        let dir = std::env::temp_dir().join("voltra-scene-idempotent-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let first = dir.join("first.ron");
        let second = dir.join("second.ron");

        save(&source_world(), &registry, &first).expect("saving cannot fail here");

        let mut world = World::new();
        load(&first, &registry, &mut world).expect("loading cannot fail here");
        save(&world, &registry, &second).expect("saving cannot fail here");

        assert_eq!(
            std::fs::read_to_string(&first).expect("written"),
            std::fs::read_to_string(&second).expect("written"),
            "save -> load -> save must be a fixed point"
        );
    }
}
```

- [ ] **Step 2: Run them and confirm they fail**

```sh
cargo test -p voltra-scene --lib format::load
```

Expected: compile error — `cannot find function 'from_scene_file' in this scope`.

- [ ] **Step 3: Write the implementation**

Above that test module:

```rust
//! Reading a scene file back into a world.

use std::path::Path;

use voltra_ecs::World;

use super::error::SceneError;
use super::registry::ComponentRegistry;
use super::save::{SceneFile, VERSION};
use crate::scene_id::UnknownComponents;

/// Spawns every entity in `file` into `world`.
///
/// Adds rather than replaces. Whether to clear first is the caller's decision,
/// and the editor's menu is where that belongs.
pub fn from_scene_file(
    file: &SceneFile,
    registry: &ComponentRegistry,
    world: &mut World,
) -> Result<(), SceneError> {
    if file.version != VERSION {
        return Err(SceneError::UnsupportedVersion {
            found: file.version,
            supported: VERSION,
        });
    }

    for record in &file.entities {
        let entity = world.spawn();
        world.insert(entity, record.id);

        let mut unknown = UnknownComponents::default();

        for (name, value) in &record.components {
            match registry.load_one(world, entity, name, value) {
                // Registered: a failure here is ours, and it propagates. The
                // name says this build understands the component, so data it
                // cannot read is broken rather than foreign.
                Some(result) => result?,
                // Not registered: keep it verbatim so saving writes it back.
                // Dropping it is how a build silently deletes work done by a
                // build that knew more.
                None => {
                    log::warn!(
                        "scene contains unknown component `{name}`; keeping it unread"
                    );
                    unknown.0.insert(name.clone(), value.clone());
                }
            }
        }

        if !unknown.0.is_empty() {
            world.insert(entity, unknown);
        }
    }

    log::info!("loaded {} entities", file.entities.len());
    Ok(())
}

/// Reads `path` and spawns its entities into `world`.
pub fn load(
    path: &Path,
    registry: &ComponentRegistry,
    world: &mut World,
) -> Result<(), SceneError> {
    let text = std::fs::read_to_string(path)?;
    let file: SceneFile = ron::from_str(&text).map_err(SceneError::Parse)?;
    from_scene_file(&file, registry, world)
}
```

The `log::warn!` fires once per unknown component per entity. If a scene has many
entities carrying the same unknown component that is noisy — note it in your
report if you think it needs de-duplicating, but do not change the behaviour in
this task.

- [ ] **Step 4: Export the new items**

In `crates/voltra-scene/src/format.rs`:

```rust
pub mod load;
```

```rust
pub use load::{from_scene_file, load};
```

- [ ] **Step 5: Run the tests and the workspace**

```sh
cargo test -p voltra-scene --lib format
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 6: Prove the preservation test can fail**

A test that claims to guard preservation must be shown failing when preservation
is removed. Temporarily replace the `None` arm's body in `from_scene_file` with
nothing but the `log::warn!` — dropping the component instead of keeping it — and
run:

```sh
cargo test -p voltra-scene --lib format::load::tests::an_unknown_component_survives_a_round_trip
```

Expected: **FAIL**. Paste the output into your report, then restore the arm
exactly and re-run to confirm it passes again.

If it does **not** fail, stop and report: it means the test is not testing what
its name claims, and adjusting it to pass is exactly the wrong response.

- [ ] **Step 7: Commit**

```sh
git add crates/voltra-scene/src
git commit -m "feat(scene): load a scene, keeping unknown parts

A component this build does not recognise is kept unparsed and written back
untouched, so opening and saving a scene never deletes work done by a build
that knew more. A registered component that fails to read is an error.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Save and open from the editor

**Files:**
- Modify: `crates/voltra-editor/Cargo.toml`
- Modify: `crates/voltra-editor/src/panels/menu_bar.rs`
- Modify: `crates/voltra-editor/src/main.rs`

**Interfaces:**
- Consumes from Tasks 1-4: `ComponentRegistry`, `SceneId`, `save`, `load`.
- Produces: menu entries, and a `SceneId` on every sprite the editor spawns.

- [ ] **Step 1: Give the editor the scene dependency it needs**

`crates/voltra-editor/Cargo.toml` already depends on `voltra-scene`. Confirm it, and add nothing else — `save` and `load` take a `&Path`, and `std::path` needs no dependency.

- [ ] **Step 2: Stamp spawned entities with an identity**

In `crates/voltra-editor/src/panels/menu_bar.rs`, `spawn_sprite` currently inserts a `Transform` and a `Sprite`. Add the identity, since an entity with no `SceneId` is deliberately not saved:

```rust
fn spawn_sprite(frame: &mut UiFrame<'_>) -> Entity {
    let entity = frame.world.spawn();
    frame.world.insert(entity, SceneId::new());
    frame
        .world
        .insert(entity, Transform::default().with_scale(Vec2::splat(0.4)));
    frame.world.insert(entity, Sprite::default());
    entity
}
```

Add `SceneId` to that file's `voltra_scene` import.

Do the same for the three demo sprites in `crates/voltra-editor/src/main.rs`'s `spawn_demo_scene`, so a fresh editor has a saveable scene.

- [ ] **Step 3: Add the menu entries**

In the same `Scene` menu in `menu_bar.rs`, after the existing `Spawn sprite` and `Clear` items:

```rust
                ui.separator();

                if ui.button("Save").clicked() {
                    let registry = ComponentRegistry::with_defaults();
                    match voltra_scene::format::save(frame.world, &registry, default_path()) {
                        Ok(()) => log::info!("scene saved"),
                        Err(e) => log::error!("could not save the scene: {e}"),
                    }
                    ui.close();
                }

                if ui.button("Open").clicked() {
                    let registry = ComponentRegistry::with_defaults();
                    // Replaces rather than merges: "Open" meaning "add another
                    // copy of everything" would surprise anyone.
                    let all: Vec<Entity> = frame.world.query::<SceneId>().map(|(e, _)| e).collect();
                    for entity in all {
                        frame.world.despawn(entity);
                    }
                    match voltra_scene::format::load(default_path(), &registry, frame.world) {
                        Ok(()) => log::info!("scene loaded"),
                        Err(e) => log::error!("could not open the scene: {e}"),
                    }
                    ui.close();
                }
```

And, near the top of the file:

```rust
/// Where the editor saves when no path has been chosen.
///
/// A parameter with a default rather than a value baked into `save` and `load` —
/// the second caller is a file dialog, and it is already foreseeable.
fn default_path() -> &'static Path {
    Path::new("assets/scenes/scene.ron")
}
```

The directory must exist before writing. Create `assets/scenes/` in the repo with a `.gitkeep`, and have the save arm create it too:

```rust
                    if let Some(parent) = default_path().parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            log::error!("could not create {}: {e}", parent.display());
                        }
                    }
```

Place that immediately before the `save` call.

- [ ] **Step 4: Build, lint and test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: clean. `Editor::selected` holds an `Option<Entity>` that can outlive a despawn on Open — the inspector already filters on `is_alive`, so no change is needed there, but confirm that by reading `crates/voltra-editor/src/panels/inspector.rs` rather than assuming.

- [ ] **Step 5: Run the editor**

```sh
cargo run -p voltra-editor > editor.log 2>&1 &
sleep 8
cat editor.log
taskkill //F //IM voltra-editor.exe
```

Expected: no `ERROR`, no panic. You cannot drive a mouse, so you cannot confirm that the menu items work — do not claim you did. Report what you verified.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-editor assets
git commit -m "feat(editor): save and open the scene

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/ARCHITECTURE.md`

**Interfaces:**
- Consumes: the behaviour settled in Tasks 1-5.
- Produces: nothing code depends on.

- [ ] **Step 1: Update the README**

Two edits, anchored on surrounding text rather than line numbers:

- The editor description gains a sentence: the scene saves to and loads from `assets/scenes/scene.ron` through `Scene ▸ Save` and `Scene ▸ Open`.
- The roadmap row `| 9 | Scene serialization and the asset pipeline | next |` splits, because only half of it is done:

```markdown
| 9 | Scene serialization | done |
| 12 | Asset pipeline: loading, caching, hot reload | planned |
```

Keep the rows in numeric order; put row 12 after the existing final row.

- [ ] **Step 2: Add the decision to ARCHITECTURE.md**

Append to the "Decisions" section an entry titled `### Scenes are RON, and unknown components survive`. Write it from the spec, and cover, in this order: why serialization cannot live in `voltra-ecs` (zero dependencies, and no way to enumerate type-erased storages); why the registry is keyed by a chosen name rather than a Rust path; why `SceneId` is UUID **v7** and what sorting by it buys; why unknown components are preserved and written back, naming Unity's drop-on-save as the rejected behaviour and Godot's preservation as the adopted one; and why a known component that fails to deserialize is an error instead.

State plainly that byte-identical output against an arbitrary input file is not a guarantee this design makes, and that the property actually held is idempotence: save, load, save produces identical bytes.

Match the surrounding entries' voice and wrap width.

- [ ] **Step 3: Update the crate table**

In ARCHITECTURE.md's "Current crates" table, the `voltra-scene` row's "Key types" cell gains `SceneFile`, `ComponentRegistry`, `SceneId`.

- [ ] **Step 4: Verify and commit**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

```sh
git add README.md docs/ARCHITECTURE.md
git commit -m "docs: record the scene format decision

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Final verification

- [ ] **Clean run**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **`voltra-ecs` really was not touched**

```sh
git diff --stat main...HEAD -- crates/voltra-ecs
```

Expected: no output. A change there means the registry design has a hole and the branch should not merge until that is understood.

- [ ] **No duplicate `glam` or `wgpu`**

```sh
cargo tree --workspace -d
```

Expected: neither appears. A second `glam` means two incompatible `Vec2` types.

- [ ] **No literal versions in member crates**

```sh
git grep -nE '^\s*(serde|ron|uuid|glam)\s*=\s*"' -- crates/*/Cargo.toml
```

Expected: no output. Every one must be `dep.workspace = true`.

- [ ] **Editor smoke test**

```sh
cargo run -p voltra-editor > editor.log 2>&1 &
sleep 8
cat editor.log
taskkill //F //IM voltra-editor.exe
```

Behaviours a human must confirm, since no agent can drive the mouse:

1. `Scene ▸ Save` writes `assets/scenes/scene.ron`, and the file is readable.
2. Editing a sprite, saving, and saving again produces no diff on the second save.
3. `Scene ▸ Open` restores the scene after `Scene ▸ Clear`.
4. Hand-adding a `"Physics": (mass: 5.0)` component to an entity in the file, then opening and saving in the editor, leaves that line intact — with a `WARN` in the log naming it.
5. Corrupting the file produces an `ERROR` in the log and leaves the editor running.

- [ ] **Push**

```sh
git push -u origin feature/scene-serialization
```
