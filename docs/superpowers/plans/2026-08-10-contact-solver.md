# Contact Solver Implementation Plan (stage 11b-2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the contacts `voltra-physics` already detects, so a body rests
on a floor and a stack of boxes settles, using TGS Soft.

**Architecture:** `PhysicsWorld` owns the fixed clock, the solver parameters and
a per-pair impulse cache. One fixed step collides once, gathers a dense array of
solver bodies from the ECS, prepares one constraint per contact, warm-starts it
from the cache, runs `sub_steps` of (integrate velocities → biased solve →
integrate positions), then one relax pass carrying friction, then restitution,
then scatters velocities and positions back to the ECS.

**Tech Stack:** Rust, `glam` (via `voltra-render`), `voltra-ecs`, `serde`/`ron`
for the new component. No new dependencies.

## Global Constraints

- 2D only. No angular velocity, no inertia, no torque arms, no z anything.
- Only `voltra-core` depends on `winit`; only `voltra-render` depends on `wgpu`.
  `voltra-physics` reaches `glam` through `voltra_render::glam`, as it does now.
- No `unwrap()` outside tests. `expect("why this cannot fail")` when the
  invariant is real. Log via `log`, never `println!`.
- One concept per file; split a module before it passes ~300 lines.
- All versions live in the root `[workspace.dependencies]`.
- Every task ends green on
  `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace`.
- Sign convention, unchanged from 11b-1: **`Contact::normal` pushes `a` away
  from `b`**, and the broad phase emits `(a, b)` once with `a` the lower entity.
  A positive normal impulse therefore moves `a` along `+normal` and `b` along
  `-normal`, and `vn = dot(v_a - v_b, normal)` is negative while they approach.

---

### Task 1: The `PhysicsMaterial` component

**Files:**
- Create: `crates/voltra-scene/src/material.rs`
- Modify: `crates/voltra-scene/src/lib.rs` (module + re-export)
- Modify: `crates/voltra-scene/src/format/registry.rs` (`with_defaults`)

**Interfaces:**
- Produces: `voltra_scene::PhysicsMaterial { friction: f32, restitution: f32 }`,
  `PhysicsMaterial::default()` = `{ 0.6, 0.0 }`,
  `PhysicsMaterial::combine(a: Option<&Self>, b: Option<&Self>) -> (f32, f32)`
  returning `(friction, restitution)` already mixed and clamped.

- [ ] **Step 1: Write the failing tests** in `material.rs`

```rust
#[test]
fn a_missing_material_is_the_default_surface() {
    let (friction, restitution) = PhysicsMaterial::combine(None, None);
    assert!((friction - 0.6).abs() < 1e-6);
    assert_eq!(restitution, 0.0);
}

#[test]
fn friction_is_the_geometric_mean_and_restitution_the_maximum() {
    let ice = PhysicsMaterial { friction: 0.04, restitution: 0.0 };
    let rubber = PhysicsMaterial { friction: 0.9, restitution: 0.8 };
    let (friction, restitution) = PhysicsMaterial::combine(Some(&ice), Some(&rubber));
    assert!((friction - (0.04f32 * 0.9).sqrt()).abs() < 1e-6);
    assert!((restitution - 0.8).abs() < 1e-6);
}

#[test]
fn a_scene_cannot_ask_for_negative_friction_or_a_restitution_above_one() {
    // External input: negative friction reverses the friction impulse, and a
    // restitution above one adds energy to every bounce until the body leaves
    // the world.
    let bad = PhysicsMaterial { friction: -5.0, restitution: 9.0 };
    let (friction, restitution) = PhysicsMaterial::combine(Some(&bad), Some(&bad));
    assert_eq!(friction, 0.0);
    assert_eq!(restitution, 1.0);
}

#[test]
fn a_material_round_trips_through_ron() {
    let material = PhysicsMaterial { friction: 0.25, restitution: 0.5 };
    let text = ron::to_string(&material).expect("serialise");
    assert_eq!(ron::from_str::<PhysicsMaterial>(&text).expect("deserialise"), material);
}
```

- [ ] **Step 2: Run and watch it fail**

`cargo test -p voltra-scene material` → fails to compile, `PhysicsMaterial` not found.

- [ ] **Step 3: Implement**

```rust
//! How a surface rubs and how it bounces.

use serde::{Deserialize, Serialize};

/// The surface properties of a collider.
///
/// A component of its own rather than fields on `Collider` or `RigidBody`:
/// `Collider` is an enum, so fields there are duplicated in every variant and
/// in every shape added later, and `RigidBody` would leave static geometry — a
/// collider with no body — with no surface at all. Unity's `PhysicsMaterial2D`
/// is the same separation, adapted to composition.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsMaterial {
    /// Resistance to sliding. Box2D's default of `0.6` is a wooden crate.
    pub friction: f32,
    /// Fraction of approach speed returned on impact. `0.0` does not bounce.
    pub restitution: f32,
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self { friction: 0.6, restitution: 0.0 }
    }
}

impl PhysicsMaterial {
    /// The surface of a contact between two colliders, mixed and clamped.
    ///
    /// `sqrt(fa · fb)` and `max(ra, rb)`, which is what Box2D mixes: the
    /// geometric mean means one slippery surface makes the pair slippery, and
    /// the maximum means one bouncy surface makes the pair bounce.
    ///
    /// A missing component is the default surface, so an entity nobody has
    /// given a material still rubs.
    pub fn combine(a: Option<&Self>, b: Option<&Self>) -> (f32, f32) {
        let default = Self::default();
        let a = a.unwrap_or(&default);
        let b = b.unwrap_or(&default);
        let friction = (a.friction.max(0.0) * b.friction.max(0.0)).sqrt();
        let restitution = a.restitution.max(b.restitution).clamp(0.0, 1.0);
        (friction, restitution)
    }
}
```

`lib.rs`: `pub mod material;` and `pub use material::PhysicsMaterial;`.
`registry.rs`: add `PhysicsMaterial` to the import and
`registry.register::<PhysicsMaterial>("PhysicsMaterial");` in `with_defaults`,
next to `RigidBody`.

- [ ] **Step 4: Run the tests**

`cargo test -p voltra-scene` → all pass, including the registry's round-trip.

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-scene
git commit -m "feat(scene): add a physics material component"
```

---

### Task 2: Softness and solver parameters

**Files:**
- Create: `crates/voltra-physics/src/solver.rs` (module root, doc + re-exports)
- Create: `crates/voltra-physics/src/solver/softness.rs`
- Create: `crates/voltra-physics/src/solver/params.rs`
- Modify: `crates/voltra-physics/src/lib.rs`

**Interfaces:**
- Produces: `Softness { bias_rate, mass_scale, impulse_scale }`,
  `Softness::RIGID`, `Softness::new(hertz: f32, damping_ratio: f32, h: f32)`;
  `SolverParams { sub_steps: u32, contact_hertz: f32, contact_damping_ratio: f32,
  max_push_speed: f32, restitution_threshold: f32, warm_starting: bool }`,
  `SolverParams::sub_step(&self, dt: f32) -> f32`,
  `SolverParams::softness(&self, h: f32) -> (Softness, Softness)` returning
  `(dynamic, against_immovable)`.

- [ ] **Step 1: Write the failing tests**

```rust
// softness.rs
#[test]
fn a_stiffer_spring_biases_harder() {
    let h = 1.0 / 240.0;
    let soft = Softness::new(30.0, 10.0, h);
    let stiff = Softness::new(60.0, 10.0, h);
    assert!(stiff.bias_rate > soft.bias_rate);
    assert!(soft.bias_rate.is_finite() && soft.mass_scale.is_finite());
}

#[test]
fn the_scales_stay_inside_the_unit_interval() {
    // mass_scale = a2·a3 and impulse_scale = a3 with a3 = 1/(1+a2), a2 >= 0.
    let s = Softness::new(30.0, 10.0, 1.0 / 240.0);
    assert!((0.0..=1.0).contains(&s.mass_scale), "{s:?}");
    assert!((0.0..=1.0).contains(&s.impulse_scale), "{s:?}");
}

#[test]
fn a_non_positive_hertz_or_step_is_a_rigid_constraint() {
    // Not Box2D's all-zero softness: a zero mass_scale would delete the
    // constraint rather than harden it, and every caller here wants the hard
    // constraint when softening is switched off.
    assert_eq!(Softness::new(0.0, 10.0, 1.0 / 240.0), Softness::RIGID);
    assert_eq!(Softness::new(30.0, 10.0, 0.0), Softness::RIGID);
}

// params.rs
#[test]
fn contacts_against_immovable_bodies_are_stiffer() {
    let params = SolverParams::default();
    let (dynamic, immovable) = params.softness(params.sub_step(1.0 / 60.0));
    assert!(immovable.bias_rate > dynamic.bias_rate);
}

#[test]
fn the_hertz_is_capped_well_under_the_sub_step_rate() {
    // Nyquist: a spring faster than the sample rate is noise. Box2D caps at
    // 0.125/h, which is 30 Hz at the default four sub-steps of a 60 Hz step
    // and lower when the step is long.
    let params = SolverParams::default();
    let h = 1.0 / 10.0;
    let (dynamic, _) = params.softness(h);
    let capped = Softness::new(0.125 / h, params.contact_damping_ratio, h);
    assert!((dynamic.bias_rate - capped.bias_rate).abs() < 1e-6);
}

#[test]
fn zero_sub_steps_still_produces_a_usable_step() {
    let params = SolverParams { sub_steps: 0, ..Default::default() };
    assert!(params.sub_step(1.0 / 60.0) > 0.0);
}
```

- [ ] **Step 2: Run and watch it fail**

- [ ] **Step 3: Implement**

```rust
// softness.rs
use std::f32::consts::PI;

/// A constraint's spring, as three precomputed coefficients.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Softness {
    pub bias_rate: f32,
    pub mass_scale: f32,
    pub impulse_scale: f32,
}

impl Softness {
    /// No softening: the constraint is solved hard.
    pub const RIGID: Self = Self { bias_rate: 0.0, mass_scale: 1.0, impulse_scale: 0.0 };

    /// The coefficients of a mass-spring-damper at `hertz` and `damping_ratio`,
    /// solved at a step of `h`. `b2MakeSoft`, verbatim.
    pub fn new(hertz: f32, damping_ratio: f32, h: f32) -> Self {
        if hertz <= 0.0 || h <= 0.0 {
            return Self::RIGID;
        }
        let omega = 2.0 * PI * hertz;
        let a1 = 2.0 * damping_ratio.max(0.0) + h * omega;
        let a2 = h * omega * a1;
        let a3 = 1.0 / (1.0 + a2);
        Self { bias_rate: omega / a1, mass_scale: a2 * a3, impulse_scale: a3 }
    }
}
```

```rust
// params.rs — SolverParams with the Default impl holding 4, 30.0, 10.0, 3.0,
// 1.0, true, and:
    pub fn sub_step(&self, dt: f32) -> f32 {
        dt / self.sub_steps.max(1) as f32
    }

    pub fn softness(&self, h: f32) -> (Softness, Softness) {
        // Nyquist: the spring must stay well under the sub-step rate, so a long
        // step lowers the frequency rather than ringing.
        let hertz = if h > 0.0 { self.contact_hertz.min(0.125 / h) } else { 0.0 };
        (
            Softness::new(hertz, self.contact_damping_ratio, h),
            Softness::new(2.0 * hertz, self.contact_damping_ratio, h),
        )
    }
```

- [ ] **Step 4: Run the tests** — `cargo test -p voltra-physics`

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-physics
git commit -m "feat(physics): add soft constraint coefficients"
```

---

### Task 3: Solver bodies, and integration split in two

**Files:**
- Create: `crates/voltra-physics/src/solver/body.rs`
- Modify: `crates/voltra-physics/src/integrate.rs` (rewrite over solver bodies)
- Modify: `crates/voltra-physics/src/lib.rs`

**Interfaces:**
- Consumes: nothing from tasks 1–2.
- Produces: `SolverBody { entity, velocity, delta_position, inverse_mass,
  body_type, gravity_scale, linear_damping }`; `SolverBodies::gather(&World)`,
  `SolverBodies::index_of(Entity) -> Option<usize>`,
  `SolverBodies::pair_mut(usize, usize) -> (&mut SolverBody, &mut SolverBody)`,
  `SolverBodies::get(usize) -> &SolverBody`, `SolverBodies::scatter(self, &mut World)`;
  `integrate_velocities(&mut SolverBodies, gravity: Vec2, h: f32)` and
  `integrate_positions(&mut SolverBodies, h: f32)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_collider_without_a_body_is_gathered_as_immovable() {
    // Static geometry already exists in the tests: a collider and a transform
    // and nothing else. It has to reach the solver, or contacts against the
    // world would have no second body.
    let mut world = World::new();
    let wall = world.spawn();
    world.insert(wall, Transform::default());
    world.insert(wall, Collider::Circle { radius: 1.0 });

    let bodies = SolverBodies::gather(&world);
    let index = bodies.index_of(wall).expect("gathered");

    assert_eq!(bodies.get(index).inverse_mass, 0.0);
    assert_eq!(bodies.get(index).velocity, Vec2::ZERO);
}

#[test]
fn a_kinematic_body_moves_itself_but_cannot_be_pushed() {
    let mut world = World::new();
    let platform = world.spawn();
    world.insert(platform, Transform::default());
    world.insert(platform, RigidBody {
        body_type: BodyType::Kinematic,
        velocity: Vec2::new(2.0, 0.0),
        ..Default::default()
    });

    let mut bodies = SolverBodies::gather(&world);
    let index = bodies.index_of(platform).expect("gathered");
    assert_eq!(bodies.get(index).inverse_mass, 0.0, "immovable by contacts");

    integrate_velocities(&mut bodies, Vec2::new(0.0, -10.0), 1.0 / 60.0);
    integrate_positions(&mut bodies, 1.0 / 60.0);

    assert_eq!(bodies.get(index).velocity, Vec2::new(2.0, 0.0), "gravity must not touch it");
    assert!(bodies.get(index).delta_position.x > 0.0, "it still moves itself");
}

#[test]
fn velocity_is_integrated_before_position() {
    // Semi-implicit Euler, unchanged from 11b-1: after one step from rest the
    // body has moved by exactly g·h², and explicit Euler would leave it at zero.
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    world.insert(entity, RigidBody::new_dynamic(1.0));

    let mut bodies = SolverBodies::gather(&world);
    let h = 1.0 / 60.0;
    integrate_velocities(&mut bodies, Vec2::new(0.0, -10.0), h);
    integrate_positions(&mut bodies, h);

    let index = bodies.index_of(entity).expect("gathered");
    let expected = -10.0 * h * h;
    assert!((bodies.get(index).delta_position.y - expected).abs() < 1e-9);
}

#[test]
fn enormous_damping_stops_a_body_rather_than_reversing_it() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    world.insert(entity, RigidBody {
        velocity: Vec2::new(10.0, 0.0),
        gravity_scale: 0.0,
        linear_damping: 10_000.0,
        ..RigidBody::new_dynamic(1.0)
    });

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, Vec2::ZERO, 1.0 / 60.0);

    let index = bodies.index_of(entity).expect("gathered");
    assert!(bodies.get(index).velocity.x >= 0.0);
}

#[test]
fn scatter_writes_velocity_and_the_accumulated_move_back() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    world.insert(entity, RigidBody::new_dynamic(1.0));

    let mut bodies = SolverBodies::gather(&world);
    let h = 1.0 / 60.0;
    integrate_velocities(&mut bodies, Vec2::new(0.0, -10.0), h);
    integrate_positions(&mut bodies, h);
    bodies.scatter(&mut world);

    assert!(world.get::<Transform>(entity).expect("transform").translation.y < 0.0);
    assert!(world.get::<RigidBody>(entity).expect("body").velocity.y < 0.0);
}

#[test]
fn a_static_body_is_never_moved_by_integration() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    let mut body = RigidBody::new_static();
    body.velocity = Vec2::new(100.0, 100.0);
    world.insert(entity, body);

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, Vec2::new(0.0, -10.0), 1.0 / 60.0);
    integrate_positions(&mut bodies, 1.0 / 60.0);
    bodies.scatter(&mut world);

    assert_eq!(world.get::<Transform>(entity).expect("transform").translation, Vec2::ZERO);
}

#[test]
fn a_body_without_a_transform_simulates_and_moves_nothing() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, RigidBody::new_dynamic(1.0));

    let mut bodies = SolverBodies::gather(&world);
    integrate_velocities(&mut bodies, Vec2::new(0.0, -10.0), 1.0 / 60.0);
    bodies.scatter(&mut world);

    assert!(world.get::<Transform>(entity).is_none());
    assert!(world.get::<RigidBody>(entity).expect("body").velocity.y < 0.0);
}
```

- [ ] **Step 2: Run and watch it fail**

- [ ] **Step 3: Implement `body.rs`**

Gather walks `world.query::<RigidBody>()` first, then `world.query::<Collider>()`
adding only entities not already present, so every collider has a solver body and
the order is the ECS's dense order rather than a hash order. `pair_mut` uses
`split_at_mut` on the sorted index pair. `scatter` writes `velocity` back to any
non-static `RigidBody` and adds `delta_position` to the `Transform` when there is
one. Static bodies are skipped on both counts.

`integrate.rs` keeps its doc comment about symplectic Euler and becomes:

```rust
pub fn integrate_velocities(bodies: &mut SolverBodies, gravity: Vec2, h: f32) {
    for body in bodies.iter_mut() {
        if body.body_type != BodyType::Dynamic {
            continue;
        }
        let accelerated = body.velocity + gravity * body.gravity_scale * h;
        // Clamped at zero: `1 − damping·h` turns a brake into a catapult once
        // the damping is large enough, and a scene file can say 10 000.
        body.velocity = accelerated * (1.0 - body.linear_damping * h).max(0.0);
    }
}

pub fn integrate_positions(bodies: &mut SolverBodies, h: f32) {
    for body in bodies.iter_mut() {
        if body.body_type == BodyType::Static {
            continue;
        }
        body.delta_position += body.velocity * h;
    }
}
```

- [ ] **Step 4: Run the tests**

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-physics
git commit -m "feat(physics): gather bodies for the solver"
```

---

### Task 4: The impulse cache

**Files:**
- Create: `crates/voltra-physics/src/solver/cache.rs`
- Modify: `crates/voltra-physics/src/solver.rs`

**Interfaces:**
- Produces: `CachedImpulse { normal: f32, tangent: f32 }` (`Default` = zeroes);
  `ImpulseCache::warm_start(&self, key: (Entity, Entity)) -> CachedImpulse`,
  `ImpulseCache::record(&mut self, key, CachedImpulse)`,
  `ImpulseCache::commit(&mut self)`, `ImpulseCache::len(&self) -> usize`,
  `ImpulseCache::is_empty(&self) -> bool`, `ImpulseCache::clear(&mut self)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_unknown_pair_starts_from_zero() {
    let (a, b) = two_entities();
    assert_eq!(ImpulseCache::default().warm_start((a, b)), CachedImpulse::default());
}

#[test]
fn a_recorded_pair_is_returned_after_the_step_is_committed() {
    let (a, b) = two_entities();
    let mut cache = ImpulseCache::default();
    cache.record((a, b), CachedImpulse { normal: 3.0, tangent: -1.0 });
    assert_eq!(cache.warm_start((a, b)), CachedImpulse::default(), "not until committed");
    cache.commit();
    assert_eq!(cache.warm_start((a, b)), CachedImpulse { normal: 3.0, tangent: -1.0 });
}

#[test]
fn a_pair_that_stops_touching_is_forgotten() {
    // The eviction rule every engine uses: the entry dies when the pair stops
    // being reported. Without it the map grows for the lifetime of the process
    // and a despawned body keeps its impulses forever.
    let (a, b) = two_entities();
    let mut cache = ImpulseCache::default();
    cache.record((a, b), CachedImpulse { normal: 3.0, tangent: 0.0 });
    cache.commit();
    cache.commit(); // a step in which nothing was recorded
    assert!(cache.is_empty());
    assert_eq!(cache.warm_start((a, b)), CachedImpulse::default());
}
```

- [ ] **Step 2: Run and watch it fail**

- [ ] **Step 3: Implement** — two `HashMap`s, `current` read by `warm_start` and
`next` written by `record`; `commit` swaps them and clears the new `next`, which
evicts everything untouched without a second pass and without reallocating.

- [ ] **Step 4: Run the tests**

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-physics
git commit -m "feat(physics): keep contact impulses between steps"
```

---

### Task 5: Constraints, and contacts that actually stop a body

**Files:**
- Create: `crates/voltra-physics/src/solver/constraint.rs`
- Create: `crates/voltra-physics/src/solver/contact.rs`
- Modify: `crates/voltra-physics/src/step.rs` (rewritten around the solver)
- Modify: `crates/voltra-physics/src/lib.rs`

**Interfaces:**
- Consumes: `Softness`, `SolverParams`, `SolverBodies`, `integrate_velocities`,
  `integrate_positions`, `ImpulseCache`, `PhysicsMaterial::combine`.
- Produces: `ContactConstraint { a, b, key, normal, base_separation, normal_mass,
  tangent_mass, friction, restitution, relative_velocity, normal_impulse,
  tangent_impulse, total_normal_impulse, softness }`;
  `prepare(&[Contact], &SolverBodies, &World, (Softness, Softness), &ImpulseCache, bool) -> Vec<ContactConstraint>`;
  `warm_start(&[ContactConstraint], &mut SolverBodies)`;
  `solve(&mut [ContactConstraint], &mut SolverBodies, h: f32, use_bias: bool, max_push_speed: f32)`;
  `apply_restitution(&mut [ContactConstraint], &mut SolverBodies, threshold: f32)`;
  `step(&mut World, &mut ImpulseCache, &SolverParams, gravity: Vec2, dt: f32) -> Vec<Contact>`.

- [ ] **Step 1: Write the failing tests** in `step.rs`

```rust
#[test]
fn a_falling_body_comes_to_rest_on_the_floor() {
    // Replaces `a_falling_body_keeps_going_through_the_floor`, which pinned
    // 11b-1's stated limit. Same scene, opposite assertion.
    let (mut world, ball) = floor_and_ball();
    let mut cache = ImpulseCache::default();
    let params = SolverParams::default();

    for _ in 0..300 {
        step(&mut world, &mut cache, &params, G, DT);
    }

    let y = translation(&world, ball).y;
    // Floor top at -1.5, ball radius 0.5, so resting centre is -1.0.
    assert!((y + 1.0).abs() < 0.05, "should rest on the floor, got {y}");
}

#[test]
fn a_body_at_rest_stays_at_rest() {
    let (mut world, ball) = floor_and_ball();
    let mut cache = ImpulseCache::default();
    let params = SolverParams::default();
    for _ in 0..300 {
        step(&mut world, &mut cache, &params, G, DT);
    }
    let settled = translation(&world, ball).y;
    for _ in 0..300 {
        step(&mut world, &mut cache, &params, G, DT);
    }
    assert!((translation(&world, ball).y - settled).abs() < 0.01, "it must not creep");
}

#[test]
fn a_stack_of_boxes_settles_without_sinking() {
    let mut world = World::new();
    let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
    let mut boxes = Vec::new();
    for i in 0..5 {
        boxes.push(spawn_box(
            &mut world,
            Vec2::new(0.0, 0.55 + i as f32 * 1.05),
            Vec2::splat(0.5),
            Some(1.0),
        ));
    }
    let _ = floor;

    let mut cache = ImpulseCache::default();
    let params = SolverParams::default();
    for _ in 0..600 {
        step(&mut world, &mut cache, &params, G, DT);
    }

    let mut previous = 0.0; // floor top
    for (i, entity) in boxes.iter().enumerate() {
        let y = translation(&world, *entity).y;
        assert!(y > previous, "box {i} sank into the one below: {y} <= {previous}");
        previous = y;
    }
    assert!(translation(&world, boxes[4]).y < 5.2, "the stack must not climb");
}

#[test]
fn a_static_body_is_not_pushed_by_what_lands_on_it() {
    let (mut world, _) = floor_and_ball();
    let floor = first_entity(&world);
    let mut cache = ImpulseCache::default();
    for _ in 0..120 {
        step(&mut world, &mut cache, &SolverParams::default(), G, DT);
    }
    assert_eq!(translation(&world, floor), Vec2::new(0.0, -2.0));
}

#[test]
fn a_thousand_to_one_mass_ratio_does_not_explode() {
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(10.0, 1.0), None);
    let light = spawn_box(&mut world, Vec2::new(0.0, 0.5), Vec2::splat(0.5), Some(0.001));
    let heavy = spawn_box(&mut world, Vec2::new(0.0, 1.6), Vec2::splat(0.5), Some(1.0));

    let mut cache = ImpulseCache::default();
    for _ in 0..600 {
        step(&mut world, &mut cache, &SolverParams::default(), G, DT);
    }

    assert!(translation(&world, light).is_finite());
    assert!(translation(&world, heavy).y > translation(&world, light).y);
    assert!(translation(&world, heavy).y < 10.0, "it must not be launched");
}

#[test]
fn the_cache_forgets_a_body_that_was_despawned() {
    let (mut world, ball) = floor_and_ball();
    let mut cache = ImpulseCache::default();
    for _ in 0..200 {
        step(&mut world, &mut cache, &SolverParams::default(), G, DT);
    }
    assert!(!cache.is_empty(), "resting bodies should be warm started");

    world.despawn(ball);
    step(&mut world, &mut cache, &SolverParams::default(), G, DT);

    assert!(cache.is_empty(), "the pair is gone, so the impulses must be too");
}

#[test]
fn coincident_bodies_separate_without_a_nan() {
    let mut world = World::new();
    let a = spawn_box(&mut world, Vec2::ZERO, Vec2::splat(0.5), Some(1.0));
    let b = spawn_box(&mut world, Vec2::ZERO, Vec2::splat(0.5), Some(1.0));

    let mut cache = ImpulseCache::default();
    for _ in 0..120 {
        step(&mut world, &mut cache, &SolverParams::default(), Vec2::ZERO, DT);
    }

    assert!(translation(&world, a).is_finite() && translation(&world, b).is_finite());
    assert!(translation(&world, a).distance(translation(&world, b)) > 0.5, "they must separate");
}

#[test]
fn an_empty_world_steps_without_contacts() {
    let mut world = World::new();
    let mut cache = ImpulseCache::default();
    assert!(step(&mut world, &mut cache, &SolverParams::default(), G, DT).is_empty());
}

#[test]
fn a_zero_length_step_changes_nothing() {
    let (mut world, ball) = floor_and_ball();
    let before = translation(&world, ball);
    let mut cache = ImpulseCache::default();
    step(&mut world, &mut cache, &SolverParams::default(), G, 0.0);
    assert_eq!(translation(&world, ball), before);
}
```

Helpers in the same test module: `floor_and_ball()` spawning a static AABB floor
of half-extents `(10, 0.5)` at `(0, -2)` and a dynamic circle of radius `0.5` at
the origin; `spawn_box(world, at, half_extents, mass)` where `None` means static
geometry with no `RigidBody`; `translation`, `first_entity`.

- [ ] **Step 2: Run and watch it fail**

- [ ] **Step 3: Implement**

`constraint.rs` holds the struct and `prepare`, which for each contact looks up
both solver-body indices, computes
`normal_mass = 1 / (inv_a + inv_b)` (zero when the sum is zero — two immovable
bodies, whose constraint does nothing and must not divide by zero), the same for
the tangent, `base_separation = -penetration`,
`relative_velocity = dot(v_a - v_b, normal)`, the friction and restitution from
`PhysicsMaterial::combine`, the softness (`immovable` when either body's inverse
mass is zero), and the warm-start impulses from the cache scaled by
`params.warm_starting`.

`contact.rs` holds `warm_start`, `solve` and `apply_restitution`, exactly as the
spec's pseudocode. `solve` applies friction only when `use_bias` is false, which
is what the Box2D source does — friction applied during the biased pass would be
scaled by the separation push.

`step.rs` becomes the sequence in the spec: collide, gather, prepare, warm start,
`sub_steps` × (integrate velocities, solve with bias, integrate positions), relax
solve without bias, restitution, record every constraint's impulses, commit the
cache, scatter. `dt <= 0.0` returns early with no contacts.

- [ ] **Step 4: Run the tests** — `cargo test -p voltra-physics`

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-physics
git commit -m "feat(physics): resolve contacts with a soft TGS solver"
```

---

### Task 6: Friction and restitution behaviour

**Files:**
- Modify: `crates/voltra-physics/src/step.rs` (tests only, if tasks 5 landed
  friction and restitution together — otherwise the passes as well)

**Interfaces:** unchanged.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn friction_stops_a_sliding_body() {
    let mut world = World::new();
    spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
    let crate_ = spawn_box(&mut world, Vec2::new(0.0, 0.5), Vec2::splat(0.5), Some(1.0));
    world.get_mut::<RigidBody>(crate_).expect("body").velocity = Vec2::new(10.0, 0.0);

    let mut cache = ImpulseCache::default();
    for _ in 0..600 {
        step(&mut world, &mut cache, &SolverParams::default(), G, DT);
    }

    let speed = world.get::<RigidBody>(crate_).expect("body").velocity.x;
    assert!(speed.abs() < 0.5, "friction should have stopped it, got {speed}");
}

#[test]
fn a_frictionless_body_keeps_sliding() {
    let mut world = World::new();
    let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
    world.insert(floor, PhysicsMaterial { friction: 0.0, restitution: 0.0 });
    let puck = spawn_box(&mut world, Vec2::new(0.0, 0.5), Vec2::splat(0.5), Some(1.0));
    world.insert(puck, PhysicsMaterial { friction: 0.0, restitution: 0.0 });
    world.get_mut::<RigidBody>(puck).expect("body").velocity = Vec2::new(10.0, 0.0);

    let mut cache = ImpulseCache::default();
    for _ in 0..300 {
        step(&mut world, &mut cache, &SolverParams::default(), G, DT);
    }

    assert!(world.get::<RigidBody>(puck).expect("body").velocity.x > 9.0);
}

#[test]
fn a_bouncy_body_returns_most_of_its_drop() {
    let mut world = World::new();
    let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
    world.insert(floor, PhysicsMaterial { friction: 0.6, restitution: 0.9 });
    let ball = spawn_box(&mut world, Vec2::new(0.0, 5.0), Vec2::splat(0.5), Some(1.0));
    world.insert(ball, PhysicsMaterial { friction: 0.6, restitution: 0.9 });

    let mut cache = ImpulseCache::default();
    let mut highest_after_impact: f32 = -10.0;
    let mut bounced = false;
    for _ in 0..600 {
        step(&mut world, &mut cache, &SolverParams::default(), G, DT);
        let body = *world.get::<RigidBody>(ball).expect("body");
        if body.velocity.y > 1.0 {
            bounced = true;
        }
        if bounced {
            highest_after_impact = highest_after_impact.max(translation(&world, ball).y);
        }
    }

    assert!(bounced, "restitution 0.9 must send it back up");
    assert!(highest_after_impact > 2.0, "it should return most of the drop, got {highest_after_impact}");
}

#[test]
fn a_dead_body_does_not_bounce_on_its_own_noise() {
    // Restitution below the threshold speed is discarded, which is what stops a
    // resting body from vibrating forever.
    let mut world = World::new();
    let floor = spawn_box(&mut world, Vec2::new(0.0, -1.0), Vec2::new(20.0, 1.0), None);
    world.insert(floor, PhysicsMaterial { friction: 0.6, restitution: 1.0 });
    let ball = spawn_box(&mut world, Vec2::new(0.0, 0.51), Vec2::splat(0.5), Some(1.0));
    world.insert(ball, PhysicsMaterial { friction: 0.6, restitution: 1.0 });

    let mut cache = ImpulseCache::default();
    for _ in 0..600 {
        step(&mut world, &mut cache, &SolverParams::default(), G, DT);
    }

    let speed = world.get::<RigidBody>(ball).expect("body").velocity.y;
    assert!(speed.abs() < 1.0, "a resting body must not keep bouncing, got {speed}");
}
```

- [ ] **Step 2: Run, fix the passes until green**

- [ ] **Step 3: Commit**

```bash
git add crates/voltra-physics
git commit -m "feat(physics): add friction and restitution to contacts"
```

---

### Task 7: `PhysicsWorld`, and the editor stepping through it

**Files:**
- Create: `crates/voltra-physics/src/world.rs`
- Modify: `crates/voltra-physics/src/lib.rs`
- Modify: `crates/voltra-core/src/app.rs`
- Modify: `crates/voltra-core/src/app/ui_frame.rs` (only if the borrow needs it)

**Interfaces:**
- Produces: `PhysicsWorld::new()`, `PhysicsWorld::with_step(f32)`,
  `PhysicsWorld::params(&self) -> &SolverParams`,
  `PhysicsWorld::params_mut(&mut self) -> &mut SolverParams`,
  `PhysicsWorld::contacts(&self) -> &[Contact]`,
  `PhysicsWorld::advance(&mut self, &mut World, gravity: Vec2, delta: f32) -> &[Contact]`,
  `PhysicsWorld::cached_pairs(&self) -> usize`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_frame_shorter_than_the_step_simulates_nothing_yet() {
    let mut physics = PhysicsWorld::new();
    let mut world = World::new();
    assert!(physics.advance(&mut world, G, 1.0 / 600.0).is_empty());
}

#[test]
fn a_long_frame_is_capped_rather_than_spiralling() {
    // The clock's own cap, exercised through the solver: a ten-second frame
    // must not run six hundred steps.
    let mut physics = PhysicsWorld::new();
    let (mut world, ball) = floor_and_ball();
    physics.advance(&mut world, G, 10.0);
    let y = world.get::<Transform>(ball).expect("transform").translation.y;
    assert!(y > -1.5, "at most eight steps of falling, got {y}");
}

#[test]
fn contacts_survive_a_frame_that_owed_no_step() {
    let mut physics = PhysicsWorld::new();
    let (mut world, _) = floor_and_ball();
    for _ in 0..300 {
        physics.advance(&mut world, G, 1.0 / 60.0);
    }
    assert!(!physics.contacts().is_empty());
    physics.advance(&mut world, G, 0.0);
    assert!(!physics.contacts().is_empty(), "a frame with no step must not blank them");
}
```

- [ ] **Step 2: Run and watch it fail**

- [ ] **Step 3: Implement `PhysicsWorld`**, then rewire `voltra-core::App`:
replace the `physics_clock: PhysicsClock` and `contacts: Vec<Contact>` fields
with a single `physics_world: PhysicsWorld`, and make `step_physics` call
`advance`. The UI frame reads `self.physics_world.contacts()`; if the borrow
checker objects at the construction site, take the slice into a local binding
before the `&mut` borrow of the world.

- [ ] **Step 4: Run the workspace tests**

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-physics crates/voltra-core
git commit -m "feat(physics): own the clock, params and impulse cache"
```

---

### Task 8: Documentation and verification

**Files:**
- Modify: `docs/ARCHITECTURE.md` (Decisions)
- Modify: `crates/voltra-physics/src/lib.rs` (crate doc: the stage limit is gone)
- Modify: `crates/voltra-core/src/app.rs` (the `with_physics` doc says nothing
  resolves contacts — it does now)

- [ ] **Step 1: Write the decisions**

Three entries, each with its rejected alternatives: TGS Soft as implemented
here (the sub-step order, friction in the relax pass, the coefficient formulas
and where they came from); warm starting and why the persistent unit is the pair
owned by the world, with what Box2D, Godot and Avian each do; the material as a
component rather than fields on `Collider` or `RigidBody`. Add the note that a
second manifold point in 11b-3 needs a per-point key.

- [ ] **Step 2: Run everything**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 3: Look at it**

Launch the editor detached with a stacked scene, wait a few seconds, read the
log, kill it. Contacts drawn by the debug overlay should sit on the touching
faces and the stack should be still.

- [ ] **Step 4: Commit**

```bash
git add docs crates
git commit -m "docs: record the contact solver decisions"
```

## Self-review

- Spec coverage: scope (tasks 5–7), algorithm (5, 6), solver bodies (3),
  warm starting and cache (4, 5, 7), materials (1), parameters (2), files (all),
  tests (3–7), verification (8). The inverted 11b-1 test is named in task 5.
- No placeholders: every step names the file and carries the code or the exact
  rule to apply.
- Type consistency: `SolverParams` fields are used with the same names in tasks
  2, 5 and 7; `ImpulseCache::{warm_start, record, commit, is_empty}` match
  between tasks 4 and 5; `step`'s signature is identical in tasks 5, 6 and 7.
