# Stage 11b-2 — The contact solver

Bodies detect contacts today and do nothing about them: a ball sinks through a
floor, and `step.rs` has a test pinning exactly that. This stage resolves them.

The solver was chosen in 11b-1 and recorded in ARCHITECTURE.md so it would not
be re-argued: **TGS Soft**, the solver Erin Catto took for Box2D v3 after
comparing eight of them in [Solver2D](https://box2d.org/posts/2024/02/solver2d/).
XPBD, the obvious alternative, lost on friction and on precision far from the
origin. This spec is how that solver lands here.

## Scope

Delivered:

- Contacts resolved: a body rests on a floor instead of passing through it.
- Stacks that settle rather than jitter or sink — warm starting is what buys
  this, and it is why the solver has to hold state between steps.
- Friction and restitution, from a new `PhysicsMaterial` component.
- Sub-stepping inside the existing fixed step, with the relax and restitution
  passes TGS Soft needs.

Not in this stage, each with the reason:

- **Rotation.** No angular velocity, no inertia, no torque arms. `Collider::Aabb`
  is axis-aligned in *world* space, so a body that spun would still collide as an
  upright box — simulation and screen would disagree. Rotation arrives in 11b-3
  together with oriented shapes, where the collider turns with the body.
- **Two-point manifolds.** One contact point per pair is enough while there is no
  torque: a second point exists to resist rotation. It arrives with rotation.
- **Sleeping.** Bodies at rest keep being solved. Correct, just wasteful, and the
  island tracking it needs is its own subsystem.
- **Continuous collision.** A fast body still tunnels. Speculative contacts
  (below) narrow the window; they are not CCD.
- **Joints, sensors, collision events, collision layers.** Each is a stage.

## What is already here

`voltra-physics` has `PhysicsClock` (fixed step, accumulator, 8-step cap),
`integrate` (semi-implicit Euler over the ECS), `candidate_pairs` (O(n²) broad
phase, pairs emitted once as `(a, b)` with `a` the lower entity), `contact`
(circle/AABB narrow phase whose normal pushes `a` away from `b`), and `debug`
(collider outlines and contact normals through the line pipeline).

`step(world, gravity, dt)` integrates, then collects contacts, then returns them.
`voltra-core::App` owns a `PhysicsClock` and calls it once per owed step.

`voltra-scene` owns the components: `RigidBody` (`body_type`, `velocity`,
`inverse_mass`, `gravity_scale`, `linear_damping`) and `Collider`
(`Circle { radius }`, `Aabb { half_extents }`).

## The algorithm

One fixed step of `dt`, sub-divided into `N` sub-steps of `h = dt / N`. Verified
against the Box2D v3 source (`src/contact_solver.c`, `src/solver.h`,
`src/types.c`), not from memory.

```
collide once            contacts from the positions at the start of the step
prepare                 effective masses, base separation, softness, warm-start
                        impulses from the cache, relative normal velocity
warm start              apply the cached impulses to the velocities
for each sub-step:
    integrate velocities    gravity and damping, h
    solve contacts          use_bias = true   (soft constraint pushes apart)
    integrate positions     h, accumulating a per-body delta position
relax                   solve contacts again, use_bias = false, and do NOT
                        integrate positions afterwards
restitution             one pass, using the relative normal velocity captured
                        in prepare
store                   impulses to the cache; velocities and positions to ECS
```

**Collide once per step, not per sub-step.** Box2D computes the manifold before
solving and holds the normal constant through the sub-steps, tracking separation
analytically from the accumulated delta positions:
`s = base_separation + dot(delta_b - delta_a, normal)`. Re-colliding per sub-step
costs N narrow-phase passes and gains nothing at these speeds.

**Relaxation is why the soft constraint does not add energy.** The biased solve
injects velocity to push bodies apart; the second solve, with the bias off,
takes that extra energy back out of the velocities and the accumulated impulses.
Positions are deliberately not integrated from the relaxed velocities.

**Friction is solved in the relax pass**, with `use_bias == false`, which is what
the current Box2D source does — friction applied alongside the biased normal
solve would be scaled by the separation push.

### Soft constraint coefficients

`b2MakeSoft`, verbatim:

```
omega         = 2π · hertz
a1            = 2ζ + h·omega
a2            = h·omega·a1
a3            = 1 / (1 + a2)
bias_rate     = omega / a1
mass_scale    = a2 · a3
impulse_scale = a3
```

Two sets per step, as Box2D and Avian both keep: one for dynamic-versus-dynamic
contacts and a stiffer one for contacts against something immovable, so a body
is not pushed through the ground.

```
contact_hertz = min(30, 0.125 / h)      # Nyquist: well under the sub-step rate
dynamic       = soft(contact_hertz,     damping_ratio, h)
static        = soft(2 · contact_hertz, damping_ratio, h)
```

### The normal solve

```
s = base_separation + dot(delta_b - delta_a, normal)

if s > 0:                       # speculative: still separated this sub-step
    bias = s / h
    mass_scale, impulse_scale = 1, 0
else if use_bias:
    bias = max(mass_scale · bias_rate · s, -max_push_speed)
    mass_scale, impulse_scale = softness.mass_scale, softness.impulse_scale
else:
    bias = 0
    mass_scale, impulse_scale = 1, 0

vn      = dot(v_b - v_a, normal)
impulse = -normal_mass · (mass_scale · vn + bias) - impulse_scale · accumulated
new     = max(accumulated + impulse, 0)     # clamp the TOTAL, never the increment
applied = new - accumulated
```

Clamping the accumulated impulse rather than the increment is what lets a later
iteration *reduce* an earlier one; clamping the increment is a classic jitter
source.

The speculative branch matters even though the narrow phase only reports actual
overlaps: a contact prepared as overlapping can separate part-way through the
sub-steps, and without the branch the solver keeps pushing.

### Friction and restitution

```
vt      = dot(v_b - v_a, tangent)            # tangent = perp(normal)
impulse = -tangent_mass · vt
max     = friction · accumulated_normal_impulse
new     = clamp(accumulated_tangent + impulse, -max, max)
```

Restitution runs once, after relaxation, and only where it can do anything:

```
skip unless relative_velocity < -threshold and total_normal_impulse > 0
impulse = -normal_mass · (vn + restitution · relative_velocity)
```

`relative_velocity` is the approach speed captured in prepare, before any
impulse was applied. The threshold — 1 m/s in Box2D — is what stops a resting
body from bouncing on its own numerical noise forever.

## Solver bodies

The sub-step loop needs a per-body accumulated delta position, which no
component holds, and `voltra-ecs` cannot hand out two mutable components at
once. Both problems have the same answer, and it is the one every engine uses:
gather a dense array of solver bodies at the start of the step and scatter the
results back at the end.

```rust
struct SolverBody {
    entity: Entity,
    velocity: Vec2,
    delta_position: Vec2,   // accumulated across this step's sub-steps
    inverse_mass: f32,      // 0 for anything that cannot be pushed
    body_type: BodyType,
    gravity_scale: f32,
    linear_damping: f32,
}
```

Every entity with a `Collider` **or** a `RigidBody` gets one. Static geometry — a
collider with no body, which the current tests already rely on — enters with
`inverse_mass = 0` and zero velocity, so contacts against it need no special
case and Box2D's dummy-body branch does not have to exist here. Kinematic bodies
integrate their own velocity but take `inverse_mass = 0`, which is precisely
"moves as told, immovable by contacts".

Scatter writes `velocity` back to the `RigidBody` and adds `delta_position` to
the `Transform`. An entity with no `Transform` still simulates and moves nothing
visible, as it does today.

## Warm starting, and where the state lives

Warm starting feeds the previous step's impulses into this one. It is the single
biggest quality difference in the whole solver — without it a stack of boxes
sinks under its own weight and jitters — so it decides the shape of the API:
`step` cannot stay a pure function.

How the others hold it:

- **Box2D v3** keeps the impulses on the manifold points inside the persistent
  contact object owned by the world, and matches points across steps by contact
  feature ID.
- **Godot** keeps a `GodotBodyPair2D` per overlapping pair. A new contact
  inherits `acc_normal_impulse` / `acc_tangent_impulse` from a previous contact
  within `contact_recycle_radius` in each body's local space, and
  `_validate_contacts` erases any contact not touched this frame.
- **Avian** keys the constraint by the `ContactId` of the contact-graph edge and
  stores `warm_start_normal_impulse` on the manifold point.

All three agree on the shape: **the persistent unit is the pair, it is owned by
the physics world, and an entry dies when the pair stops being reported.** The
per-point matching that distinguishes them exists because their manifolds have
several points; ours has one.

So:

```rust
pub struct PhysicsWorld {
    clock: PhysicsClock,
    params: SolverParams,
    impulses: HashMap<(Entity, Entity), CachedImpulse>,
    contacts: Vec<Contact>,
}
```

keyed by the ordered pair the broad phase already emits — `(a, b)` with `a` the
lower entity, never mirrored. Entries not touched during a step are evicted, so
a despawned body, a reloaded scene and a pair that simply separated all clean up
by the same rule rather than three. `Entity` carries a generation, so a recycled
index cannot inherit a dead entity's impulses.

When 11b-3 gives manifolds a second point, the key gains a point identifier —
that is exactly the problem Box2D's feature IDs and Godot's recycle radius
solve, and the note belongs in the code so the next stage does not rediscover it.

`PhysicsWorld` is also where sleeping, joints and collision events will live, so
introducing it now is not scaffolding for an imagined caller — it is the crate's
missing owner, and 11b-1 deferred it only because nothing yet needed state.

## Materials

`friction` and `restitution` exist nowhere today. They go in a new component:

```rust
pub struct PhysicsMaterial { pub friction: f32, pub restitution: f32 }
```

in `voltra-scene`, registered in `ComponentRegistry::with_defaults`, defaulting
to Box2D's `friction: 0.6, restitution: 0.0` when an entity has none.

Not on `Collider`, which is an enum: two fields per variant duplicates them
across every shape and grows with each shape added. Not on `RigidBody`, which
would leave static geometry — a collider with no body — with no surface at all.
A separate component is Unity's `PhysicsMaterial2D` adapted to composition, and
it is the only option of the three that is not already wrong for cases the
current tests cover.

Mixing follows Box2D: `friction = sqrt(fa · fb)`, `restitution = max(ra, rb)`.
Both are clamped where they are read, because a scene file is external input:
friction below zero would reverse the friction impulse, restitution above one
would add energy on every bounce until the body leaves the world.

## Parameters

On `SolverParams`, with these defaults, all from Box2D v3's `b2DefaultWorldDef`:

| Parameter | Default | Why |
| --- | --- | --- |
| `sub_steps` | 4 | Box2D's default; the primary iteration count in Solver2D's comparison |
| `contact_hertz` | 30 Hz | Clamped to `0.125 / h`; Nyquist, well under the sub-step rate |
| `contact_damping_ratio` | 10 | Heavily damped: contacts should not ring |
| `max_push_speed` | 3 m/s | Caps how fast overlap is resolved, so deep overlap does not launch bodies |
| `restitution_threshold` | 1 m/s | Below this, no bounce — stops resting jitter |
| `warm_starting` | on | A switch because turning it off is how its effect gets demonstrated |

Every one is a field rather than a constant, because a scene that stacks and a
scene that shoots projectiles want different values and the engine will need the
parameter either way.

## Files

`voltra-physics` gains a solver directory rather than a fatter `step.rs`:

```
crates/voltra-physics/src/
  world.rs          PhysicsWorld: clock, params, impulse cache, last contacts
  step.rs           one fixed step: collide, prepare, sub-step loop, scatter
  solver.rs         + solver/
    body.rs         SolverBody, gather from the ECS and scatter back
    softness.rs     Softness and the coefficients above
    constraint.rs   ContactConstraint: masses, base separation, accumulators
    cache.rs        ImpulseCache: warm-start lookup and non-touched eviction
    contact.rs      the three passes: solve, relax, restitution
  integrate.rs      split into velocity and position over solver bodies
```

`voltra-scene` gains `material.rs` and one registry line. `voltra-core::App`
swaps its `PhysicsClock` field for a `PhysicsWorld` and keeps `gravity` and
`with_physics` exactly as they are.

## Tests

Behaviour, not implementation:

- A body released above a floor comes to rest **on** it, and stays within a
  small tolerance for hundreds of steps afterwards.
- A stack of five boxes settles, each above the one below, none sunk into it.
- Restitution `1.0` returns a dropped ball to nearly its drop height;
  restitution `0.0` leaves it on the ground.
- A box sliding along a floor stops under friction `0.6`, and keeps sliding
  under friction `0.0`.
- A static body and a kinematic body are not moved by a dynamic body landing on
  them; the dynamic body still stops.
- A mass ratio of 1000:1 does not explode.
- The impulse cache does not grow when bodies separate or are despawned.
- Concentric and coincident shapes stay finite — no `NaN` reaches a velocity.
- A resting scene reports the same contacts on consecutive steps.

**An existing test is inverted, deliberately**:
`step::tests::a_falling_body_keeps_going_through_the_floor` pins 11b-1's stated
limit. It is replaced by the resting test above, which is the same scene with the
opposite assertion.

## Verification

`cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, and the editor launched detached with a scene of
stacked boxes to confirm on screen what the tests assert in numbers.
