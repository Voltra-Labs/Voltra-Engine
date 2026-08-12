# Stage 11b-3 — Rotation and two-point manifolds

**Date:** 2026-08-11
**Status:** design

## Scope

Bodies rotate, boxes are oriented, and a contact carries up to two points.

In:

- Angular velocity, rotational inertia and torque arms through the whole solver.
- `Collider::Box` replaces `Collider::Aabb`: the shape follows the transform's
  rotation. The broad phase keeps bounding it with a world AABB, which is what
  that name has always meant.
- Box–box collision by SAT with face clipping, producing **two** manifold points
  for a face contact and one for a vertex contact. Box–circle and circle–circle
  keep producing one.
- A warm-start key per manifold point, which is the extension `solver/cache.rs`
  already names.
- `lock_rotation` on `RigidBody`, and angular damping.

Out, and stated so their absence is not read as a bug:

- **Sleeping and islands** — 11b-4. A settled body is still solved every step,
  and the residual jitter 11b-2 left (~0.04 m/s) stays until sleeping lands.
- **Convex polygons and capsules.** Nothing in the editor can author one. The
  SAT written here takes a vertex list, so the shape is a new `Collider`
  variant, not a new algorithm.
- **Rolling resistance.** A circle on a floor rolls forever. Box2D's
  `rollingResistance` is a per-material parameter, and it belongs with the
  material work rather than here.
- **A rotate gizmo.** The inspector already edits `Transform::rotation`. A
  gizmo is stage 11a's subsystem, not this one.
- **Continuous collision.** A fast body still tunnels; a fast *rotation* is
  capped instead (see "Speed caps").

## What is already here

`PhysicsWorld` owns the clock, `SolverParams` and the `ImpulseCache`. A step
collides once, prepares constraints, warm starts, runs four sub-steps of
(integrate velocities → solve with bias → integrate positions), relaxes once
without bias applying friction, applies restitution, then records the impulses
and scatters the bodies back to the ECS. Everything below is an extension of
that shape, not a replacement: the pass order does not change.

The pieces that change are named in the code already —
`ContactConstraint::effective_mass` says rotation splits it in two,
`solver/cache.rs` says the key gains a point identifier, and `narrow.rs` says
`Contact::point` becomes a torque arm.

## Components

`RigidBody` gains three fields:

```rust
pub angular_velocity: f32,   // radians per second, counter-clockwise
pub angular_damping: f32,    // fraction of spin shed per second
pub lock_rotation: bool,     // a body that takes torque but never turns
```

**Rotational inertia is not a field.** It is computed in
`SolverBodies::gather`, from the body's mass and the collider's world shape:

```text
box     I = m·(hx² + hy²)/3      → inverse_inertia = 3·inv_mass / (hx² + hy²)
circle  I = m·r²/2               → inverse_inertia = 2·inv_mass / r²
```

with `inverse_inertia = 0.0` when the body cannot be pushed, when
`lock_rotation` is set, when there is no collider, or when the shape is
degenerate. Zero reads as *infinite* inertia, exactly as `inverse_mass` does.

Inertia is a function of mass and shape, both of which already live in
components a user edits. Storing it would make a third value that has to be
invalidated whenever either changes — and there is no invalidation mechanism in
this ECS, so the inspector's collider slider would silently leave a body
spinning like the box it used to be. Computing it per step costs four
multiplies per body.

`lock_rotation` is how a body opts out, as `freezeRotation` (Unity),
`lock_rotation` (Godot) and `fixedRotation` (Box2D) all do: a platformer
character must not topple, and its shape says nothing about that.

### `Collider::Aabb` becomes `Collider::Box`

The shape is oriented by the transform from this stage on, so the old name
would be a lie of exactly the kind `CLAUDE.md` forbids — `z_index` for
something that is not a Z. `Collider::world_aabb` keeps its name because it
keeps its meaning: the world-space *bounds*, now the bounds of a rotated box:

```text
half = (|cos|·hx + |sin|·hy,  |sin|·hx + |cos|·hy)
```

This is a breaking change to the scene format's variant name. No scene file in
the repository contains a collider, so no migration is written; a file that has
one fails to parse with the variant named, which is a legible error.

## The narrow phase

`Contact` becomes a manifold:

```rust
pub struct Contact {
    pub a: Entity,
    pub b: Entity,
    /// Unit vector pushing `a` away from `b`. One normal for the manifold.
    pub normal: Vec2,
    points: [ManifoldPoint; 2],
    count: u8,
}

pub struct ManifoldPoint {
    /// World space, on the overlap.
    pub point: Vec2,
    /// Negative while overlapping. The solver's sign, not the old
    /// `penetration`, because the sub-steps track separation.
    pub separation: f32,
    /// Which features produced this point, for warm starting.
    pub id: u16,
}
```

`Contact::points()` returns the live slice; `count` is never read raw. Two
points rather than a `Vec` because a box–box face contact has exactly two and
nothing here can produce three — the same bound Box2D uses.

**Box–box is SAT with face clipping**, the standard algorithm and the one Box2D
v3's `b2CollidePolygons` implements:

1. Work in A's frame, so one rotation is applied to B rather than two to
   everything.
2. Find the maximum separation over A's two face normals and B's two, tracking
   the reference edge on each.
3. If either separation is positive there is no overlap — no speculative margin
   in this stage, matching what the current narrow phase reports.
4. Pick the reference face: A's unless B's separation is greater by more than a
   small bias, which is what stops the reference flip-flopping between frames on
   a near-square contact and undoing warm starting every step. Box2D uses
   `0.1 · linear_slop`; ours is a named constant with the same purpose.
5. Clip the incident face against the reference face's two side planes
   (Sutherland–Hodgman), keep the points whose separation is negative.

**A point's id is its feature pair**: `(reference_vertex << 8) | incident_vertex`,
Box2D's `B2_MAKE_ID` exactly, flipped consistently when B is the reference so
the same physical corner keeps its id across steps. Circle contacts carry id
`0`: a circle has one feature and there is nothing to distinguish.

Box–circle rotates the circle's centre into the box's frame, clamps, and
un-rotates the result — the existing algorithm with two rotations added.
Circle–circle is unchanged. The degenerate cases already handled (concentric
centres, zero-area shapes, the `FALLBACK_NORMAL`) stay handled where they are.

## The solver with rotation

Every formula below is Box2D v3's `contact_solver.c`, adapted to one normal per
manifold and our two-point maximum.

**Anchors.** Each point carries `anchor_a` and `anchor_b`: the contact point
relative to each body's centre, in world orientation at the moment the step
began. Our bodies have no separate centre of mass — the transform's translation
is the centre — so an anchor is `point − translation`.

**Effective mass, per point, per axis:**

```text
rn  = cross(r, normal)                 rt  = cross(r, tangent)
k_n = mA + mB + iA·rnA² + iB·rnB²      k_t = mA + mB + iA·rtA² + iB·rtB²
normal_mass = 1/k_n                    tangent_mass = 1/k_t
```

This is what `effective_mass` splits into, and why one number could not stay.

**Separation through the sub-steps** stops being "base plus how far the centres
moved" and becomes the same with the anchors carried along:

```text
base_separation = separation₀ − dot(rB − rA, normal)
rsA = rotate(delta_rotation_a, anchor_a)      (likewise B)
s   = dot(normal, delta_position + (rsB − rsA)) + base_separation
```

The normal is still held constant for the whole step and the narrow phase still
runs once. `s > 0` is still the speculative case, biased at `s/h`.

**Velocity at a point** gains the spin: `v + cross(w, r)`, i.e.
`v + w · perp(r)`. An impulse `P` applies as

```text
v += ±P·inv_mass          w += ±inv_inertia · cross(r, P)
```

Friction uses the **fixed** anchors for its Jacobian while the normal solve uses
the rotated ones — Box2D's split, because a friction anchor that moves with the
body turns static friction into a slow drift.

Friction stays in the relax pass, clamped per point against *that point's*
accumulated normal impulse. Restitution stays a pass of its own, per point,
gated on the point's own `max_normal_impulse` and the approach speed captured
before the solve.

**Warm starting** applies both impulses of both points before the sub-steps, and
now also their angular part. The cache key becomes `(Entity, Entity, u16)`:
the pair plus the point id. A point that changes feature — a box tipping onto
its corner — starts cold, which is correct: the impulse belonged to a contact
that no longer exists.

## Integration and speed caps

`integrate_velocities` applies angular damping with the same clamped
`(1 − damping·h).max(0.0)` as the linear one. `integrate_positions`
accumulates `delta_rotation += angular_velocity · h`, and `scatter` writes
`transform.rotation += delta_rotation` alongside the translation.

**Angular speed is capped per sub-step** at `max_rotation / h` radians per
second, with `max_rotation = 0.25π`, Box2D's `B2_MAX_ROTATION`. A body spinning
further than a quarter turn within one sub-step invalidates the constant normal
the whole step is built on: the contact would be solved against a face that has
already turned away. The cap is a `SolverParams` field like every other tuning
value.

## Debug draw

A box outline becomes its four rotated corners, and a contact draws one normal
per point. Both are how a wrong manifold becomes visible: two points on a
resting box, one on a tipped one, and the normal pointing out of the face rather
than out of a corner.

## Files

`narrow.rs` is at its limit and gains three algorithms, so it splits **first**,
in a move-only commit, before anything is added:

```
crates/voltra-physics/src/
  narrow.rs                 the Contact/ManifoldPoint types and the dispatch
  narrow/
    circle.rs               circle–circle
    box_circle.rs           box–circle, both argument orders
    box_box.rs              SAT, face clipping and the feature ids
```

`solver/constraint.rs` gains a `ContactPoint` struct and keeps its file;
`solver/body.rs` gains the angular fields and the inertia computation;
`solver/contact.rs` (the passes) grows a loop over points inside each pass.
Everything else keeps its shape.

## Tests

Behaviour, in `step.rs`, all of which fail today:

- A box dropped flat on a floor settles **without rocking**: after it comes to
  rest its rotation stays within a hair of where it started, over 200 steps.
  This is the test the second manifold point exists for.
- A box dropped on a corner tips onto a face and stays there.
- A plank resting across a fulcrum, loaded on one end, rotates that end down.
- An off-centre impulse spins a body: linear *and* angular velocity change, and
  the sign of the spin follows the side that was hit.
- A body with `lock_rotation` takes the same hit and does not turn, while its
  linear response is unchanged.
- A box on a slope with high friction does not slide; with zero friction it
  slides and does not spin (a box, unlike a circle, has no reason to).
- Angular damping brings a free spin to a stop; without it the spin is kept.
- A body spun absurdly fast is capped rather than passing through the floor.
- Energy does not grow: a scene of stacked rotated boxes has no more kinetic
  energy after 300 steps than after 10.

Unit level:

- SAT reports the axis of least penetration, and the reference face does not
  flip between two frames of a resting box (the bias).
- A face contact yields two points; a corner contact yields one.
- A point id is stable across steps while the features are, and changes when
  the contact moves to another corner.
- Inertia: a box's is `m(hx²+hy²)/3`, a circle's is `mr²/2`, zero for an
  immovable body, zero when `lock_rotation`, zero without a collider.
- `world_aabb` of a box rotated 45° is the larger, correct bound.
- The cache evicts a point whose id disappeared while keeping the pair's other
  point.

## Rejected

- **Storing `inverse_inertia` on `RigidBody`.** A derived value with no
  invalidation path; editing the collider would leave it stale.
- **Keeping one `effective_mass`.** It is only correct when every contact acts
  through the centre of mass, which is exactly what this stage ends.
- **A `Vec<ManifoldPoint>`.** An allocation per contact per step for a list that
  cannot exceed two.
- **Keying the cache by contact position (Godot's recycle radius).** Needed when
  point identity cannot be recovered from features; ours can, and a feature id
  is exact where a radius is a guess.
- **GJK/EPA.** General, and slower than SAT for two boxes; it would also return
  a single deepest point, which is the problem this stage is solving.
- **Convex polygons now.** The SAT here is written over a vertex list, so the
  polygon is a `Collider` variant later, not a rewrite. Nothing can author one
  today.
- **A speculative margin.** Box2D reports contacts slightly before touching, so
  the solver can stop a body before it overlaps. It interacts with the whole
  separation pipeline and deserves its own stage rather than a corner of this
  one.
