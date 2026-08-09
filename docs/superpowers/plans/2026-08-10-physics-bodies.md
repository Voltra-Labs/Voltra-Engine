# Bodies, Integration and Contacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bodies fall on a fixed timestep, overlaps are detected, and both the
shapes and the contacts are drawn over the scene — stage 11b-1. Nothing is
pushed apart yet; that is 11b-2.

**Architecture:** `RigidBody` and `Collider` are components in `voltra-scene`,
beside `Transform` and `Sprite`, because `ComponentRegistry::with_defaults`
lives there and the alternative is a dependency cycle. The new crate
`voltra-physics` holds the simulation: an accumulator, a semi-implicit
integrator, an O(n²) broad phase, three narrow-phase pairs and a debug draw that
reuses stage 11a's line pipeline.

**Tech Stack:** Rust 2021, `glam` through `voltra_render::glam`, `serde`. No new
external dependency.

Design: [`docs/superpowers/specs/2026-08-10-physics-bodies-design.md`](../specs/2026-08-10-physics-bodies-design.md).

## Global Constraints

Copied from `CLAUDE.md`, `docs/ARCHITECTURE.md` and `docs/CONVENTIONS.md`.
Every task's requirements implicitly include this section.

- The engine is **2D only**. No z, no depth, no 3D scaffolding.
- Only `voltra-core` may depend on `winit`. Only `voltra-render` may depend on
  `wgpu`. Everything else goes through `voltra_render::wgpu` and
  `voltra_render::glam`.
- **`voltra-scene` must not depend on `voltra-physics`.** The components live in
  `voltra-scene` precisely so that edge never exists; adding it creates a cycle,
  because `voltra-physics` needs `Transform`.
- All versions live in the root `[workspace.dependencies]`; member crates write
  `dep.workspace = true`. This plan adds no external dependency at all.
- No `unwrap()` outside tests. `expect("why this cannot fail")` when the
  invariant is real. Log through `log`, never `println!`.
- One concept per file. Split past roughly 300 lines or a second concept,
  `foo.rs` + `foo/`, never `foo/mod.rs`.
- Acceptance for every task: `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings` clean, then
  `cargo test --workspace` green. All three, every task, before the commit.
- **Do not commit code with no caller.** `-D warnings` rejects dead code, and an
  `#[allow(dead_code)]` outlives the reason it was added. Where a task's output
  has no consumer until a later task, say so and fold the commits together.
- Conventional Commits, scope = crate without the `voltra-` prefix, imperative
  subject ≤50 chars.
- Branch: `feature/physics-bodies`. Do not push; the dispatching session does.

## Research this plan is built on

- **Semi-implicit Euler**: velocity first, then position. Explicit Euler injects
  energy — a stack of boxes climbs. Two lines, in the right order.
- **Inverse mass, not mass**, as Box2D stores it: every formula divides by mass,
  and infinite mass is `0.0` rather than a branch in each of them.
- **The solver is already chosen for 11b-2** and is not built here. Erin Catto
  compared eight solvers in [Solver2D](https://box2d.org/posts/2024/02/solver2d/)
  and took TGS_Soft ("Soft Step") for Box2D v3; XPBD lost on friction and on
  precision far from the origin. Recorded so 11b-2 does not restart the
  question.
- **In-house rather than Rapier2D**, because `bevy_rapier` *"has to maintain a
  separate physics world and synchronize a ton of data with Bevy each frame"*
  and `avian` exists to avoid exactly that. With a hand-written ECS, adopting
  Rapier reproduces the problem `avian` was written to solve.

## File Structure

**Created in `voltra-scene`**

- `src/body.rs` — `BodyType`, `RigidBody`.
- `src/collider.rs` — `Collider`, and `world_aabb` given a `Transform`.

**Created as `crates/voltra-physics`**

- `Cargo.toml`, `src/lib.rs`
- `src/clock.rs` — `PhysicsClock`.
- `src/integrate.rs` — `integrate`.
- `src/broad.rs` — `candidate_pairs`.
- `src/narrow.rs` — `Contact`, `contact`.
- `src/step.rs` — `step`.
- `src/debug.rs` — `draw`.

**Modified**

- `crates/voltra-scene/src/lib.rs`, `src/format/registry.rs` — the modules and
  the two `register` calls.
- `Cargo.toml` — the workspace member and dependency entry.
- `crates/voltra-core/src/app.rs` — the clock, the gravity, the per-frame step.
- `crates/voltra-editor/` — the debug-draw toggle.
- `docs/ARCHITECTURE.md`, `README.md`.

## Execution waves

| Wave | Tasks | Why they cannot move |
| --- | --- | --- |
| 1 | Task 1 | Every later task needs the components. |
| 2 | Task 2, Task 3 | Disjoint files: the clock and the integrator vs the two phases of detection. |
| 3 | Task 4 | Needs all of the above. |
| 4 | Task 5 | Needs `step` and `Contact`. |
| 5 | Task 6 | Documents what the rest decided. |

---

### Task 1: The two components

**Files:**
- Create: `crates/voltra-scene/src/body.rs`
- Create: `crates/voltra-scene/src/collider.rs`
- Modify: `crates/voltra-scene/src/lib.rs`
- Modify: `crates/voltra-scene/src/format/registry.rs`

**Interfaces:**
- Produces:
  - `voltra_scene::{BodyType, RigidBody, Collider}`
  - `BodyType::{Static, Kinematic, Dynamic}`, `Dynamic` is **not** the default —
    see below
  - `RigidBody::{new_dynamic(mass: f32) -> Self, new_static() -> Self}`
  - `Collider::world_aabb(&self, transform: &Transform) -> (Vec2, Vec2)`
    returning `(min, max)`
  - `Collider::world_extent(&self, transform: &Transform)` — the scaled half
    extents or radius

- [ ] **Step 1: Write the failing component tests**

Create `crates/voltra-scene/src/body.rs` with only its test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_body_is_static_and_immovable() {
        // Static, not Dynamic: a component added by a click in the inspector
        // must not make the sprite fall off the screen before anyone has typed
        // a mass. Unity, Godot and Box2D all default to the inert body too.
        let body = RigidBody::default();

        assert_eq!(body.body_type, BodyType::Static);
        assert_eq!(body.inverse_mass, 0.0);
        assert_eq!(body.velocity, Vec2::ZERO);
    }

    #[test]
    fn a_dynamic_body_stores_the_reciprocal_of_its_mass() {
        let body = RigidBody::new_dynamic(4.0);

        assert_eq!(body.body_type, BodyType::Dynamic);
        assert_eq!(body.inverse_mass, 0.25);
    }

    #[test]
    fn a_zero_mass_body_is_infinitely_massive_not_infinitely_light() {
        // 1/0 is inf, and an inf inverse mass makes a body react to the
        // smallest impulse by leaving the world. Zero mass means "cannot be
        // moved", which is what every engine means by it.
        let body = RigidBody::new_dynamic(0.0);

        assert_eq!(body.inverse_mass, 0.0);
        assert!(body.inverse_mass.is_finite());
    }

    #[test]
    fn a_negative_mass_is_treated_as_zero() {
        // A scene file is external input and can say anything.
        assert_eq!(RigidBody::new_dynamic(-5.0).inverse_mass, 0.0);
    }

    #[test]
    fn a_static_body_has_no_inverse_mass() {
        assert_eq!(RigidBody::new_static().inverse_mass, 0.0);
        assert_eq!(RigidBody::new_static().body_type, BodyType::Static);
    }

    #[test]
    fn a_body_round_trips_through_ron() {
        let body = RigidBody {
            body_type: BodyType::Kinematic,
            velocity: Vec2::new(1.5, -2.5),
            inverse_mass: 0.5,
            gravity_scale: 2.0,
            linear_damping: 0.1,
        };

        let text = ron::to_string(&body).expect("serialise");
        let back: RigidBody = ron::from_str(&text).expect("deserialise");

        assert_eq!(back, body);
    }
}
```

Create `crates/voltra-scene/src/collider.rs` with only its test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_aabb_spans_twice_its_half_extents() {
        let collider = Collider::Aabb {
            half_extents: Vec2::new(2.0, 3.0),
        };
        let (min, max) = collider.world_aabb(&Transform::default());

        assert_eq!(min, Vec2::new(-2.0, -3.0));
        assert_eq!(max, Vec2::new(2.0, 3.0));
    }

    #[test]
    fn an_aabb_follows_the_transform_translation() {
        let collider = Collider::Aabb {
            half_extents: Vec2::splat(1.0),
        };
        let transform = Transform::from_translation(Vec2::new(10.0, -4.0));
        let (min, max) = collider.world_aabb(&transform);

        assert_eq!(min, Vec2::new(9.0, -5.0));
        assert_eq!(max, Vec2::new(11.0, -3.0));
    }

    #[test]
    fn a_collider_is_scaled_by_its_transform() {
        // A scaled sprite with an unscaled collider is a bug that looks like a
        // physics bug: the outline and the picture disagree and neither is
        // obviously wrong.
        let collider = Collider::Aabb {
            half_extents: Vec2::splat(1.0),
        };
        let transform = Transform::default().with_scale(Vec2::new(3.0, 5.0));
        let (min, max) = collider.world_aabb(&transform);

        assert_eq!(min, Vec2::new(-3.0, -5.0));
        assert_eq!(max, Vec2::new(3.0, 5.0));
    }

    #[test]
    fn a_negative_scale_still_gives_a_min_below_its_max() {
        // A mirrored sprite is a negative scale, and an AABB whose min exceeds
        // its max overlaps nothing — the collider would silently stop working.
        let collider = Collider::Aabb {
            half_extents: Vec2::splat(2.0),
        };
        let transform = Transform::default().with_scale(Vec2::new(-1.0, 1.0));
        let (min, max) = collider.world_aabb(&transform);

        assert!(min.x <= max.x && min.y <= max.y, "got {min:?}..{max:?}");
    }

    #[test]
    fn a_circle_takes_the_larger_axis_of_a_non_uniform_scale() {
        // A true ellipse is a different shape, not a parameter. Taking the
        // larger axis keeps the collider covering the sprite rather than
        // cutting into it.
        let collider = Collider::Circle { radius: 1.0 };
        let transform = Transform::default().with_scale(Vec2::new(2.0, 5.0));

        assert_eq!(collider.world_radius(&transform), 5.0);
    }

    #[test]
    fn a_circles_aabb_is_its_bounding_square() {
        let collider = Collider::Circle { radius: 2.0 };
        let (min, max) = collider.world_aabb(&Transform::default());

        assert_eq!(min, Vec2::splat(-2.0));
        assert_eq!(max, Vec2::splat(2.0));
    }

    #[test]
    fn a_collider_round_trips_through_ron() {
        for collider in [
            Collider::Aabb {
                half_extents: Vec2::new(1.0, 2.0),
            },
            Collider::Circle { radius: 3.0 },
        ] {
            let text = ron::to_string(&collider).expect("serialise");
            let back: Collider = ron::from_str(&text).expect("deserialise");
            assert_eq!(back, collider);
        }
    }
}
```

`ron` must be a `[dev-dependencies]` entry of `voltra-scene` for those two round
trips. Check first — the scene format already uses `ron`, so it is likely a
normal dependency already; if so, add nothing.

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p voltra-scene body:: collider::`
Expected: FAIL — `cannot find type 'RigidBody' in this scope`.

- [ ] **Step 3: Write the components**

`crates/voltra-scene/src/body.rs`, above its tests:

```rust
//! What moves, and how much it resists being moved.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

/// How a body responds to time and, later, to contacts.
///
/// The three every engine converged on. Kinematic is not a special case of the
/// other two: it moves under its own velocity like a dynamic body, and is
/// immovable by contacts like a static one, which is what a moving platform is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BodyType {
    /// Never moves. Walls, floors.
    ///
    /// The default because a `RigidBody` added from the inspector must not make
    /// the sprite fall off the screen before a mass has been typed.
    #[default]
    Static,
    /// Moves by its velocity, ignoring gravity and — in 11b-2 — impulses.
    Kinematic,
    /// Moves by velocity and gravity, and will be pushed by contacts.
    Dynamic,
}

/// A body in the simulation.
///
/// Stores `inverse_mass` rather than mass, as Box2D does: every formula that
/// uses it divides by mass, and infinite mass — anything that cannot be pushed
/// — is `0.0` instead of a branch repeated at each of them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RigidBody {
    pub body_type: BodyType,
    pub velocity: Vec2,
    /// `1.0 / mass`, or `0.0` for a body that cannot be pushed.
    pub inverse_mass: f32,
    /// Multiplier on the world's gravity. `0.0` for a floating body.
    pub gravity_scale: f32,
    /// Fraction of speed shed per second. `0.0` keeps all of it.
    pub linear_damping: f32,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self {
            body_type: BodyType::Static,
            velocity: Vec2::ZERO,
            inverse_mass: 0.0,
            gravity_scale: 1.0,
            linear_damping: 0.0,
        }
    }
}

impl RigidBody {
    /// A dynamic body of `mass`.
    ///
    /// A mass of zero or less means "cannot be pushed" rather than "infinitely
    /// light": `1.0 / 0.0` is infinity, and an infinite inverse mass sends the
    /// body out of the world on the first contact. Every engine reads zero the
    /// same way.
    pub fn new_dynamic(mass: f32) -> Self {
        Self {
            body_type: BodyType::Dynamic,
            inverse_mass: if mass > 0.0 { 1.0 / mass } else { 0.0 },
            ..Default::default()
        }
    }

    pub fn new_static() -> Self {
        Self::default()
    }
}
```

`crates/voltra-scene/src/collider.rs`, above its tests:

```rust
//! The shape an entity occupies, at the origin.
//!
//! Where it sits comes from the entity's [`Transform`], so a collider is a size
//! and nothing else. That is also why an [`Collider::Aabb`] does not rotate:
//! it is axis-aligned in *world* space, not in the entity's. A rotated sprite
//! keeps an upright box, which is a real limitation and the reason oriented
//! boxes and polygons are on 11b-3 rather than pretended at here.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use crate::Transform;

/// The shape used for collision.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Collider {
    /// Axis-aligned box, centred on the transform.
    Aabb { half_extents: Vec2 },
    Circle { radius: f32 },
}

impl Collider {
    /// The world-space bounds, as `(min, max)`.
    ///
    /// Scale is applied, and its absolute value is taken: a mirrored sprite has
    /// a negative scale, and a box whose min exceeds its max overlaps nothing —
    /// the collider would silently stop working on exactly the sprites that
    /// face left.
    pub fn world_aabb(&self, transform: &Transform) -> (Vec2, Vec2) {
        let half = match self {
            Self::Aabb { half_extents } => (*half_extents * transform.scale).abs(),
            Self::Circle { .. } => Vec2::splat(self.world_radius(transform)),
        };
        (transform.translation - half, transform.translation + half)
    }

    /// The scaled radius, for a circle. Zero for a box.
    ///
    /// A non-uniform scale takes the larger axis. A true ellipse is a different
    /// shape rather than a parameter of this one, and the larger axis keeps the
    /// collider covering the sprite instead of cutting into it.
    pub fn world_radius(&self, transform: &Transform) -> f32 {
        match self {
            Self::Circle { radius } => {
                radius.abs() * transform.scale.abs().max_element()
            }
            Self::Aabb { .. } => 0.0,
        }
    }
}
```

- [ ] **Step 4: Export and register them**

In `crates/voltra-scene/src/lib.rs`, add `pub mod body;` and `pub mod collider;`
alphabetically, and `pub use body::{BodyType, RigidBody};` plus
`pub use collider::Collider;` beside the other re-exports.

In `crates/voltra-scene/src/format/registry.rs`, extend `with_defaults`:

```rust
        registry.register::<Transform>("Transform");
        registry.register::<Sprite>("Sprite");
        registry.register::<RigidBody>("RigidBody");
        registry.register::<Collider>("Collider");
```

Registering here rather than making callers opt in is deliberate: there are
several `with_defaults` call sites, and forgetting one would not fail — it would
silently drop the component from that save.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p voltra-scene`
Expected: PASS, thirteen new tests among the existing ones.

- [ ] **Step 6: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 7: Commit**

```sh
git add crates/voltra-scene/src/body.rs crates/voltra-scene/src/collider.rs crates/voltra-scene/src/lib.rs crates/voltra-scene/src/format/registry.rs
git commit -m "feat(scene): add rigid body and collider components"
```

---

### Task 2: The crate, its clock and its integrator

**Files:**
- Create: `crates/voltra-physics/Cargo.toml`, `src/lib.rs`, `src/clock.rs`,
  `src/integrate.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces:
  - `PhysicsClock::{new(step: f32), steps(&mut self, delta: f32) -> u32, step(&self) -> f32}`
  - `PhysicsClock::default()` — 1/60 s, 8 steps maximum
  - `integrate(world: &mut World, gravity: Vec2, dt: f32)`

- [ ] **Step 1: Create the crate**

`crates/voltra-physics/Cargo.toml`:

```toml
[package]
name = "voltra-physics"
description = "2D rigid bodies, integration and contact detection."
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
voltra-ecs.workspace = true
voltra-scene.workspace = true
voltra-render.workspace = true
log.workspace = true
```

`voltra-render` is for `glam` and, in Task 5, `LineBatch` — the same re-export
route every non-render crate takes. In the root `Cargo.toml`, add
`voltra-physics = { path = "crates/voltra-physics" }` alphabetically among the
`voltra-*` entries. `members = ["crates/*"]` picks the directory up already.

`crates/voltra-physics/src/lib.rs`:

```rust
//! 2D rigid bodies, integration and contact detection.
//!
//! The components are in `voltra-scene`, not here: `ComponentRegistry` lives
//! there, and registering physics components from there would need
//! `voltra-scene → voltra-physics`, while integration needs `Transform` and so
//! needs the edge back. This crate is the simulation over those components and
//! holds nothing a scene file contains.
//!
//! Nothing in this stage resolves a contact. Bodies move, overlaps are found
//! and reported, and a body will sink through a floor until 11b-2.

pub mod broad;
pub mod clock;
pub mod debug;
pub mod integrate;
pub mod narrow;
pub mod step;

pub use clock::PhysicsClock;
pub use narrow::Contact;
pub use step::step;
```

Create the other module files empty enough to compile as they are written; the
`lib.rs` above is its final form, so expect it not to compile until Task 4.
Write `lib.rs` last if that is easier — the compiler is the guide.

- [ ] **Step 2: Write the failing clock tests**

`crates/voltra-physics/src/clock.rs`, test module only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const STEP: f32 = 1.0 / 60.0;

    #[test]
    fn a_delta_below_one_step_runs_nothing() {
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.steps(STEP * 0.5), 0);
    }

    #[test]
    fn exactly_one_step_runs_once() {
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.steps(STEP), 1);
    }

    #[test]
    fn two_steps_worth_runs_twice() {
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.steps(STEP * 2.0), 2);
    }

    #[test]
    fn the_remainder_carries_to_the_next_frame() {
        // Two frames of 0.75 steps owe one step and then one more, not zero
        // and then one. Dropping the remainder makes physics run slow by a
        // fraction that depends on the frame rate.
        let mut clock = PhysicsClock::default();

        assert_eq!(clock.steps(STEP * 0.75), 0);
        assert_eq!(clock.steps(STEP * 0.75), 1);
    }

    #[test]
    fn a_huge_delta_is_capped() {
        // The spiral of death: running every owed step makes the next frame
        // longer, which owes more steps again. Every engine drops the debt.
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.steps(STEP * 1000.0), 8);
    }

    #[test]
    fn the_accumulator_does_not_keep_the_dropped_debt() {
        // Otherwise the cap only defers the spiral: the next frame starts
        // already owing 992 steps.
        let mut clock = PhysicsClock::default();
        clock.steps(STEP * 1000.0);

        assert_eq!(clock.steps(0.0), 0);
    }

    #[test]
    fn a_negative_delta_runs_nothing_and_does_not_rewind() {
        let mut clock = PhysicsClock::default();
        assert_eq!(clock.steps(-1.0), 0);
        assert_eq!(clock.steps(STEP), 1);
    }

    #[test]
    fn a_custom_step_is_honoured() {
        let mut clock = PhysicsClock::new(0.5);
        assert_eq!(clock.steps(1.0), 2);
        assert_eq!(clock.step(), 0.5);
    }
}
```

- [ ] **Step 3: Write the clock**

```rust
//! How many fixed steps a variable frame owes.
//!
//! Physics integrated with the render delta changes behaviour with the frame
//! rate — the same scene settles differently at 60 and 144 Hz, and a stall
//! makes it explode. A fixed step with an accumulator is what every engine does
//! instead.

/// The fixed step and the debt carried between frames.
#[derive(Debug, Clone)]
pub struct PhysicsClock {
    accumulator: f32,
    step: f32,
    max_steps: u32,
}

/// Steps allowed in one frame.
///
/// The spiral of death is the reason there is a cap at all: a frame slow enough
/// to owe many steps gets slower by running them, and then owes more. Past this
/// the debt is dropped and simulated time runs slow, which is what every engine
/// chooses — the alternative is a hang.
const MAX_STEPS: u32 = 8;

impl Default for PhysicsClock {
    fn default() -> Self {
        // 60 Hz: fast enough that a falling body does not tunnel at ordinary
        // speeds, slow enough to leave the frame budget alone.
        Self::new(1.0 / 60.0)
    }
}

impl PhysicsClock {
    pub fn new(step: f32) -> Self {
        Self {
            accumulator: 0.0,
            // A non-positive step would divide by zero below, and a scene or a
            // caller can say anything.
            step: if step > 0.0 { step } else { 1.0 / 60.0 },
            max_steps: MAX_STEPS,
        }
    }

    /// How many steps to run for a frame of `delta` seconds.
    ///
    /// The remainder stays in the accumulator, so a frame that owes three
    /// quarters of a step is not simply lost.
    pub fn steps(&mut self, delta: f32) -> u32 {
        if delta > 0.0 {
            self.accumulator += delta;
        }

        let owed = (self.accumulator / self.step) as u32;
        if owed >= self.max_steps {
            // Dropped rather than carried: carrying it means the cap only
            // defers the spiral to the next frame.
            self.accumulator = 0.0;
            return self.max_steps;
        }

        self.accumulator -= owed as f32 * self.step;
        owed
    }

    /// The fixed step, in seconds. What to pass to `step`.
    pub fn step(&self) -> f32 {
        self.step
    }
}
```

- [ ] **Step 4: Write the failing integrator tests**

`crates/voltra-physics/src/integrate.rs`, test module only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{BodyType, RigidBody};

    const DT: f32 = 1.0 / 60.0;
    const G: Vec2 = Vec2::new(0.0, -10.0);

    /// A world with one body at the origin, and its entity.
    fn world_with(body: RigidBody) -> (World, Entity) {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(entity, body);
        (world, entity)
    }

    fn transform_of(world: &World, entity: Entity) -> Transform {
        *world.get::<Transform>(entity).expect("the transform is there")
    }

    #[test]
    fn a_dynamic_body_falls() {
        let (mut world, entity) = world_with(RigidBody::new_dynamic(1.0));

        integrate(&mut world, G, DT);

        assert!(transform_of(&world, entity).translation.y < 0.0);
    }

    #[test]
    fn a_static_body_never_moves() {
        let mut body = RigidBody::new_static();
        body.velocity = Vec2::new(100.0, 100.0);
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        assert_eq!(transform_of(&world, entity).translation, Vec2::ZERO);
    }

    #[test]
    fn a_kinematic_body_moves_but_ignores_gravity() {
        let mut body = RigidBody::default();
        body.body_type = BodyType::Kinematic;
        body.velocity = Vec2::new(2.0, 0.0);
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        assert_eq!(transform_of(&world, entity).translation.x, 2.0 * DT);
        assert_eq!(transform_of(&world, entity).translation.y, 0.0);
        assert_eq!(
            world.get::<RigidBody>(entity).expect("body").velocity.y,
            0.0,
            "gravity must not touch a kinematic body's velocity"
        );
    }

    #[test]
    fn velocity_is_updated_before_position() {
        // Semi-implicit Euler. Explicit Euler — position from the *old*
        // velocity — injects energy, and a stack of boxes slowly climbs.
        // After one step from rest the two differ by exactly g·dt².
        let (mut world, entity) = world_with(RigidBody::new_dynamic(1.0));

        integrate(&mut world, G, DT);

        let expected = G.y * DT * DT;
        let actual = transform_of(&world, entity).translation.y;
        assert!(
            (actual - expected).abs() < 1e-9,
            "got {actual}, expected {expected} — explicit Euler would give 0"
        );
    }

    #[test]
    fn gravity_scale_zero_makes_a_body_float() {
        let mut body = RigidBody::new_dynamic(1.0);
        body.gravity_scale = 0.0;
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        assert_eq!(transform_of(&world, entity).translation, Vec2::ZERO);
    }

    #[test]
    fn damping_reduces_speed_without_reversing_it() {
        let mut body = RigidBody::new_dynamic(1.0);
        body.velocity = Vec2::new(10.0, 0.0);
        body.gravity_scale = 0.0;
        body.linear_damping = 0.5;
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        let v = world.get::<RigidBody>(entity).expect("body").velocity.x;
        assert!(v < 10.0 && v > 0.0, "got {v}");
    }

    #[test]
    fn enormous_damping_stops_a_body_rather_than_reversing_it() {
        // `v *= 1 - damping·dt` goes negative once damping·dt exceeds one,
        // which turns a brake into a catapult. A scene file can say 10000.
        let mut body = RigidBody::new_dynamic(1.0);
        body.velocity = Vec2::new(10.0, 0.0);
        body.gravity_scale = 0.0;
        body.linear_damping = 10_000.0;
        let (mut world, entity) = world_with(body);

        integrate(&mut world, G, DT);

        let v = world.get::<RigidBody>(entity).expect("body").velocity.x;
        assert!(v >= 0.0, "damping reversed the velocity: {v}");
    }

    #[test]
    fn a_body_without_a_transform_is_skipped() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, RigidBody::new_dynamic(1.0));

        integrate(&mut world, G, DT);

        assert!(world.get::<Transform>(entity).is_none());
    }
}
```

- [ ] **Step 5: Write the integrator**

```rust
//! Moving bodies forward by one fixed step.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{BodyType, RigidBody, Transform};

/// Advances every body by `dt`.
///
/// Semi-implicit (symplectic) Euler: velocity is updated from the acceleration
/// *first*, then position from the new velocity. Explicit Euler — position from
/// the old velocity — adds energy to the system, and a stack of boxes slowly
/// climbs. The correct order costs nothing but being written down.
pub fn integrate(world: &mut World, gravity: Vec2, dt: f32) {
    let moved: Vec<(Entity, Vec2, Vec2)> = world
        .query::<RigidBody>()
        .filter_map(|(entity, body)| {
            let velocity = match body.body_type {
                BodyType::Static => return None,
                BodyType::Kinematic => body.velocity,
                BodyType::Dynamic => {
                    let accelerated = body.velocity + gravity * body.gravity_scale * dt;
                    // Clamped at zero: `1 - damping·dt` goes negative once the
                    // damping is large enough, which turns a brake into a
                    // catapult. A scene file can contain any number.
                    let retained = (1.0 - body.linear_damping * dt).max(0.0);
                    accelerated * retained
                }
            };
            Some((entity, velocity, velocity * dt))
        })
        .collect();

    for (entity, velocity, delta) in moved {
        if let Some(body) = world.get_mut::<RigidBody>(entity) {
            body.velocity = velocity;
        }
        // A body with no transform integrates its velocity and moves nothing.
        // Legitimate: not everything simulated is drawn.
        if let Some(transform) = world.get_mut::<Transform>(entity) {
            transform.translation += delta;
        }
    }
}
```

The collect-then-apply shape is because `World::query` borrows immutably and
`get_mut` needs the borrow back. If `voltra-ecs` has a `query_mut` that yields
`(Entity, &mut RigidBody)` — `ui_frame.rs` uses one — prefer it and drop the
intermediate `Vec`, but only if the same loop can still reach `Transform`; two
components mutably at once may not be expressible, and the `Vec` is then
correct rather than lazy. Say which one you used in the report.

- [ ] **Step 6: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p voltra-physics
```

`lib.rs` names modules that do not exist yet, so either write stub files for
`broad`, `narrow`, `step` and `debug` containing only a doc comment, or trim
`lib.rs` to the modules that exist and restore it in Task 4. Do not commit a
`lib.rs` that does not compile.

- [ ] **Step 7: Commit**

```sh
git add Cargo.toml Cargo.lock crates/voltra-physics
git commit -m "feat(physics): add a fixed clock and an integrator"
```

---

### Task 3: Detection, both phases

**Files:**
- Create: `crates/voltra-physics/src/broad.rs`, `src/narrow.rs`

**Interfaces:**
- Produces:
  - `broad::candidate_pairs(world: &World) -> Vec<(Entity, Entity)>`
  - `narrow::Contact { a, b, normal, penetration, point }`
  - `narrow::contact(a: (&Collider, &Transform), b: (&Collider, &Transform)) -> Option<(Vec2, f32, Vec2)>`
    returning `(normal, penetration, point)`

- [ ] **Step 1: Write the failing broad-phase tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Collider, Transform};

    fn spawn(world: &mut World, at: Vec2, half: f32) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(
            entity,
            Collider::Aabb {
                half_extents: Vec2::splat(half),
            },
        );
        entity
    }

    #[test]
    fn distant_bodies_are_not_a_pair() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);
        spawn(&mut world, Vec2::new(100.0, 0.0), 1.0);

        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn overlapping_bodies_are_a_pair() {
        let mut world = World::new();
        let a = spawn(&mut world, Vec2::ZERO, 1.0);
        let b = spawn(&mut world, Vec2::new(0.5, 0.0), 1.0);

        assert_eq!(candidate_pairs(&world), vec![(a, b)]);
    }

    #[test]
    fn a_pair_appears_once_and_never_mirrored() {
        // (a,b) and (b,a) are the same contact. Emitting both doubles every
        // impulse the solver will apply in 11b-2.
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);
        spawn(&mut world, Vec2::new(0.5, 0.0), 1.0);

        assert_eq!(candidate_pairs(&world).len(), 1);
    }

    #[test]
    fn a_body_is_never_paired_with_itself() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);

        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn an_entity_without_a_collider_is_never_in_a_pair() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 1.0);
        let bare = world.spawn();
        world.insert(bare, Transform::default());

        assert!(candidate_pairs(&world).is_empty());
    }

    #[test]
    fn three_overlapping_bodies_give_three_pairs() {
        let mut world = World::new();
        spawn(&mut world, Vec2::ZERO, 2.0);
        spawn(&mut world, Vec2::new(0.5, 0.0), 2.0);
        spawn(&mut world, Vec2::new(1.0, 0.0), 2.0);

        assert_eq!(candidate_pairs(&world).len(), 3);
    }
}
```

- [ ] **Step 2: Write the broad phase**

```rust
//! Which pairs are worth an exact test.
//!
//! O(n²) with a world-space AABB rejection, deliberately. For the tens of
//! bodies a scene holds today this beats a spatial hash, which pays for
//! bucketing before it saves anything.
//!
//! **Replace it past roughly 200 bodies** — n² is then 20 000 pair tests per
//! step at 60 Hz. Sweep-and-prune on the x axis is the replacement, because a
//! 2D world is wide. The signature below does not change when it happens: that
//! is what it is for.

use voltra_ecs::{Entity, World};
use voltra_scene::{Collider, Transform};

/// Pairs whose bounds overlap, each once, with `a.index() < b.index()`.
pub fn candidate_pairs(world: &World) -> Vec<(Entity, Entity)> {
    let bounds: Vec<(Entity, (Vec2, Vec2))> = world
        .query::<Collider>()
        .filter_map(|(entity, collider)| {
            let transform = world.get::<Transform>(entity)?;
            Some((entity, collider.world_aabb(transform)))
        })
        .collect();

    let mut pairs = Vec::new();
    for (i, (a, a_bounds)) in bounds.iter().enumerate() {
        for (b, b_bounds) in &bounds[i + 1..] {
            if overlaps(*a_bounds, *b_bounds) {
                pairs.push((*a, *b));
            }
        }
    }
    pairs
}

/// Whether two `(min, max)` boxes share any area.
fn overlaps((a_min, a_max): (Vec2, Vec2), (b_min, b_max): (Vec2, Vec2)) -> bool {
    a_min.x <= b_max.x && a_max.x >= b_min.x && a_min.y <= b_max.y && a_max.y >= b_min.y
}
```

Add `use voltra_render::glam::Vec2;`. Starting the inner loop at `i + 1` is what
gives each pair once and never mirrored, and never pairs a body with itself.

- [ ] **Step 3: Write the failing narrow-phase tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32) -> Transform {
        Transform::from_translation(Vec2::new(x, y))
    }

    fn circle(r: f32) -> Collider {
        Collider::Circle { radius: r }
    }

    fn boxed(half: f32) -> Collider {
        Collider::Aabb {
            half_extents: Vec2::splat(half),
        }
    }

    #[test]
    fn separated_circles_do_not_touch() {
        assert!(contact((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(5.0, 0.0))).is_none());
    }

    #[test]
    fn overlapping_circles_report_the_overlap_along_the_centre_line() {
        let (normal, penetration, _) =
            contact((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0)))
                .expect("they overlap");

        // The normal pushes `a` away from `b`, so it points in -x.
        assert!((normal - Vec2::new(-1.0, 0.0)).length() < 1e-6, "{normal:?}");
        assert!((penetration - 0.5).abs() < 1e-6, "{penetration}");
    }

    #[test]
    fn circles_touching_exactly_do_not_report_a_contact() {
        // Zero penetration is not a collision, and reporting it gives the
        // solver a contact with nothing to resolve every frame two bodies rest
        // against each other.
        assert!(contact((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(2.0, 0.0))).is_none());
    }

    #[test]
    fn concentric_circles_give_a_unit_normal_rather_than_nan() {
        // Normalising a zero vector is NaN, and a NaN normal spreads into
        // every velocity the solver touches.
        let (normal, penetration, _) =
            contact((&circle(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(0.0, 0.0)))
                .expect("fully overlapping");

        assert!(normal.is_finite(), "{normal:?}");
        assert!((normal.length() - 1.0).abs() < 1e-6, "{normal:?}");
        assert!(penetration.is_finite() && penetration > 0.0);
    }

    #[test]
    fn boxes_report_the_axis_of_least_penetration() {
        // Overlapping 0.2 in x and 1.5 in y: the way out is x, because it is
        // nearer. Choosing the deeper axis pushes a box through its neighbour.
        let a = (&boxed(1.0), &at(0.0, 0.0));
        let b = (&boxed(1.0), &at(1.8, 0.5));
        let (normal, penetration, _) = contact(a, b).expect("they overlap");

        assert!(normal.y.abs() < 1e-6, "expected an x normal, got {normal:?}");
        assert!((penetration - 0.2).abs() < 1e-6, "{penetration}");
    }

    #[test]
    fn separated_boxes_do_not_touch() {
        assert!(contact((&boxed(1.0), &at(0.0, 0.0)), (&boxed(1.0), &at(3.0, 0.0))).is_none());
    }

    #[test]
    fn a_circle_beside_a_box_pushes_out_of_the_nearest_face() {
        let (normal, penetration, _) =
            contact((&boxed(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0)))
                .expect("they overlap");

        assert!((normal - Vec2::new(-1.0, 0.0)).length() < 1e-6, "{normal:?}");
        assert!((penetration - 0.5).abs() < 1e-6, "{penetration}");
    }

    #[test]
    fn a_circle_at_a_boxs_centre_still_gives_a_finite_normal() {
        // The closest point on the box to the centre *is* the centre, so the
        // usual difference is zero and normalising it is NaN. The nearest face
        // is the answer instead.
        let (normal, penetration, _) =
            contact((&boxed(2.0), &at(0.0, 0.0)), (&circle(0.5), &at(0.0, 0.0)))
                .expect("fully inside");

        assert!(normal.is_finite() && (normal.length() - 1.0).abs() < 1e-6, "{normal:?}");
        assert!(penetration > 0.0);
    }

    #[test]
    fn a_box_beside_a_circle_mirrors_the_circle_beside_a_box() {
        // Argument order must not change the physics, only the normal's sign.
        let one = contact((&boxed(1.0), &at(0.0, 0.0)), (&circle(1.0), &at(1.5, 0.0)))
            .expect("overlap");
        let other = contact((&circle(1.0), &at(1.5, 0.0)), (&boxed(1.0), &at(0.0, 0.0)))
            .expect("overlap");

        assert!((one.0 + other.0).length() < 1e-6, "{:?} {:?}", one.0, other.0);
        assert!((one.1 - other.1).abs() < 1e-6);
    }

    #[test]
    fn a_zero_sized_collider_never_collides() {
        // A scene file can contain a zero or negative radius, and an inverted
        // shape would report a contact with a backwards normal.
        assert!(contact((&circle(0.0), &at(0.0, 0.0)), (&circle(1.0), &at(0.0, 0.0))).is_none());
        assert!(contact(
            (&Collider::Aabb { half_extents: Vec2::ZERO }, &at(0.0, 0.0)),
            (&boxed(1.0), &at(0.0, 0.0))
        )
        .is_none());
    }

    #[test]
    fn scale_changes_whether_two_bodies_touch() {
        let big = Transform::default().with_scale(Vec2::splat(4.0));
        assert!(contact((&circle(1.0), &big), (&circle(1.0), &at(3.0, 0.0))).is_some());
        assert!(contact(
            (&circle(1.0), &Transform::default()),
            (&circle(1.0), &at(3.0, 0.0))
        )
        .is_none());
    }
}
```

- [ ] **Step 4: Write the narrow phase**

Implement `contact` with the three pairs. The rules, restated so they are not
improvised:

- Circle–circle: `d = a.pos − b.pos`; overlap when `|d| < ra + rb`; normal is
  `d.normalize()`, or `Vec2::X` when `|d|` is under an epsilon; penetration is
  `ra + rb − |d|`; point is `b.pos + normal · rb`.
- AABB–AABB: overlap on both axes; `overlap = (a_half + b_half) − |a.pos − b.pos|`
  componentwise, positive on both axes to collide. The normal is the axis of the
  *smaller* overlap, signed away from `b`; penetration is that smaller overlap.
- AABB–circle: clamp the circle's centre into the box to get the closest point.
  If it differs from the centre, it is a circle-vs-point test. If it does not —
  the centre is inside — use the nearest face: the axis whose remaining distance
  to a face is smallest, penetration `half + r − |offset|` on that axis.
- Zero or negative extent or radius: return `None` before any of it.
- The normal always pushes **`a` away from `b`**. Swapping the arguments must
  negate the normal and change nothing else; there is a test for it.

Anything with `.normalize()` in it needs the zero-length case handled first;
`glam`'s `normalize` of a zero vector is NaN, not an error.

- [ ] **Step 5: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p voltra-physics
```

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-physics/src/broad.rs crates/voltra-physics/src/narrow.rs crates/voltra-physics/src/lib.rs
git commit -m "feat(physics): detect overlaps between bodies"
```

---

### Task 4: One step

**Files:**
- Create: `crates/voltra-physics/src/step.rs`
- Modify: `crates/voltra-physics/src/lib.rs`

**Interfaces:**
- Produces: `step(world: &mut World, gravity: Vec2, dt: f32) -> Vec<Contact>`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::{Collider, RigidBody, Transform};

    const DT: f32 = 1.0 / 60.0;
    const G: Vec2 = Vec2::new(0.0, -10.0);

    #[test]
    fn a_falling_body_eventually_reaches_the_floor() {
        // The whole stage in one test: it falls, and the overlap is reported.
        // It does not stop — nothing resolves contacts until 11b-2 — so this
        // asserts a contact appears, not that the body rests.
        let mut world = World::new();

        let floor = world.spawn();
        world.insert(floor, Transform::from_translation(Vec2::new(0.0, -5.0)));
        world.insert(
            floor,
            Collider::Aabb {
                half_extents: Vec2::new(10.0, 1.0),
            },
        );
        world.insert(floor, RigidBody::new_static());

        let ball = world.spawn();
        world.insert(ball, Transform::default());
        world.insert(ball, Collider::Circle { radius: 0.5 });
        world.insert(ball, RigidBody::new_dynamic(1.0));

        let mut contacts = Vec::new();
        for _ in 0..240 {
            contacts = step(&mut world, G, DT);
            if !contacts.is_empty() {
                break;
            }
        }

        assert_eq!(contacts.len(), 1, "one contact between ball and floor");
        assert!(
            contacts[0].normal.y > 0.5,
            "the floor must push the ball up, got {:?}",
            contacts[0].normal
        );
    }

    #[test]
    fn an_empty_world_steps_without_contacts() {
        let mut world = World::new();
        assert!(step(&mut world, G, DT).is_empty());
    }

    #[test]
    fn a_body_with_no_collider_moves_and_never_collides() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(entity, RigidBody::new_dynamic(1.0));

        let contacts = step(&mut world, G, DT);

        assert!(contacts.is_empty());
        assert!(world.get::<Transform>(entity).expect("t").translation.y < 0.0);
    }

    #[test]
    fn a_collider_with_no_body_is_static_geometry() {
        let mut world = World::new();
        let wall = world.spawn();
        world.insert(wall, Transform::default());
        world.insert(
            wall,
            Collider::Aabb {
                half_extents: Vec2::splat(1.0),
            },
        );

        let ball = world.spawn();
        world.insert(ball, Transform::from_translation(Vec2::new(0.5, 0.0)));
        world.insert(ball, Collider::Circle { radius: 0.5 });

        assert_eq!(step(&mut world, G, DT).len(), 1);
        assert_eq!(
            world.get::<Transform>(wall).expect("t").translation,
            Vec2::ZERO,
            "a collider with no body must not move"
        );
    }

    #[test]
    fn the_contact_names_the_entities_it_is_between() {
        let mut world = World::new();
        let a = world.spawn();
        world.insert(a, Transform::default());
        world.insert(a, Collider::Circle { radius: 1.0 });
        let b = world.spawn();
        world.insert(b, Transform::from_translation(Vec2::new(1.0, 0.0)));
        world.insert(b, Collider::Circle { radius: 1.0 });

        let contacts = step(&mut world, G, DT);

        assert_eq!(contacts.len(), 1);
        assert_eq!((contacts[0].a, contacts[0].b), (a, b));
    }
}
```

- [ ] **Step 2: Write the step**

```rust
//! One fixed step: move everything, then find what overlaps.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Transform};

use crate::broad::candidate_pairs;
use crate::integrate::integrate;
use crate::narrow::{contact, Contact};

/// Advances the world by `dt` and returns what is overlapping afterwards.
///
/// Contacts are returned rather than stored. Nothing consumes them yet but the
/// debug draw, and a `Contacts` resource with no reader would be a structure
/// designed for an imagined caller. 11b-2's solver takes this list as its
/// input, which is the reason detection is worth shipping without it: a wrong
/// normal is a line pointing the wrong way on screen, not a number nobody sees.
pub fn step(world: &mut World, gravity: Vec2, dt: f32) -> Vec<Contact> {
    integrate(world, gravity, dt);

    candidate_pairs(world)
        .into_iter()
        .filter_map(|(a, b)| {
            let a_shape = (world.get::<Collider>(a)?, world.get::<Transform>(a)?);
            let b_shape = (world.get::<Collider>(b)?, world.get::<Transform>(b)?);
            let (normal, penetration, point) = contact(a_shape, b_shape)?;
            Some(Contact {
                a,
                b,
                normal,
                penetration,
                point,
            })
        })
        .collect()
}
```

- [ ] **Step 3: Restore `lib.rs` and run**

`lib.rs` is as written in Task 2 Step 1 once every module exists.

Run: `cargo test -p voltra-physics`
Expected: PASS, everything from Tasks 2, 3 and 4.

- [ ] **Step 4: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 5: Commit**

```sh
git add crates/voltra-physics/src/step.rs crates/voltra-physics/src/lib.rs
git commit -m "feat(physics): step the world and report contacts"
```

---

### Task 5: On screen, and in the loop

**Files:**
- Create: `crates/voltra-physics/src/debug.rs`
- Modify: `crates/voltra-core/src/app.rs`, `crates/voltra-core/Cargo.toml`
- Modify: `crates/voltra-editor/src/editor.rs`, `src/panels/menu_bar.rs`,
  `src/panels/viewport.rs`, `crates/voltra-editor/Cargo.toml`

**Interfaces:**
- Produces:
  - `debug::draw(world: &World, contacts: &[Contact], lines: &mut LineBatch)`
  - `App::{with_physics(self) -> Self, gravity: Vec2}`
  - `UiFrame::contacts(&self) -> &[Contact]`

- [ ] **Step 1: The debug draw**

`crates/voltra-physics/src/debug.rs`. An AABB is four segments; a circle is a
24-segment closed polyline; a contact is a segment from its point along its
normal, long enough to see. Shapes in green, contacts in magenta — the colour
that already means "look here" in this engine.

Unit-test what is testable without a GPU: that a world with one box produces
four segments, a circle produces 24, an empty world produces none, and a contact
adds exactly one more. `LineBatch::len` gives the count.

- [ ] **Step 2: Drive it from `App`**

`App` gains `physics: bool`, `clock: PhysicsClock`, `gravity: Vec2` and
`contacts: Vec<Contact>`, plus `with_physics()` beside `with_hot_reload()`. Off
by default, like every other opt-in on `App`.

In `update`, after the clock tick:

```rust
        if self.physics {
            let dt = self.physics_clock.step();
            for _ in 0..self.physics_clock.steps(self.clock.delta().as_secs_f32()) {
                self.contacts = voltra_physics::step(&mut self.world, self.gravity, dt);
            }
        }
```

Read `Clock`'s accessor name before writing this; `delta()` is a guess.

The contacts kept are the last step's, which is what the debug draw should show:
the state the frame ends in.

- [ ] **Step 3: The editor toggle**

`Editor` gains `show_colliders: bool`, off by default — a scene full of green
outlines is not what anyone wants while placing sprites. A `Physics` menu in the
menu bar toggles it. The viewport panel, after the gizmo's `draw`, calls
`voltra_physics::debug::draw` when it is on.

`UiFrame` needs to expose the contacts for that; add `contacts: &'a [Contact]`
beside `lines`, filled from `App::contacts`.

- [ ] **Step 4: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 5: Drive the editor**

Launch it detached, never in the foreground. Spawn or open a scene with a
dynamic body over a static floor, turn the toggle on, and confirm: green
outlines follow their sprites, the body falls, magenta appears when it reaches
the floor, and it keeps going through it — because nothing resolves contacts
yet, which is this stage's stated limit and not a bug. Kill it and report which
of those held.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-physics/src/debug.rs crates/voltra-core crates/voltra-editor
git commit -m "feat(core): step physics and draw its shapes"
```

---

### Task 6: Record the decisions

- [ ] **Step 1: `docs/ARCHITECTURE.md`**

Under "Decisions": physics is in-house because adopting Rapier with a
hand-written ECS reproduces `bevy_rapier`'s parallel world, which `avian` was
written to avoid. The components live in `voltra-scene` because
`ComponentRegistry` does and the alternative is a cycle. Semi-implicit Euler,
inverse mass, the fixed step and its cap. The broad phase is O(n²) with a
written replacement threshold of ~200 bodies. And that 11b-2's solver is already
chosen — TGS Soft, from Erin Catto's eight-way comparison — so it is not
re-argued. Each with its rejected alternatives.

Add `voltra-physics` to the crate table.

- [ ] **Step 2: `README.md`**

```markdown
| 11b-1 | Rigid bodies, fixed-step integration, contact detection | done |
| 11b-2 | The contact solver: bodies stop overlapping and stack | planned |
```

- [ ] **Step 3: Tick this plan, verify, commit**

Check each box against `git log --oneline main..HEAD`; leave unticked anything
not in the tree and say so. Then `cargo fmt --all --check`, clippy, the full
test run, and commit as `docs: record the physics decisions`.

---

## Definition of done

- A dynamic body falls on a fixed step and its motion does not change with the
  frame rate.
- Overlapping bodies produce exactly one contact each, with a normal that pushes
  `a` away from `b` and a positive penetration.
- Concentric circles, a circle at a box's centre, zero-sized colliders, negative
  scale, negative mass and enormous damping all produce finite, sensible results
  — each has a test.
- The colliders and contacts can be seen in the editor, behind a toggle that is
  off by default.
- `voltra-scene` does not depend on `voltra-physics`.
- A scene round-trips `RigidBody` and `Collider` without any caller opting in.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` are clean.

## Spec coverage

| Spec section | Task |
| --- | --- |
| In-house rather than Rapier | the whole plan |
| Components in `voltra-scene`, and why | Task 1 |
| `BodyType`, inverse mass | Task 1 |
| Fixed timestep and its cap | Task 2 |
| Semi-implicit integration | Task 2 |
| O(n²) broad phase with a stated limit | Task 3 |
| Three narrow-phase pairs and their degenerate cases | Task 3 |
| `Contact`, returned not stored | Task 4 |
| Debug draw over the line pipeline | Task 5 |
| Edge cases: no collider, no body, zero extent, huge damping | Tasks 1–4 |
| Decisions recorded | Task 6 |
