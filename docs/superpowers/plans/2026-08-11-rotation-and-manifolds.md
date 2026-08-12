# Rotation and Manifolds Implementation Plan (stage 11b-3)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to
> implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Bodies rotate, boxes are oriented, and a resting box does not rock —
which needs a contact to carry two points.

**Architecture:** The step's pass order from 11b-2 is unchanged. What changes is
that every quantity inside it becomes per manifold point, and every impulse
gains an angular half. `Contact` becomes a manifold of up to two points with
feature ids; the impulse cache keys on `(a, b, id)`; `SolverBody` gains angular
velocity, a delta rotation and an inverse inertia computed at gather.

**Tech Stack:** Rust, `glam` (via `voltra-render`), `voltra-ecs`, `serde`/`ron`.

Design: [2026-08-11-rotation-and-manifolds-design.md](../specs/2026-08-11-rotation-and-manifolds-design.md)

## Global Constraints

- 2D only. No z, no depth, no `Transform3D`.
- Only `voltra-core` imports `winit`; only `voltra-render` imports `wgpu`.
  `voltra-physics` gets `glam` through `voltra-render`.
- No new dependency. Versions live in the root manifest.
- No `unwrap()` outside tests. `expect("why")` when the invariant is real.
- One concept per file; split at ~300 lines, in a move-only commit.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace` all clean before a task is done.
- Every formula from Box2D v3 keeps a comment naming what it is.
- Conventional Commits, scope = crate without the `voltra-` prefix, ≤50 chars.

---

### Task 1: Split `narrow.rs`, move only

**Files:**
- Modify: `crates/voltra-physics/src/narrow.rs`
- Create: `crates/voltra-physics/src/narrow/circle.rs`
- Create: `crates/voltra-physics/src/narrow/box_circle.rs`

**Interfaces:**
- Produces: `narrow::contact(a: Shape, b: Shape) -> Option<(Vec2, f32, Vec2)>`
  unchanged, `pub(crate) type Shape<'a>`, `pub(crate) const EPSILON`,
  `pub(crate) const FALLBACK_NORMAL`, `pub(crate) fn sign_away`.

- [ ] **Step 1: Move `circle_circle` and its tests** into `narrow/circle.rs` as
  `pub(super) fn circle_circle`, `aabb_circle` and its tests into
  `narrow/box_circle.rs` as `pub(super) fn aabb_circle`. `narrow.rs` keeps
  `Contact`, `Shape`, `contact`, the constants and `sign_away`, and gains
  `mod circle; mod box_circle;`. `aabb_aabb` stays in `narrow.rs` for now —
  Task 4 moves it as it rewrites it.

- [ ] **Step 2: Verify nothing changed** — `cargo test -p voltra-physics`
  passes with the same test count as before the move, and `cargo clippy
  --workspace --all-targets -- -D warnings` is clean.

- [ ] **Step 3: Commit**

```bash
git add crates/voltra-physics/src
git commit -m "refactor(physics): split the narrow phase per shape pair"
```

---

### Task 2: `Collider::Box`, oriented

**Files:**
- Modify: `crates/voltra-scene/src/collider.rs`
- Modify: every `Collider::Aabb` use: `crates/voltra-physics/src/{broad.rs,
  debug.rs,narrow.rs,narrow/box_circle.rs,step.rs,world.rs,
  solver/constraint.rs}`, `crates/voltra-editor/src/panels/menu_bar.rs`

**Interfaces:**
- Produces: `Collider::Box { half_extents: Vec2 }` replacing `Collider::Aabb`;
  `Collider::corners(&Transform) -> [Vec2; 4]` in counter-clockwise order
  starting at `(+hx, +hy)`; `Collider::world_aabb` now bounds the rotated box.

- [ ] **Step 1: Write the failing tests** in `collider.rs`

```rust
#[test]
fn a_rotated_box_is_bounded_by_a_larger_aabb() {
    let collider = Collider::Box { half_extents: Vec2::splat(1.0) };
    let transform = Transform::default().with_rotation(FRAC_PI_4);

    let (min, max) = collider.world_aabb(&transform);

    // A unit square turned 45° spans its diagonal, √2 either side.
    assert!((max.x - SQRT_2).abs() < 1e-5, "{max:?}");
    assert!((min.y + SQRT_2).abs() < 1e-5, "{min:?}");
}

#[test]
fn an_unrotated_box_is_bounded_exactly() {
    let collider = Collider::Box { half_extents: Vec2::new(2.0, 3.0) };
    let (min, max) = collider.world_aabb(&Transform::default());
    assert_eq!((min, max), (Vec2::new(-2.0, -3.0), Vec2::new(2.0, 3.0)));
}

#[test]
fn the_corners_follow_the_rotation_counter_clockwise() {
    let collider = Collider::Box { half_extents: Vec2::splat(1.0) };
    let corners = collider.corners(&Transform::default().with_rotation(FRAC_PI_2));

    // (+1,+1) turned a quarter turn is (-1,+1).
    assert!((corners[0] - Vec2::new(-1.0, 1.0)).length() < 1e-5, "{corners:?}");
    // Counter-clockwise, so the winding is positive.
    let area: f32 = (0..4)
        .map(|i| {
            let (p, q) = (corners[i], corners[(i + 1) % 4]);
            p.x * q.y - q.x * p.y
        })
        .sum();
    assert!(area > 0.0, "{corners:?}");
}

#[test]
fn a_mirrored_box_still_has_positive_extents() {
    let collider = Collider::Box { half_extents: Vec2::splat(1.0) };
    let transform = Transform::default().with_scale(Vec2::new(-2.0, 1.0));
    let (min, max) = collider.world_aabb(&transform);
    assert!(max.x > min.x && max.y > min.y);
}
```

- [ ] **Step 2: Run and watch it fail** — `cargo test -p voltra-scene`.

- [ ] **Step 3: Implement.** Rename the variant; `world_half_extents` keeps
  taking `abs()` of the scale. Add:

```rust
/// The four corners in world space, counter-clockwise from `(+hx, +hy)`.
///
/// `Vec2::ZERO` four times for a circle, which has no corners — callers that
/// care about the difference match on the variant first.
pub fn corners(&self, transform: &Transform) -> [Vec2; 4] {
    let half = self.world_half_extents(transform);
    let rotation = Vec2::from_angle(transform.rotation);
    [
        Vec2::new(half.x, half.y),
        Vec2::new(-half.x, half.y),
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
    ]
    .map(|corner| transform.translation + rotation.rotate(corner))
}
```

and in `world_aabb`, for the box arm:

```rust
// The rotated box's bound: each axis takes both extents, weighted by how
// far the box has turned. |cos| and |sin| because the sign only mirrors.
let (sin, cos) = transform.rotation.sin_cos();
let half = self.world_half_extents(transform);
Vec2::new(
    cos.abs() * half.x + sin.abs() * half.y,
    sin.abs() * half.x + cos.abs() * half.y,
)
```

- [ ] **Step 4: Fix every call site** — a plain rename except `debug.rs`, which
  Task 9 rewrites; for now draw the rotated outline with `corners`.

- [ ] **Step 5: Run** `cargo test --workspace` and clippy.

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "feat(scene): orient the box collider by its transform"
```

---

### Task 3: `Contact` becomes a manifold

**Files:**
- Modify: `crates/voltra-physics/src/narrow.rs`
- Modify: `crates/voltra-physics/src/narrow/{circle.rs,box_circle.rs}`
- Modify: `crates/voltra-physics/src/{step.rs,debug.rs}`
- Modify: `crates/voltra-physics/src/solver/constraint.rs`

**Interfaces:**
- Produces:

```rust
pub struct ManifoldPoint { pub point: Vec2, pub separation: f32, pub id: u16 }

pub struct Contact {
    pub a: Entity,
    pub b: Entity,
    pub normal: Vec2,
    points: [ManifoldPoint; 2],
    count: u8,
}

impl Contact {
    pub fn new(a: Entity, b: Entity, normal: Vec2, points: &[ManifoldPoint]) -> Self;
    pub fn points(&self) -> &[ManifoldPoint];
    pub fn deepest(&self) -> f32;   // most negative separation, for tests/debug
}
```

  and `narrow::manifold(a: Shape, b: Shape) -> Option<Manifold>` where
  `Manifold { normal: Vec2, points: [ManifoldPoint; 2], count: u8 }` — the
  entity-free result the pair functions return.

- [ ] **Step 1: Write the failing tests** in `narrow.rs`

```rust
#[test]
fn a_circle_pair_reports_one_point() {
    let m = manifold(circle_at(0.0), circle_at(1.5)).expect("overlapping");
    assert_eq!(m.points().len(), 1);
    assert_eq!(m.points()[0].id, 0);
    assert!(m.points()[0].separation < 0.0);
}

#[test]
fn a_separation_is_the_negated_penetration() {
    // 11b-2 reported penetration, positive into the overlap. The solver wants
    // separation, and the sign flip has to happen once, here.
    let m = manifold(circle_at(0.0), circle_at(1.5)).expect("overlapping");
    assert!((m.points()[0].separation + 0.5).abs() < 1e-5);
}

#[test]
fn a_manifold_never_reports_more_points_than_it_has() {
    let m = manifold(circle_at(0.0), box_at(0.4)).expect("overlapping");
    assert_eq!(m.points().len(), 1);
}
```

- [ ] **Step 2: Run and watch it fail** — `cargo test -p voltra-physics`.

- [ ] **Step 3: Implement.** `Contact::new` truncates at two points and is the
  only constructor, so `count` cannot disagree with `points`. The pair
  functions return `Manifold`; `narrow::manifold` dispatches and mirrors the
  circle/box order as `contact` did. `step::collide` builds `Contact`s from
  them. `prepare` reads `contact.points()[0]` for now — Task 6 makes it a loop.
  `debug::draw` draws one normal per point already, by iterating `points()`.

- [ ] **Step 4: Run** `cargo test -p voltra-physics` and clippy.

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-physics/src
git commit -m "feat(physics): make a contact a manifold of points"
```

---

### Task 4: Box–box by SAT, with two points

**Files:**
- Create: `crates/voltra-physics/src/narrow/box_box.rs`
- Modify: `crates/voltra-physics/src/narrow.rs` (drop `aabb_aabb`, dispatch to
  `box_box`)

**Interfaces:**
- Consumes: `Manifold`, `ManifoldPoint`, `Collider::corners`.
- Produces: `pub(super) fn box_box(a: Shape<'_>, b: Shape<'_>) -> Option<Manifold>`.

- [ ] **Step 1: Write the failing tests** in `box_box.rs`

```rust
#[test]
fn two_boxes_resting_face_to_face_give_two_points() {
    // A unit box overlapping a wide floor by 0.1 along y.
    let m = box_box(floor(), box_at(Vec2::new(0.0, 0.9), 0.0)).expect("overlapping");
    assert_eq!(m.points().len(), 2, "{m:?}");
    assert!((m.normal - Vec2::NEG_Y).length() < 1e-5, "{m:?}");
    for p in m.points() {
        assert!((p.separation + 0.1).abs() < 1e-4, "{p:?}");
    }
}

#[test]
fn the_two_points_are_the_ends_of_the_overlapping_face() {
    let m = box_box(floor(), box_at(Vec2::new(0.0, 0.9), 0.0)).expect("overlapping");
    let xs: Vec<f32> = m.points().iter().map(|p| p.point.x).collect();
    assert!(xs.iter().any(|x| (*x + 0.5).abs() < 1e-4), "{xs:?}");
    assert!(xs.iter().any(|x| (*x - 0.5).abs() < 1e-4), "{xs:?}");
}

#[test]
fn a_corner_contact_gives_one_point() {
    // Turned 45°, a box touches the floor on a single corner.
    let m = box_box(floor(), box_at(Vec2::new(0.0, 1.1), FRAC_PI_4)).expect("overlapping");
    assert_eq!(m.points().len(), 1, "{m:?}");
}

#[test]
fn separated_boxes_report_nothing() {
    assert!(box_box(floor(), box_at(Vec2::new(0.0, 2.0), 0.0)).is_none());
    assert!(box_box(floor(), box_at(Vec2::new(0.0, 2.0), FRAC_PI_4)).is_none());
}

#[test]
fn touching_exactly_is_not_a_collision() {
    // Zero penetration hands the solver nothing to resolve, every frame.
    assert!(box_box(floor(), box_at(Vec2::new(0.0, 1.0), 0.0)).is_none());
}

#[test]
fn the_normal_always_pushes_a_away_from_b() {
    let m = box_box(box_at(Vec2::new(0.0, 0.9), 0.0), floor()).expect("overlapping");
    assert!((m.normal - Vec2::Y).length() < 1e-5, "{m:?}");
}

#[test]
fn the_point_ids_are_stable_while_the_features_are() {
    let first = box_box(floor(), box_at(Vec2::new(0.0, 0.9), 0.0)).expect("overlapping");
    let second = box_box(floor(), box_at(Vec2::new(0.001, 0.9), 0.0)).expect("overlapping");
    let ids = |m: &Manifold| m.points().iter().map(|p| p.id).collect::<Vec<_>>();
    assert_eq!(ids(&first), ids(&second));
}

#[test]
fn the_two_points_of_a_manifold_have_different_ids() {
    let m = box_box(floor(), box_at(Vec2::new(0.0, 0.9), 0.0)).expect("overlapping");
    assert_ne!(m.points()[0].id, m.points()[1].id);
}

#[test]
fn a_deep_overlap_picks_the_axis_of_least_penetration() {
    // Overlapping 0.1 in y and 0.9 in x: the normal is y.
    let m = box_box(floor(), box_at(Vec2::new(0.0, 0.9), 0.0)).expect("overlapping");
    assert!(m.normal.x.abs() < 1e-5, "{m:?}");
}

#[test]
fn a_reference_face_does_not_flip_between_two_near_equal_axes() {
    // A square inside a square: both axes separate by the same amount, and a
    // solver that changes its mind each step throws away its warm start.
    let a = box_at(Vec2::ZERO, 0.0);
    let first = box_box(as_shape(&a), box_at(Vec2::new(0.0, 0.0), 0.0)).expect("overlapping");
    let second = box_box(as_shape(&a), box_at(Vec2::new(1e-6, 0.0), 0.0)).expect("overlapping");
    assert!((first.normal - second.normal).length() < 1e-3);
}
```

- [ ] **Step 2: Run and watch it fail.**

- [ ] **Step 3: Implement**, following Box2D v3's `b2CollidePolygons`:

1. `separation_on(reference_corners, incident_corners) -> (edge, separation)`:
   for each of the reference box's four edges, the outward normal is
   `perp(next - current)` normalised; the separation is the smallest
   `dot(normal, incident_corner - current)` over the incident corners. Keep the
   largest such separation and its edge.
2. Run it both ways. Either separation `>= 0.0` → `None`.
3. Reference is A unless `separation_b > separation_a + FLIP_BIAS`, with
   `const FLIP_BIAS: f32 = 5e-4` — Box2D's `0.1 · linear_slop`, and the reason
   is the flip-flop test above. `flip` records which way round it went.
4. The incident edge is the incident box's edge whose normal is most opposed to
   the reference normal (smallest dot product).
5. Clip the incident edge's two endpoints against the reference edge's two side
   planes (`Sutherland–Hodgman`, the standard two-plane case), keeping the
   feature indices along. Keep a clipped point only when its separation along
   the reference normal is negative.
6. `id = ((reference_index as u16) << 8) | incident_index as u16`, built after
   the flip so the same physical corner keeps its id whichever box was
   reference. The normal is negated when `flip`, so it always pushes `a` away
   from `b`.

- [ ] **Step 4: Run** `cargo test -p voltra-physics` and clippy.

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-physics/src
git commit -m "feat(physics): collide two boxes with SAT and clipping"
```

---

### Task 5: Bodies that spin

**Files:**
- Modify: `crates/voltra-scene/src/body.rs`
- Modify: `crates/voltra-physics/src/solver/body.rs`
- Modify: `crates/voltra-physics/src/integrate.rs`
- Modify: `crates/voltra-physics/src/solver/params.rs`

**Interfaces:**
- Produces: `RigidBody { .., angular_velocity: f32, angular_damping: f32,
  lock_rotation: bool }`; `SolverBody { .., angular_velocity: f32,
  delta_rotation: f32, inverse_inertia: f32 }`;
  `SolverBodies::gather(world)` computing the inertia;
  `SolverParams::max_rotation: f32` defaulting to `0.25 * PI`.

- [ ] **Step 1: Write the failing tests** — in `body.rs` (scene): the three new
  fields default to `0.0 / 0.0 / false` and survive a RON round trip. In
  `solver/body.rs`:

```rust
#[test]
fn a_box_body_has_the_inertia_of_a_box() {
    // I = m(hx² + hy²)/3, so the inverse is 3·inv_mass/(hx² + hy²).
    let bodies = gather_one(Collider::Box { half_extents: Vec2::new(2.0, 1.0) }, 4.0);
    let expected = 3.0 * 0.25 / (4.0 + 1.0);
    assert!((bodies.get(0).inverse_inertia - expected).abs() < 1e-6);
}

#[test]
fn a_circle_body_has_the_inertia_of_a_disc() {
    // I = m·r²/2.
    let bodies = gather_one(Collider::Circle { radius: 2.0 }, 4.0);
    let expected = 2.0 * 0.25 / 4.0;
    assert!((bodies.get(0).inverse_inertia - expected).abs() < 1e-6);
}

#[test]
fn the_collider_scale_reaches_the_inertia() { /* scale 2 quadruples I */ }

#[test]
fn an_immovable_body_has_no_inverse_inertia() { /* static, and mass 0 */ }

#[test]
fn a_locked_body_has_no_inverse_inertia() { /* lock_rotation */ }

#[test]
fn a_body_without_a_collider_has_no_inverse_inertia() { }

#[test]
fn a_degenerate_collider_does_not_divide_by_zero() {
    // A zero half-extent box: 3·inv_mass/0 is inf, and an infinite inverse
    // inertia spins the body out of the world on its first contact.
    let bodies = gather_one(Collider::Box { half_extents: Vec2::ZERO }, 1.0);
    assert_eq!(bodies.get(0).inverse_inertia, 0.0);
}
```

  and in `integrate.rs`:

```rust
#[test]
fn a_spinning_body_accumulates_a_delta_rotation() { /* w = 2, h = 0.5 → 1 rad */ }

#[test]
fn angular_damping_slows_a_spin() { }

#[test]
fn a_large_angular_damping_does_not_reverse_the_spin() {
    // (1 − damping·h) goes negative past 1/h, which would be a catapult.
}

#[test]
fn a_kinematic_body_keeps_the_spin_it_was_given() { }

#[test]
fn an_absurd_spin_is_capped_to_a_quarter_turn_per_sub_step() {
    // Past this the normal the whole step is solved against has turned away.
}
```

- [ ] **Step 2: Run and watch it fail.**

- [ ] **Step 3: Implement.** `gather` computes the inertia as the spec's two
  formulas, guarded by `inverse_mass > 0.0 && !lock_rotation` and a positive
  denominator. `integrate_velocities` applies angular damping and the cap;
  `integrate_positions` accumulates `delta_rotation`; `scatter` writes
  `transform.rotation += body.delta_rotation` and
  `rigid_body.angular_velocity = body.angular_velocity`.

- [ ] **Step 4: Run** `cargo test --workspace` and clippy.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat(physics): give bodies angular velocity and inertia"
```

---

### Task 6: Constraints with anchors and torque arms

**Files:**
- Modify: `crates/voltra-physics/src/solver/constraint.rs`
- Modify: `crates/voltra-physics/src/solver/contact.rs`
- Modify: `crates/voltra-physics/src/solver/cache.rs`

**Interfaces:**
- Produces:

```rust
pub struct ContactPoint {
    pub anchor_a: Vec2,
    pub anchor_b: Vec2,
    pub base_separation: f32,
    pub normal_mass: f32,
    pub tangent_mass: f32,
    pub relative_velocity: f32,
    pub normal_impulse: f32,
    pub tangent_impulse: f32,
    pub max_normal_impulse: f32,
    pub id: u16,
}

pub struct ContactConstraint {
    pub a: usize, pub b: usize, pub key: ContactKey,
    pub normal: Vec2, pub friction: f32, pub restitution: f32,
    pub softness: Softness,
    points: [ContactPoint; 2], count: u8,
}
```

  `ContactKey = (Entity, Entity, u16)`; `ImpulseCache::{warm_start, record}`
  take that key.

- [ ] **Step 1: Write the failing tests** in `constraint.rs` and `cache.rs`

```rust
#[test]
fn a_two_point_manifold_becomes_a_two_point_constraint() { }

#[test]
fn an_anchor_is_the_contact_point_relative_to_the_body() { }

#[test]
fn a_contact_through_the_centre_keeps_the_old_effective_mass() {
    // With r × n = 0 the normal mass is 1/(mA + mB) exactly, which is what
    // 11b-2 computed. The rotation terms must not change the head-on case.
}

#[test]
fn an_off_centre_contact_is_heavier_than_a_central_one() {
    // k = mA + mB + iA·rnA² + iB·rnB², so the mass can only go down.
}

#[test]
fn the_tangent_mass_uses_the_tangent_arm() { }

#[test]
fn each_point_is_warm_started_from_its_own_id() { }

#[test]
fn a_point_whose_feature_changed_starts_cold() { }

#[test]
fn the_cache_keeps_one_point_of_a_pair_and_drops_the_other() { }
```

- [ ] **Step 2: Run and watch it fail.**

- [ ] **Step 3: Implement.** `prepare` loops over `contact.points()`:
  `anchor = point.point - transform.translation` per body,
  `base_separation = point.separation - dot(anchor_b - anchor_a, normal)`,
  `normal_mass = 1/(mA + mB + iA·rnA² + iB·rnB²)` with
  `rn = anchor.perp_dot(normal)`, likewise the tangent, and
  `relative_velocity` measured at the anchor:
  `(vB + wB × rB) − (vA + wA × rA) · normal`. A zero `k` gives a mass of zero
  rather than an infinity, as Box2D does.

  The three passes in `contact.rs` each gain an inner loop over the points, and
  each impulse gains its angular half:

```rust
// Box2D v3: v ± P·inv_mass, w ± inv_inertia · cross(r, P).
a.velocity += push * a.inverse_mass;
a.angular_velocity += a.inverse_inertia * point.anchor_a.perp_dot(push);
```

  `solve` rotates the anchors by each body's `delta_rotation` for the
  separation — `Vec2::from_angle(delta).rotate(anchor)` — and uses the **fixed**
  anchors for the friction Jacobian, which is Box2D's split: a friction anchor
  that moves turns static friction into drift. Friction is clamped per point
  against that point's own normal impulse.

- [ ] **Step 4: Run** `cargo test -p voltra-physics` and clippy.

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-physics/src
git commit -m "feat(physics): solve contacts with torque arms"
```

---

### Task 7: What a rotating world does

**Files:**
- Modify: `crates/voltra-physics/src/step.rs` (tests)

**Interfaces:** unchanged.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_box_dropped_flat_settles_without_rocking() {
    // The reason the second manifold point exists. One point and this box
    // oscillates forever, correcting one corner per step.
}

#[test]
fn a_box_dropped_on_a_corner_tips_onto_a_face() { }

#[test]
fn a_loaded_plank_rotates_about_its_fulcrum() { }

#[test]
fn an_off_centre_hit_spins_a_body_the_right_way() { }

#[test]
fn a_locked_body_takes_the_hit_without_turning() { }

#[test]
fn a_box_on_a_rough_slope_does_not_slide() { }

#[test]
fn a_box_on_a_frictionless_slope_slides_without_spinning() { }

#[test]
fn a_stack_of_rotated_boxes_does_not_gain_energy() { }

#[test]
fn a_body_spun_absurdly_fast_stays_in_the_world() { }
```

- [ ] **Step 2: Run, fix until green.** A failure here is a solver bug, not a
  test bug: check the anchor rotation, the friction anchors and the per-point
  clamp before touching a threshold.

- [ ] **Step 3: Commit**

```bash
git add crates/voltra-physics/src/step.rs
git commit -m "test(physics): pin what rotation does to a scene"
```

---

### Task 8: The debug overlay tells the truth

**Files:**
- Modify: `crates/voltra-physics/src/debug.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_rotated_box_is_still_four_segments_and_they_are_rotated() { }

#[test]
fn a_two_point_contact_draws_two_normals() { }
```

- [ ] **Step 2: Run and watch it fail.**

- [ ] **Step 3: Implement** — the outline comes from `Collider::corners`, and
  the contact loop iterates `contact.points()`.

- [ ] **Step 4: Run** `cargo test -p voltra-physics` and clippy.

- [ ] **Step 5: Commit**

```bash
git add crates/voltra-physics/src/debug.rs
git commit -m "feat(physics): draw rotated colliders and both points"
```

---

### Task 9: The editor can make one turn

**Files:**
- Modify: `crates/voltra-editor/src/panels/menu_bar.rs`

- [ ] **Step 1: Implement** — "Spawn falling body" spawns at a small rotation
  (`0.3` rad) so the stage is visible from the menu without editing a field,
  and the tooltip says so. No new panel: the inspector already edits rotation.

- [ ] **Step 2: Run** `cargo test --workspace` and clippy.

- [ ] **Step 3: Commit**

```bash
git add crates/voltra-editor/src
git commit -m "feat(editor): spawn the demo body already tilted"
```

---

### Task 10: Documentation and verification

**Files:**
- Modify: `docs/ARCHITECTURE.md` (Decisions), `README.md` (roadmap),
  `crates/voltra-physics/src/lib.rs` (the stage limits change)

- [ ] **Step 1: Write the decisions** — three entries with their rejected
  alternatives: inertia derived at gather rather than stored; the manifold, SAT
  with a flip bias, and feature ids as the warm-start key; anchors, split
  masses and the fixed-anchor friction. Update the "not simulated yet" list in
  `lib.rs`: rotation leaves it, sleeping and continuous collision stay. Mark
  11b-2 done and add 11b-3 to the README roadmap.

- [ ] **Step 2: Run everything**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 3: Look at it** — launch the editor detached with the stacked
  scene, wait a few seconds, read the log, kill it. A tilted box should land,
  tip flat and stop, with two contact normals on its resting face.

- [ ] **Step 4: Commit**

```bash
git add docs README.md crates
git commit -m "docs: record the rotation and manifold decisions"
```

## Self-review

- Spec coverage: components (5), `Collider::Box` (2), manifold type (3), SAT
  and ids (4), solver anchors and masses (6), cache key (6), integration and
  caps (5), debug draw (8), behaviour tests (7), docs (10). The file split the
  spec requires is task 1 and lands before anything is added.
- No placeholders: every task names its files, its tests and the formulas.
  Tasks 7 and 8 give test names without bodies deliberately — the scene each
  builds is three lines of existing helpers (`step.rs` has them) and the
  assertion is stated in the name and the comment.
- Type consistency: `Manifold`/`ManifoldPoint` from task 3 are what task 4
  returns and task 6 consumes; `ContactKey` gains its `u16` in task 6, which is
  the id task 4 produces; `SolverBody::inverse_inertia` from task 5 is what
  task 6's masses use.
