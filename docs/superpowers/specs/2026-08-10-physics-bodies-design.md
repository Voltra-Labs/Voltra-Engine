# Bodies, integration and contacts — design

Date: 2026-08-10
Status: approved
Stage: 11b-1 of 11b

## What this is, and what it is not

Stage 11b is physics. This spec is its first third: **bodies move, and overlaps
are detected and reported. Nothing is pushed apart yet.**

| | Delivers | Visible |
| --- | --- | --- |
| **11b-1**, this spec | `voltra-physics`: `RigidBody`, `Collider`, a fixed-step integrator, contact detection, debug draw | Yes — shapes and contacts drawn over the scene |
| **11b-2** | The contact solver: bodies stop overlapping, stack and rest | Later, its own spec |
| **11b-3** | Triggers, queries, layers | Later |

Splitting here is not caution, it is where the seam already is. Integration,
broad phase, narrow phase and constraint solving are four subsystems with four
failure modes, and a contact list is a thing you can look at and check before
anything depends on it being correct. Solving contacts you cannot see is how a
physics engine ends up with a solver tuned against a broken normal.

## The decision that had to be made first

**Written in-house, native to our ECS. Not Rapier2D.**

`CLAUDE.md` bans ECS and framework crates but explicitly allows physics as a
leaf library, so this was open rather than pre-decided.

The Rust ecosystem already ran the experiment. `bevy_rapier` is the mature,
battle-tested option, and it *"has to maintain a separate physics world and
synchronize a ton of data with Bevy each frame"*. `avian` — the successor to
`bevy_xpbd` — exists precisely because of that, and is *"built for Bevy with
Bevy, using the ECS for both the internals and the public API"*. We have a
hand-written ECS. Adopting Rapier here would reproduce `bevy_rapier`'s shape
exactly: a `RigidBodyHandle` per entity, a parallel world, and transforms copied
both ways every frame — plus a scene format that has to serialise a handle into
something stable, which a handle is not.

Writing it means bodies are components, the scene format already round-trips
them, and there is nothing to synchronise because there is no second world.

### Rejected

- **Rapier2D as a leaf library.** Saves months and is cross-platform
  deterministic. Costs the parallel world above, and makes the physics something
  we no longer understand from the inside on the day it misbehaves — which for a
  simulation is the day that matters.
- **"Ours now, Rapier if it hurts."** Sounds prudent, and is usually the worst
  of both: the API gets designed twice, and the migration arrives after scenes
  have been saved in the old shape. A scene file is the one thing that cannot be
  refactored freely.

## How the established engines do it

- **The integrator.** Semi-implicit (symplectic) Euler is what every game
  physics engine uses: `v += a·dt` *then* `p += v·dt`. Explicit Euler — position
  first — injects energy and a stack of boxes slowly climbs. The cost of the
  correct one is the order of two lines.
- **The solver, when it arrives.** Erin Catto, Box2D's author, implemented eight
  solvers side by side in [Solver2D](https://box2d.org/posts/2024/02/solver2d/)
  — PGS, three position-corrected variants, PGS_Soft, TGS_Sticky, TGS_Soft,
  TGS_NGS and XPBD — and adopted **TGS_Soft**, renamed "Soft Step", for Box2D
  v3. XPBD was rejected there for friction trouble and precision loss far from
  the origin. That is 11b-2's answer, recorded now so it is not re-litigated
  from scratch; nothing in this spec forecloses it, because a solver consumes a
  contact list and this spec produces one.
- **Inverse mass, not mass.** Box2D stores `m_invMass`. Every formula divides by
  mass, and infinite mass — a static body — is `0.0` rather than a special case
  threaded through each of them.
- **A fixed timestep.** Physics that integrates with the render delta changes
  behaviour with the frame rate, and a stall makes it explode. Every engine runs
  it on a fixed step with an accumulator.

## Where the components live, and why not in `voltra-physics`

**`RigidBody` and `Collider` live in `voltra-scene`. `voltra-physics` holds the
simulation and no components of its own.**

This is forced, not preferred. `ComponentRegistry::with_defaults` is in
`voltra-scene` (`format/registry.rs:56`) and is what makes a scene file
round-trip. Putting the components in `voltra-physics` means either

- `voltra-scene` registers them, so `voltra-scene → voltra-physics`, while
  integration needs `Transform`, so `voltra-physics → voltra-scene`. A cycle.
- Or the registry stops being complete on its own and every caller has to
  remember a second `register` call. There are already four call sites of
  `with_defaults`, and forgetting one does not fail: it silently drops the
  component from that save. A scene format that loses data when a caller forgets
  a line is not a format.

So the split follows the one the repo already has: `voltra-scene` is components
and the geometry they turn into — `Transform`, `Sprite`, `SpriteBatch`, `pick` —
and the crate that *acts* on them is separate. `RigidBody` and `Collider` are
authored in the editor, serialised in the scene and shown in the inspector,
exactly like `Sprite`. `voltra-physics` owns `Contact`, `PhysicsClock` and the
four steps, and nothing that a scene file contains.

## Components

Both are plain components in the ECS, `Serialize`/`Deserialize`, added to
`ComponentRegistry::with_defaults` so a scene round-trips them without any
caller opting in.

```rust
pub enum BodyType {
    /// Never moves. Walls, floors.
    Static,
    /// Moves by its velocity, ignores gravity and, later, impulses.
    Kinematic,
    /// Moves by velocity and gravity, and will be pushed by contacts.
    Dynamic,
}

pub struct RigidBody {
    pub body_type: BodyType,
    pub velocity: Vec2,
    /// `1.0 / mass`, and `0.0` for infinite mass.
    pub inverse_mass: f32,
    pub gravity_scale: f32,
    /// Fraction of velocity retained per second.
    pub linear_damping: f32,
}

pub enum Collider {
    /// Axis-aligned, half-width and half-height, centred on the transform.
    Aabb { half_extents: Vec2 },
    Circle { radius: f32 },
}
```

All three body types from the start rather than "static is `inverse_mass == 0`".
Unity, Godot and Box2D all landed on exactly these three, and kinematic is one
more match arm in the integrator — the general case here costs the same as the
special one. Conflating static with infinite mass also loses the distinction the
day a kinematic platform needs infinite mass *and* motion.

`Collider` is deliberately not `Transform`-aware: it is a shape at the origin,
and where it sits comes from the entity's `Transform`. An AABB does not rotate,
which is a real limitation and is stated in the docs rather than hidden — a
rotated sprite's box stays axis-aligned, and OBB/polygon support is 11b-3's.

## The fixed timestep

`voltra-physics` owns an accumulator; `voltra-core` drives it once per frame.

```rust
pub struct PhysicsClock { accumulator: f32, step: f32, max_steps: u32 }
impl PhysicsClock {
    pub fn steps(&mut self, delta: f32) -> u32;
}
```

`step` defaults to `1.0 / 60.0`. `max_steps` defaults to 8 and exists to stop
the spiral of death: if a frame took long enough to owe more steps than that,
running them all makes the next frame slower still, which owes more steps again.
Past the cap the debt is dropped and simulation time runs slow — which is what
every engine chooses, because the alternative is a hang.

`Clock` already clamps a single frame's delta to 0.25 s and its doc comment
already names physics integration as the reason. The two guards stack: the clamp
bounds one frame, the cap bounds the work that frame can demand.

## The step

```rust
pub fn step(world: &mut World, gravity: Vec2, dt: f32) -> Vec<Contact>;
```

1. **Integrate.** For each `(RigidBody, Transform)`:
   - `Static` — untouched.
   - `Kinematic` — `translation += velocity · dt`. No gravity.
   - `Dynamic` — `velocity += gravity · gravity_scale · dt`, then damping, then
     `translation += velocity · dt`. Velocity before position: semi-implicit.
2. **Broad phase.** Candidate pairs by world-space AABB overlap.
3. **Narrow phase.** Exact test per pair, producing a `Contact` or nothing.

```rust
pub struct Contact {
    pub a: Entity,
    pub b: Entity,
    /// Unit vector pushing `a` away from `b`.
    pub normal: Vec2,
    /// How deep the overlap is, along the normal. Always positive.
    pub penetration: f32,
    /// A point on the overlap, in world space. For drawing, and for the
    /// solver's torque arm once there is one.
    pub point: Vec2,
}
```

Returned rather than stored: nothing consumes contacts yet except the debug
draw, and a `Contacts` resource with no reader would be a structure designed for
an imagined caller.

### The broad phase is O(n²), on purpose and with a stated limit

Every pair, rejected by world-space AABB overlap before the exact test. For the
tens of bodies a scene has today this beats a spatial hash, which pays for
bucketing before it saves anything.

It is behind `candidate_pairs(world) -> Vec<(Entity, Entity)>`, so replacing it
is one function and no caller changes. **The trigger for replacing it is written
down here so it is not a judgement call later: past roughly 200 bodies,** where
n² is 20 000 pair tests per step at 60 Hz. A sweep-and-prune on the x axis is
the replacement, because the world is 2D and wide.

Pairs are emitted with `a.index() < b.index()`, so a pair is generated once and
`(a,b)` and `(b,a)` cannot both appear.

## Narrow phase

Three pairs, none of which needs SAT or GJK:

- **Circle–circle.** Overlap when the centre distance is under the radius sum.
  Normal is the centre difference, normalised; penetration is the remainder.
  Concentric circles have no defined normal — the degenerate case is resolved to
  `Vec2::X` rather than producing a NaN that propagates into the solver.
- **AABB–AABB.** Overlap on both axes. The normal is the axis of *least*
  penetration, which is the direction that separates them soonest and the one
  every engine picks.
- **AABB–circle.** Closest point on the box to the centre, then a circle test
  against that point. The centre being inside the box is its own case: the
  closest point is then the centre itself and gives a zero-length normal, so the
  nearest face is used instead.

Scale is honoured: `half_extents` and `radius` are multiplied by the transform's
scale, because a scaled sprite with an unscaled collider is a bug that looks
like a physics bug. A non-uniform scale on a circle takes the larger axis and
says so in the docs — a true ellipse is a different shape, not a parameter.

## Debug draw

`voltra_physics::debug::draw(world, contacts, &mut LineBatch)`, in green for
shapes and magenta for contacts, so the last stage's line pipeline is what makes
this stage inspectable. An AABB is four segments, a circle is a 24-segment
polyline, a contact is a short segment along its normal from its point.

The editor gets a `Physics` menu toggle. Off by default: a scene full of green
outlines is not what you want while placing sprites.

That the contact list is *drawn* is what makes the split at 11b-2 safe. A wrong
normal is invisible in a number and obvious as a line pointing the wrong way.

## Files

**Created in `voltra-scene`** — the components, beside the ones already there:

| File | Concept |
| --- | --- |
| `body.rs` | `BodyType`, `RigidBody` |
| `collider.rs` | `Collider`, and its world-space AABB given a `Transform` |

**Created as the new crate `voltra-physics`** — the simulation, which cannot be
described without becoming a second responsibility inside an existing crate:

| File | Concept |
| --- | --- |
| `clock.rs` | `PhysicsClock` — the accumulator and its cap |
| `integrate.rs` | The integration step |
| `broad.rs` | `candidate_pairs` |
| `narrow.rs` | `Contact` and the three shape pairs |
| `step.rs` | `step`, which is the three above in order |
| `debug.rs` | Shapes and contacts as line segments |

It depends on `voltra-ecs` and `voltra-scene`, and reaches `glam` through
`voltra_render::glam` like every other crate. It does **not** depend on
`voltra-assets`, and nothing depends on it except `voltra-core` and
`voltra-editor` — so the cycle above cannot reappear.

**Modified**: `voltra-scene`'s `ComponentRegistry::with_defaults` registers both
components; `voltra-core` drives the clock and the step; `voltra-editor` gets
the toggle; the root manifest gains the member.

## Errors and edge cases, decided now

- **A body with no collider** integrates and never collides. Legitimate: a
  camera rig, a particle.
- **A collider with no body** is static geometry. It participates in detection
  and never moves.
- **Zero or negative extents** produce no contacts rather than inverted normals.
  Rejected at detection, not at construction — a scene file can contain
  anything, and refusing to load it over a bad radius is worse than not
  colliding.
- **`inverse_mass` on a `Static` body** is ignored, not asserted. The scene file
  is external input.
- **Two bodies at exactly the same position** give a degenerate normal, resolved
  to `Vec2::X` with the penetration set to the full overlap.
- **A frame that owes more than `max_steps`** drops the debt and logs nothing —
  logging per frame during a stall makes the stall worse.
- **NaN in a transform** is not defended against. It cannot arise from anything
  here, and guarding every read is a cost paid on every body forever.

## Tests

Everything in this stage is pure arithmetic on plain data, so it is all unit
tested in the file it lives in. No GPU, and no headless test needed.

- **Integration**: a dynamic body falls; a static one does not; a kinematic one
  moves but ignores gravity; damping reduces speed and never reverses it; the
  velocity update precedes the position update, checked by comparing one step
  against the closed form of each ordering.
- **Clock**: exactly one step at exactly the step duration; two at twice; none
  below it; the remainder carries; the cap bounds a huge delta and the
  accumulator does not keep growing after it.
- **Broad phase**: distant bodies produce no pair; touching AABBs do; each pair
  appears once, never mirrored; a body without a collider is never in a pair.
- **Narrow phase**: overlapping circles give a normal along the centre line and
  the right penetration; concentric circles give a unit normal rather than NaN;
  AABBs give the axis of least penetration; a circle inside a box picks the
  nearest face; touching-but-not-overlapping produces nothing.
- **Scale**: a collider on a scaled transform collides at the scaled size.

## Out of scope, stated so it is not quietly added

No solver — nothing is pushed apart, and a body will sink through a floor. No
restitution, no friction, no rotation of colliders, no angular velocity, no
torque. No triggers, no layers, no masks, no raycasts, no continuous collision.
No sleeping. No joints. No polygon or capsule shapes. No spatial hash.
