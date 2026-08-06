# Sprite Picking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Click a sprite in the viewport to select it, and give the scene a stable draw order so that "what I clicked" and "what is on top" are the same question.

**Architecture:** `Sprite` gains an integer `sort_order`. `SpriteBatch::from_world` sorts by `(sort_order, entity.index())` before emitting geometry, which fixes an existing latent bug where despawning an entity could reorder unrelated overlapping sprites. A new `voltra-scene::pick` module answers "which entity is under this world point" using the same ordering, and the editor routes viewport clicks through it.

**Tech Stack:** Rust, `glam` 0.33 (via `voltra_render::glam`), `egui` 0.35 (via `voltra_core::egui`).

Spec: [docs/superpowers/specs/2026-08-06-sprite-picking-design.md](../specs/2026-08-06-sprite-picking-design.md)

## Global Constraints

- Branch is `feature/sprite-picking`, already created off `main`. Never commit to `main`.
- **This is 2D.** Per the scope section at the top of CLAUDE.md: no depth buffer, no z-axis on any component, no 3D scaffolding, and no name that implies an axis it is not. The sort key is `sort_order`, never `z_index` or `z`.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` must all pass before any commit. Clippy warnings are errors.
- No `unwrap()` outside `#[cfg(test)]`. `expect("why this cannot fail")` when the invariant is real.
- Log through `log`, never `println!`.
- Only `voltra-core` may depend on `winit`; only `voltra-render` may depend on `wgpu`. `voltra-scene` and `voltra-editor` use the re-exports — `voltra_render::glam`, `voltra_core::egui`.
- This plan adds **no dependencies**. No member crate may gain a literal version.
- One concept per file. A module splits into a directory past roughly 300 lines or a second concept.
- Unit tests live in the file they test, in `#[cfg(test)] mod tests`.
- Conventional Commits, scope = crate minus the `voltra-` prefix, imperative subject **50 characters or fewer** — count them.
- Every commit ends with the trailer `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- The editor is a GUI app with an infinite event loop. Never run `cargo run -p voltra-editor` in the foreground — launch it detached, wait a few seconds, read the log, kill it (`taskkill` on this machine; `pkill` is not available).

### Facts about the existing code that every task depends on

- `World::query2::<A, B>()` yields `(Entity, &A, &B)`. `World::query::<T>()` yields `(Entity, &T)`.
- `Entity::index()` returns `u32`. It is stable for as long as the entity lives, but is recycled after a despawn.
- `Transform` is `translation: Vec2`, `rotation: f32` (counter-clockwise radians), `scale: Vec2`. `Transform::matrix()` returns a `glam::Mat3` mapping local space to world space.
- Every sprite is the same quad, centred on the origin, spanning `-0.5..=0.5` on both axes before its transform is applied.
- `glam::Mat3` has `determinant()`, `inverse()` and `transform_point2()`.

---

### Task 1: Split `sprite.rs` into the component and the batch

Pure move, no behaviour change. `crates/voltra-scene/src/sprite.rs` is 252 lines holding two concepts — the `Sprite` component and the `SpriteBatch` geometry builder — which CONVENTIONS.md already forbids. Task 2 adds a field, sorting logic and tests, which would push it past 300. The split lands first and separately so the split and the new behaviour never share a diff.

**Files:**
- Modify: `crates/voltra-scene/src/sprite.rs` (keep only `Sprite`)
- Create: `crates/voltra-scene/src/batch.rs` (`SpriteBatch` and its geometry)
- Modify: `crates/voltra-scene/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `voltra_scene::Sprite` and `voltra_scene::SpriteBatch` remain importable at exactly those paths — `lib.rs` re-exports both, so no other crate changes.

- [ ] **Step 1: Read the current file end to end**

```sh
cat crates/voltra-scene/src/sprite.rs
```

You are redistributing this file. Everything in it lands in exactly one of the two output files, unchanged. Comments move with the code they explain — this codebase's comments carry the reasoning and a dropped one is a real loss.

- [ ] **Step 2: Reduce `sprite.rs` to the component**

`crates/voltra-scene/src/sprite.rs` keeps only the module header, the `Sprite` struct, its `Default` and its `impl`. Its `use` list shrinks to what remains — `Sprite` itself needs no imports at all once `SpriteBatch` has gone.

```rust
//! The sprite component: a coloured quad, sized by its entity's transform.

/// A coloured quad. Its size comes from the entity's [`Transform`].
///
/// [`Transform`]: crate::transform::Transform
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    /// Multiplied with the bound texture. White leaves the texture as-is.
    pub color: [f32; 4],
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

impl Sprite {
    pub fn new(color: [f32; 4]) -> Self {
        Self { color }
    }
}
```

- [ ] **Step 3: Move the batch into `batch.rs`**

Everything else from the original file goes to `crates/voltra-scene/src/batch.rs` unchanged: the `CORNERS` and `QUAD_INDICES` constants with their comments, the `SpriteBatch` struct, its whole `impl`, and the entire `#[cfg(test)] mod tests`. Its header and imports:

```rust
//! Turning a world full of sprites into vertex and index data.

use voltra_ecs::World;
use voltra_render::Vertex;

use crate::sprite::Sprite;
use crate::transform::Transform;
```

The test module inside it currently starts `use super::*;` and `use voltra_render::glam::Vec2;`. Keep both, and add whatever the tests need that used to come from `super::*` in the combined file — the compiler will name anything missing.

- [ ] **Step 4: Declare the new module and keep the public paths**

`crates/voltra-scene/src/lib.rs` becomes:

```rust
//! Scene components and the geometry they turn into.
//!
//! Sits between `voltra-ecs`, which knows nothing about rendering, and
//! `voltra-render`, which knows nothing about entities. Dependencies point
//! down into both and never back up.

pub mod batch;
pub mod sprite;
pub mod transform;

pub use batch::SpriteBatch;
pub use sprite::Sprite;
pub use transform::Transform;
```

`voltra_scene::SpriteBatch` and `voltra_scene::Sprite` still resolve, so `voltra-core` and `voltra-editor` need no change.

- [ ] **Step 5: Verify nothing changed but the layout**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: no warnings; every test that passed before still passes, with the same names.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-scene/src
git commit -m "refactor(scene): split the sprite batch out

sprite.rs held both the component and the geometry builder, and the draw-order
work is about to add a field, a sort and its tests. Move only.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: `sort_order` and a stable draw order

**Files:**
- Modify: `crates/voltra-scene/src/sprite.rs` (add the field, the constant and a builder)
- Modify: `crates/voltra-scene/src/batch.rs` (sort in `from_world`, derive `CORNERS` from the constant, add tests)

**Interfaces:**
- Consumes from Task 1: `crate::sprite::Sprite`, `crate::batch::SpriteBatch`.
- Produces:
  - `Sprite::sort_order: i32`, public field, defaulting to `0`
  - `Sprite::HALF_EXTENT: f32` — associated const, `0.5`
  - `Sprite::with_sort_order(self, order: i32) -> Self` — builder, matching `Transform::with_scale`'s style
  - `SpriteBatch::from_world` emits sprites ordered by `(sort_order, entity.index())`

- [ ] **Step 1: Write the failing tests**

Append to `#[cfg(test)] mod tests` in `crates/voltra-scene/src/batch.rs`. The existing helper in that module builds a world from a slice; these tests need the entity handles back, so add this second helper beside it:

```rust
    /// Spawns each sprite and hands back the handles, so a test can despawn a
    /// specific one.
    fn world_returning_entities(
        sprites: &[(Transform, Sprite)],
    ) -> (World, Vec<voltra_ecs::Entity>) {
        let mut world = World::new();
        let mut entities = Vec::new();
        for (transform, sprite) in sprites {
            let e = world.spawn();
            world.insert(e, *transform);
            world.insert(e, *sprite);
            entities.push(e);
        }
        (world, entities)
    }

    /// The x coordinate of each sprite's centre, in the order the GPU will
    /// receive them.
    ///
    /// Asserting on the vertex buffer rather than on an internal list checks
    /// the thing that actually reaches the driver. The mean of the four
    /// corners rather than the first one: a corner sits half a unit off the
    /// sprite's position, so reading one directly would make these tests
    /// depend on which corner `CORNERS` happens to list first, which is
    /// incidental to draw order.
    fn draw_order(batch: &SpriteBatch) -> Vec<f32> {
        batch
            .vertices
            .chunks(4)
            .map(|quad| quad.iter().map(|v| v.position[0]).sum::<f32>() / 4.0)
            .collect()
    }

    #[test]
    fn a_higher_sort_order_is_drawn_later() {
        // Spawned back-to-front on purpose: if the sort is missing, the
        // insertion order survives and this fails.
        let (world, _) = world_returning_entities(&[
            (
                Transform::from_translation(Vec2::new(10.0, 0.0)),
                Sprite::default().with_sort_order(5),
            ),
            (
                Transform::from_translation(Vec2::new(20.0, 0.0)),
                Sprite::default().with_sort_order(-3),
            ),
        ]);

        let batch = SpriteBatch::from_world(&world);
        let xs = draw_order(&batch);

        // The -3 sprite sits at x = 20 and must come first; painter's order
        // means later is on top.
        assert!(xs[0] > 19.0, "expected the -3 sprite first, got {xs:?}");
        assert!(xs[1] < 11.0, "expected the 5 sprite last, got {xs:?}");
    }

    #[test]
    fn sprites_are_drawn_in_spawn_order_by_default() {
        let (world, _) = world_returning_entities(&[
            (Transform::from_translation(Vec2::new(1.0, 0.0)), Sprite::default()),
            (Transform::from_translation(Vec2::new(2.0, 0.0)), Sprite::default()),
            (Transform::from_translation(Vec2::new(3.0, 0.0)), Sprite::default()),
        ]);

        // A baseline, and deliberately named as one. With nothing despawned,
        // storage order already equals spawn order, so this would pass with no
        // sort at all — the test that actually exercises the ordering is below.
        let xs = draw_order(&SpriteBatch::from_world(&world));
        assert_eq!(xs, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn despawning_scrambles_storage_but_not_draw_order() {
        let (mut world, entities) = world_returning_entities(&[
            (Transform::from_translation(Vec2::new(1.0, 0.0)), Sprite::default()),
            (Transform::from_translation(Vec2::new(2.0, 0.0)), Sprite::default()),
            (Transform::from_translation(Vec2::new(3.0, 0.0)), Sprite::default()),
            (Transform::from_translation(Vec2::new(4.0, 0.0)), Sprite::default()),
        ]);

        // Four sprites, and the second one goes. `SparseSet::remove` is a
        // `swap_remove`, so the *last* element takes the freed slot: storage
        // becomes [1, 4, 3] and an unsorted batch would draw it that way.
        //
        // Four rather than three on purpose. With three, removing index 1 puts
        // the last element exactly where a stable removal would have left it,
        // so [1, 3] comes out identical with and without the sort and the test
        // proves nothing.
        //
        // Every sprite is on the default sort_order of 0, so `entity.index()`
        // is the only thing producing this order. Drop that half of the key and
        // `sort_by_key`'s stability preserves storage order and this fails.
        world.despawn(entities[1]);

        let xs = draw_order(&SpriteBatch::from_world(&world));
        assert_eq!(xs, vec![1.0, 3.0, 4.0]);
    }

    #[test]
    fn a_sprite_defaults_to_sort_order_zero() {
        assert_eq!(Sprite::default().sort_order, 0);
        assert_eq!(Sprite::new([1.0, 0.0, 0.0, 1.0]).sort_order, 0);
    }
```

- [ ] **Step 2: Run them and confirm they fail**

```sh
cargo test -p voltra-scene --lib batch
```

Expected: compile error — `no method named 'with_sort_order'` and `no field 'sort_order' on type 'Sprite'`.

- [ ] **Step 3: Add the field, the constant and the builder**

`crates/voltra-scene/src/sprite.rs` becomes:

```rust
//! The sprite component: a coloured quad, sized by its entity's transform.

/// A coloured quad. Its size comes from the entity's [`Transform`].
///
/// [`Transform`]: crate::transform::Transform
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    /// Multiplied with the bound texture. White leaves the texture as-is.
    pub color: [f32; 4],
    /// Draw order within the scene. Higher is drawn later, and therefore on
    /// top.
    ///
    /// A sorting key, **not** a coordinate — this is a 2D engine with no depth
    /// buffer, and nothing here corresponds to an axis. Named after Unity's
    /// `sortingOrder` rather than Godot's `z_index` for exactly that reason:
    /// when a real Z eventually exists, the name must still be free.
    ///
    /// An `i32` rather than an `f32` so ties are exact and never depend on a
    /// float's representation.
    pub sort_order: i32,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            sort_order: 0,
        }
    }
}

impl Sprite {
    /// Half the width and height of the quad a sprite covers, before its
    /// transform is applied.
    ///
    /// Batching and picking must agree on this. If they ever disagree, a click
    /// lands somewhere other than the pixels it appears to land on, and nothing
    /// reports it.
    pub const HALF_EXTENT: f32 = 0.5;

    pub fn new(color: [f32; 4]) -> Self {
        Self {
            color,
            ..Default::default()
        }
    }

    pub fn with_sort_order(mut self, order: i32) -> Self {
        self.sort_order = order;
        self
    }
}
```

- [ ] **Step 4: Sort in `from_world`, and derive the corners from the constant**

In `crates/voltra-scene/src/batch.rs`, `CORNERS` currently hardcodes `0.5` four times. Point it at the shared constant so the quad has one definition:

```rust
/// The unit quad every sprite is built from, as `(corner, uv)` pairs.
///
/// Centred on the origin so scale and rotation act about the sprite's middle
/// rather than dragging it away from its own position. V runs opposite to Y
/// because image rows go down while clip space goes up.
const CORNERS: [(Vec2, [f32; 2]); 4] = [
    (Vec2::new(-Sprite::HALF_EXTENT, Sprite::HALF_EXTENT), [0.0, 0.0]),
    (Vec2::new(-Sprite::HALF_EXTENT, -Sprite::HALF_EXTENT), [0.0, 1.0]),
    (Vec2::new(Sprite::HALF_EXTENT, -Sprite::HALF_EXTENT), [1.0, 1.0]),
    (Vec2::new(Sprite::HALF_EXTENT, Sprite::HALF_EXTENT), [1.0, 0.0]),
];
```

That needs `Vec2` in scope at module level — add `use voltra_render::glam::Vec2;` to `batch.rs`'s imports if the file does not already have it, and remove the now-duplicate import from its test module if one results.

Then replace the body of `from_world`:

```rust
    /// Walks every entity holding both a [`Transform`] and a [`Sprite`].
    ///
    /// Collected and sorted rather than pushed as we go. With no depth buffer,
    /// the order geometry is written in is the order it is drawn in — and
    /// `query2` yields sparse-set storage order, which shifts when an unrelated
    /// entity is despawned. `Entity::index` breaks ties so sprites sharing a
    /// `sort_order` keep a stable order too; without it the common case, where
    /// everything sits on 0, would be exactly as unstable as before.
    pub fn from_world(world: &World) -> Self {
        let mut sprites: Vec<_> = world.query2::<Transform, Sprite>().collect();
        sprites.sort_by_key(|(entity, _, sprite)| (sprite.sort_order, entity.index()));

        let mut batch = Self::default();
        for (_entity, transform, sprite) in sprites {
            batch.push(transform, sprite);
        }
        batch
    }
```

- [ ] **Step 5: Run the tests**

```sh
cargo test -p voltra-scene --lib
```

Expected: PASS, including every test that was already in the file.

- [ ] **Step 6: Check the workspace**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: clean. Note that `Sprite` gaining a field breaks any struct literal that lists every field — the compiler will name them if so.

- [ ] **Step 7: Commit**

```sh
git add crates/voltra-scene/src
git commit -m "feat(scene): give sprites a stable draw order

Draw order was sparse-set order, so despawning one entity could reorder two
unrelated overlapping sprites — with no depth buffer that is the only thing
deciding what covers what, and alpha blending is order-dependent.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: `sprite_at` — the hit test

**Files:**
- Create: `crates/voltra-scene/src/pick.rs`
- Modify: `crates/voltra-scene/src/lib.rs` (declare the module)

**Interfaces:**
- Consumes from Task 2: `Sprite::sort_order`, `Sprite::HALF_EXTENT`.
- Produces: `voltra_scene::pick::sprite_at(world: &World, point: Vec2) -> Option<Entity>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/voltra-scene/src/pick.rs` containing only this test module for now, so the tests fail on a missing function rather than a missing file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_4;

    // `World`, `Entity`, `Transform`, `Sprite` and `Vec2` all arrive through
    // `super::*` — do not import them again here, or the glob and the explicit
    // import name the same item twice.

    fn spawn(world: &mut World, transform: Transform, sprite: Sprite) -> Entity {
        let e = world.spawn();
        world.insert(e, transform);
        world.insert(e, sprite);
        e
    }

    #[test]
    fn an_empty_world_picks_nothing() {
        assert_eq!(sprite_at(&World::new(), Vec2::ZERO), None);
    }

    #[test]
    fn a_point_outside_every_sprite_picks_nothing() {
        let mut world = World::new();
        spawn(&mut world, Transform::default(), Sprite::default());
        assert_eq!(sprite_at(&world, Vec2::new(5.0, 5.0)), None);
    }

    #[test]
    fn a_point_inside_picks_that_sprite() {
        let mut world = World::new();
        let a = spawn(
            &mut world,
            Transform::from_translation(Vec2::new(-2.0, 0.0)),
            Sprite::default(),
        );
        let b = spawn(
            &mut world,
            Transform::from_translation(Vec2::new(2.0, 0.0)),
            Sprite::default(),
        );

        assert_eq!(sprite_at(&world, Vec2::new(-2.1, 0.1)), Some(a));
        assert_eq!(sprite_at(&world, Vec2::new(2.1, -0.1)), Some(b));
    }

    #[test]
    fn rotation_is_respected_not_approximated() {
        let mut world = World::new();
        // A unit square turned 45 degrees. Its bounding box reaches about
        // 0.707 on each axis, but the quad itself does not: the corner of the
        // box is outside the diamond.
        spawn(
            &mut world,
            Transform::default().with_rotation(FRAC_PI_4),
            Sprite::default(),
        );

        // Inside the diamond.
        assert!(sprite_at(&world, Vec2::new(0.0, 0.6)).is_some());
        // Inside the bounding box, outside the diamond. An AABB test would
        // wrongly report a hit here, which is the whole point of this test.
        assert_eq!(sprite_at(&world, Vec2::new(0.45, 0.45)), None);
    }

    #[test]
    fn non_uniform_scale_is_respected() {
        let mut world = World::new();
        // Four wide, half tall.
        spawn(
            &mut world,
            Transform::default().with_scale(Vec2::new(4.0, 0.5)),
            Sprite::default(),
        );

        assert!(sprite_at(&world, Vec2::new(1.8, 0.0)).is_some());
        // Inside on x, outside on y.
        assert_eq!(sprite_at(&world, Vec2::new(1.8, 0.4)), None);
    }

    #[test]
    fn the_higher_sort_order_wins_an_overlap() {
        let mut world = World::new();
        let _under = spawn(
            &mut world,
            Transform::default(),
            Sprite::default().with_sort_order(10),
        );
        let over = spawn(
            &mut world,
            Transform::default(),
            Sprite::default().with_sort_order(20),
        );

        assert_eq!(sprite_at(&world, Vec2::ZERO), Some(over));

        // And the answer does not depend on which was spawned first.
        let mut reversed = World::new();
        let over_first = spawn(
            &mut reversed,
            Transform::default(),
            Sprite::default().with_sort_order(20),
        );
        spawn(
            &mut reversed,
            Transform::default(),
            Sprite::default().with_sort_order(10),
        );
        assert_eq!(sprite_at(&reversed, Vec2::ZERO), Some(over_first));
    }

    #[test]
    fn an_overlap_tie_goes_to_the_later_entity() {
        let mut world = World::new();
        let _first = spawn(&mut world, Transform::default(), Sprite::default());
        let second = spawn(&mut world, Transform::default(), Sprite::default());

        assert_eq!(sprite_at(&world, Vec2::ZERO), Some(second));
    }

    #[test]
    fn a_zero_scale_sprite_is_never_picked() {
        let mut world = World::new();
        spawn(
            &mut world,
            Transform::default().with_scale(Vec2::new(0.0, 1.0)),
            Sprite::default(),
        );

        // Honest about its own weakness: this passes with the determinant check
        // removed too. `glam` inverts an exactly-singular matrix to NaN, and
        // every comparison against NaN is false, so the sprite is skipped by
        // accident rather than by decision. Kept because it pins the behaviour
        // we want, but the test that makes the guard load-bearing is the next
        // one.
        assert_eq!(sprite_at(&world, Vec2::ZERO), None);
        assert_eq!(sprite_at(&world, Vec2::new(100.0, 100.0)), None);
    }

    #[test]
    fn a_near_zero_scale_sprite_is_never_picked() {
        let mut world = World::new();
        // Nearly singular rather than singular, which is the case the
        // determinant check actually decides. This matrix still inverts to
        // finite numbers, so NaN does not rescue us: without the check, the
        // inverse scales x by 1e30, the origin maps to the origin, and a sprite
        // far thinner than a pixel is pickable along its entire centre line.
        spawn(
            &mut world,
            Transform::default().with_scale(Vec2::new(1e-30, 1.0)),
            Sprite::default(),
        );

        assert_eq!(sprite_at(&world, Vec2::ZERO), None);
    }

    #[test]
    fn picking_agrees_with_the_draw_order() {
        // `sort_order` and spawn order deliberately disagree: the sprite that
        // must win is spawned FIRST, so it carries the *lower* entity index.
        //
        // That is what makes this test about composition rather than about
        // either key alone. Order by index only and the loser wins. Swap the
        // tuple to `(index, sort_order)` and the loser wins. Only
        // `(sort_order, index)`, in that order, in both places, gives `top` —
        // which is the property the whole design rests on, since a divergence
        // would mean clicking selects something other than the visible pixels.
        let mut world = World::new();
        let top = spawn(
            &mut world,
            Transform::default(),
            Sprite::new([0.0, 1.0, 0.0, 1.0]).with_sort_order(2),
        );
        spawn(
            &mut world,
            Transform::default(),
            Sprite::new([1.0, 0.0, 0.0, 1.0]).with_sort_order(1),
        );

        let batch = crate::batch::SpriteBatch::from_world(&world);
        let last_drawn_green = batch.vertices[batch.vertices.len() - 1].color[1];

        assert_eq!(sprite_at(&world, Vec2::ZERO), Some(top));
        assert!(
            last_drawn_green > 0.5,
            "the sort_order 2 sprite must be drawn last, got green {last_drawn_green}"
        );
    }
}
```

- [ ] **Step 2: Run them and confirm they fail**

First add `pub mod pick;` to `crates/voltra-scene/src/lib.rs`, beside the other `pub mod` lines and in alphabetical order, then:

```sh
cargo test -p voltra-scene --lib pick
```

Expected: compile error — `cannot find function 'sprite_at' in this scope`, plus unresolved `Transform`, `Sprite`, `Vec2` and `Entity`.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `crates/voltra-scene/src/pick.rs`:

```rust
//! Which entity is under a point.
//!
//! 2D only, per CLAUDE.md: a point tested against a quad, not a ray against a
//! volume. 3D picking is a different subsystem and will not reuse this.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use crate::sprite::Sprite;
use crate::transform::Transform;

/// Determinant at or below which a transform counts as collapsed.
///
/// Deliberately **not** `f32::EPSILON`. That constant is a *relative* precision
/// figure — the gap between `1.0` and the next representable float — and using
/// it as an absolute cutoff rejects any sprite with a uniform scale below about
/// `3.5e-4`, which is small but perfectly invertible and perfectly legitimate.
///
/// `1e-12` is the determinant of a uniform scale of `1e-6`, a millionth of a
/// world unit. The tightest view the camera allows is two world units divided
/// by `Camera2D::MAX_ZOOM`, so a sprite that size is orders of magnitude below
/// one pixel and cannot be meant to be clicked. Inverting a matrix with that
/// determinant still produces finite numbers, well inside `f32`'s range — so
/// this threshold is about intent, not about the arithmetic breaking down.
const MIN_DETERMINANT: f32 = 1e-12;

/// The topmost sprite whose quad contains `point`, in world space.
///
/// "Topmost" is the same `(sort_order, entity index)` ordering
/// [`SpriteBatch::from_world`] draws in, so the sprite this returns is the one
/// whose pixels are actually visible at that point.
///
/// [`SpriteBatch::from_world`]: crate::batch::SpriteBatch::from_world
pub fn sprite_at(world: &World, point: Vec2) -> Option<Entity> {
    world
        .query2::<Transform, Sprite>()
        .filter(|(_entity, transform, _sprite)| contains(transform, point))
        .max_by_key(|(entity, _transform, sprite)| (sprite.sort_order, entity.index()))
        .map(|(entity, _transform, _sprite)| entity)
}

/// Whether `point`, in world space, falls inside this transform's quad.
///
/// Carries the point into the sprite's local space rather than building an
/// oriented bounding box in world space. Every sprite is the same axis-aligned
/// unit quad before its transform, so once the point is local the test is two
/// comparisons — and rotation and non-uniform scale come out exact with no
/// second code path.
fn contains(transform: &Transform, point: Vec2) -> bool {
    let matrix = transform.matrix();

    // A zero scale on either axis makes the matrix singular. `Mat3::inverse`
    // does not panic on one: it returns infinities and NaN, and every
    // comparison against NaN is false. `inverse_or_zero` is worse rather than
    // better here — a zero matrix sends every point to the origin, which is
    // inside the quad, so a collapsed sprite would be pickable everywhere.
    if matrix.determinant().abs() < MIN_DETERMINANT {
        return false;
    }

    // Inclusive on both edges, so a point exactly on a boundary is a hit. The
    // GPU's fill convention for that same edge is its own, so a pixel on the
    // shared border of two sprites can be picked as one and shaded as the
    // other. A sub-pixel disagreement on a measure-zero set, and not worth a
    // second rule to reconcile.
    let local = matrix.inverse().transform_point2(point);
    local.x.abs() <= Sprite::HALF_EXTENT && local.y.abs() <= Sprite::HALF_EXTENT
}
```

- [ ] **Step 4: Run the tests**

```sh
cargo test -p voltra-scene --lib pick
```

Expected: PASS, all ten.

- [ ] **Step 5: Check the workspace**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: clean.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-scene/src
git commit -m "feat(scene): add point-in-sprite picking

Inverts the transform and tests against the unit quad in local space, so
rotation and non-uniform scale need no separate path. Ties resolve by the same
key the batch sorts on, so the hit matches the pixels.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Route the viewport click

**Files:**
- Create: `crates/voltra-editor/src/picking.rs`
- Modify: `crates/voltra-editor/src/main.rs` (declare the module)
- Modify: `crates/voltra-editor/src/panels/viewport.rs` (widen the sense, call the router)

**Interfaces:**
- Consumes from Task 3: `voltra_scene::pick::sprite_at`.
- Consumes, already on `main`: `Camera2D::viewport_to_world(point: Vec2, viewport: Vec2) -> Vec2`, where `point` is viewport-local and Y-down. `UiFrame` has public `world: &mut World` and `camera: &mut Camera2D` fields.
- Produces: `picking::clicked_entity(response: &Response, frame: &UiFrame<'_>) -> Option<Entity>`.

- [ ] **Step 1: Confirm the egui API before writing against it**

Two things this task assumes. Check both in the vendored source at `C:\Users\sanpa\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\egui-0.35.0\src\` and report what you find:

- `egui::Sense::click_and_drag()` exists and produces a sense that reports both.
- `Response::clicked()` and `Response::interact_pointer_pos()` behave as expected — `interact_pointer_pos` returns the position of the interaction, in global screen points, and is `Some` on the frame of a click.

If either differs, stop and tell me rather than working around it.

- [ ] **Step 2: Create `picking.rs`**

`crates/voltra-editor/src/picking.rs`:

```rust
//! Turning a click on the scene image into a selection.
//!
//! The conversion is the reason `Camera2D::viewport_to_world` exists: egui
//! reports a pointer in screen points, the world is in world units, and the
//! camera is the only thing that knows the mapping between them.

use voltra_core::egui::Response;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::pick;

/// The entity under the pointer for the interaction `response` describes.
///
/// Returns `None` both when the click landed on empty space and when there was
/// no interaction at all, so callers must test `Response::clicked` first rather
/// than clearing the selection on every frame.
pub fn clicked_entity(response: &Response, frame: &UiFrame<'_>) -> Option<Entity> {
    let pointer = response.interact_pointer_pos()?;

    // `interact_pointer_pos` is in global screen points; the camera works in
    // viewport-local ones, so the panel's own corner comes off first.
    let local = pointer - response.rect.min;
    let viewport = Vec2::new(response.rect.width(), response.rect.height());
    let world = frame
        .camera
        .viewport_to_world(Vec2::new(local.x, local.y), viewport);

    pick::sprite_at(frame.world, world)
}
```

- [ ] **Step 3: Declare the module**

In `crates/voltra-editor/src/main.rs`, the module list becomes:

```rust
mod camera;
mod editor;
mod panels;
mod picking;
```

- [ ] **Step 4: Widen the sense and route the click**

In `crates/voltra-editor/src/panels/viewport.rs`, change the sense on the image and add the click handling. The `Sense::drag()` becomes `Sense::click_and_drag()`, and a click block goes in before the camera call:

```rust
            let scene = ui.add(
                egui::Image::new(egui::load::SizedTexture::new(frame.viewport(), available))
                    // Without this the image is inert decoration and the
                    // pointer never reaches the camera or the selection.
                    .sense(egui::Sense::click_and_drag()),
            );

            // Before navigation: a click and a drag are mutually exclusive in
            // egui, so this cannot swallow a pan.
            if scene.clicked() {
                editor.selected = crate::picking::clicked_entity(&scene, frame);
            }

            editor.camera.navigate(ui, &scene, frame.camera);
```

Leave the rest of the file — the panel frame, the size request and the DPI conversion — exactly as it is.

- [ ] **Step 5: Build and lint**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: clean. If the borrow checker objects to `frame` being borrowed immutably by `clicked_entity` and then mutably by `navigate`, the calls are sequential and the first borrow ends before the second begins — if it still objects, report the exact error rather than restructuring.

- [ ] **Step 6: Run the editor**

```sh
cargo run -p voltra-editor > editor.log 2>&1 &
sleep 8
cat editor.log
taskkill //F //IM voltra-editor.exe
```

Expected: no `ERROR`, no panic. You cannot drive a mouse, so you cannot confirm that clicking selects — do not claim you did. Report what you verified.

- [ ] **Step 7: Commit**

```sh
git add crates/voltra-editor/src
git commit -m "feat(editor): select a sprite by clicking it

The scene image only sensed drags, so the only way to select was the hierarchy
list. Clicking empty space clears the selection.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Documentation

**Files:**
- Modify: `README.md` (the controls section, and the roadmap row)
- Modify: `docs/ARCHITECTURE.md` (a Decisions entry, and the crate table)

**Interfaces:**
- Consumes: the behaviour settled in Tasks 2-4.
- Produces: nothing code depends on.

- [ ] **Step 1: Document the click in the README**

The README describes the editor just above its camera-controls table, and says selecting happens in the hierarchy. Find that paragraph — anchor on the text, not on a line number, since earlier commits have shifted it — and add a sentence saying an entity can also be selected by clicking it in the viewport, and that clicking empty space clears the selection.

Then update the roadmap table. The row `| 10 | Gizmos, picking, physics | planned |` becomes two rows so the completed part is not hidden inside a planned one:

```markdown
| 10 | Picking: click to select, stable draw order | done |
| 11 | Gizmos and physics | planned |
```

- [ ] **Step 2: Add the decision to ARCHITECTURE.md**

Append to the "Decisions" section:

```markdown
### Draw order is a sort key on the sprite, not a Z

This is a 2D engine with no depth buffer — `depth_stencil: None` in every
pipeline — so what covers what is decided entirely by the order geometry reaches
the GPU. That order used to be `World::query2`'s, which is sparse-set storage
order, and a sparse set fills the hole left by a removal with its last element.
Despawning one entity could therefore reorder two unrelated overlapping sprites.
Alpha blending is order-dependent, so that was a rendering bug before it was a
picking bug.

`Sprite::sort_order` is an `i32`, and `SpriteBatch::from_world` sorts on
`(sort_order, entity.index())` before emitting vertices.

- **An integer, not a float.** Unity's `sortingOrder` and Godot's `z_index` are
  both integers. Ties are then exact rather than dependent on a float's
  representation.
- **`entity.index()` breaks ties, and that half matters most.** Without it,
  everything sharing the default `sort_order` of 0 — which is the common case —
  falls straight back to storage order, and the fix would fix nothing.
- **Named `sort_order`, not `z_index`.** Godot's name describes an axis it has no
  relation to, and 3D is on this engine's roadmap. When a real Z exists the name
  has to still be free.

Picking uses the same ordering, in `voltra_scene::pick::sprite_at`. One
definition used twice: if they diverged, a click would select something other
than the sprite whose pixels are visible.

The hit test carries the point into the sprite's local space and compares
against the unit quad, rather than building an oriented bounding box. Rotation
and non-uniform scale then need no separate code path. A transform with zero
scale is skipped explicitly — `Mat3::inverse` returns NaN rather than panicking
on a singular matrix, and `inverse_or_zero` would be worse, since a zero matrix
maps every point to the origin and would make a collapsed sprite pickable
everywhere.

Rejected: **pixel-accurate hit-testing**, which is what `bevy_sprite`'s picking
backend does. It can, because each of its sprites carries its own texture and
therefore its own alpha. Here `Sprite` holds a colour and the renderer binds one
texture for the whole batch, so there is no per-sprite alpha to test. A quad test
is not an approximation here; it is the exact answer until sprites get their own
textures.
```

- [ ] **Step 3: Update the crate table**

In ARCHITECTURE.md's "Current crates" table, the `voltra-scene` row's "Key types" cell currently reads `Transform`, `Sprite`, `SpriteBatch`. Add `pick::sprite_at` to it, so the crate's surface is discoverable from the table.

- [ ] **Step 4: Verify the tree is green and commit**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

```sh
git add README.md docs/ARCHITECTURE.md
git commit -m "docs: record the sprite draw order decision

Says why the sort key sits on Sprite as an integer named for sorting rather than
for an axis, and why the hit test is a quad rather than a pixel.

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

All three clean. `--check` on fmt, so a stray format is a failure rather than a silent fixup.

- [ ] **Grep for the forbidden name**

```sh
git grep -in "z_index\|z-index" -- crates docs README.md
```

Expected: matches only in prose explaining why the name was *not* used. Any match in code is a defect.

- [ ] **Confirm the two orderings really are one**

```sh
git grep -n "sort_order, entity" -- crates
```

Expected: exactly two matches — `batch.rs`'s sort and `pick.rs`'s `max_by_key`. A third would mean the key has been copied somewhere it can drift.

- [ ] **Editor smoke test**

```sh
cargo run -p voltra-editor > editor.log 2>&1 &
sleep 8
cat editor.log
taskkill //F //IM voltra-editor.exe
```

Expected: no `ERROR`, no panic.

Behaviours a human must confirm at the keyboard, since no agent can drive the mouse:

1. Clicking a sprite selects it — the inspector fills in and the hierarchy row highlights.
2. Clicking empty space clears the selection.
3. With two sprites overlapping, clicking selects the one drawn on top.
4. Middle-drag still pans, and a drag does not select.
5. `Scene ▸ Spawn sprite` then deleting a middle entity does not visibly reorder the others.

- [ ] **Push**

```sh
git push -u origin feature/sprite-picking
```
