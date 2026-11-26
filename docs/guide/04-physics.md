# 4. Physics

Three components make a body:

- **`Collider`** — its shape: `Box`, `Circle`, `Capsule` or a convex
  `Polygon` (Inspector ▸ Collider ▸ Fit to sprite traces one).
- **`RigidBody`** — how it moves: `Static` (walls), `Kinematic` (moved by
  its velocity, pushes but is not pushed), `Dynamic` (gravity, mass,
  impulses). `bullet: true` keeps a fast body from tunnelling.
- **`PhysicsMaterial`** — `friction` and `restitution` (bounce).

A collider with no body is static. `Sensor` makes a collider detect
without pushing: pickups, triggers.

## Contacts

`tick.events` in `fixed_update` lists every pair that began, stayed or
stopped touching this step:

```rust
for event in tick.events {
    if event.touch == Touch::Began {
        if let Some(other) = event.other(player) {
            if tick.world.get::<Coin>(other).is_some() {
                tick.world.despawn(other);
            }
        }
    }
}
```

`games/breakout` is a whole game on this: a ball with no gravity and a
bounce of one, bricks taken on `Began`.

## Characters

`CharacterBody2D` moves a kinematic body by sliding along what it hits,
with a floor, slopes and steps — what a platformer's walker wants instead
of a dynamic body tipping over. `OneWay` makes a platform passable from
below. `games/platformer` uses both.

## Tiles

A `Tilemap` layer with `collision` on is solid: the engine builds static
colliders from its tiles, and rebuilds them as they are painted. Each
frame of the layer's atlas can say how its tiles collide, in the layer's
`tile_data` — edited in the inspector's "Tile N" section for the tile
picked in the palette:

- `Full` (the default): the whole cell; neighbouring full tiles merge into
  one box.
- `Empty` ("None" in the inspector): walked through — a ladder, a
  doorway, a background tile.
- `Half(Bottom)` (or `Top`, `Left`, `Right`): half the cell.
- `Slope(left: 0.0, right: 1.0)`: a floor from one height to another, in
  fractions of the cell — `(0, 1)` is 45°, `(0, 0.5)` then `(0.5, 1)` a
  gentle slope over two tiles.
- `Polygon([...])`: any outline in the cell, colliding as its convex hull.

A shape turns and mirrors with its tile, so one slope frame mirrored is
the slope down. A `CharacterBody2D` walks up and down slopes as it walks on
the flat.

A frame can also carry values a game reads — `custom: {"ladder": true,
"damage": 5}` — by cell with `Tilemap::custom_at`, or by a point in the
world:

```rust
use voltra_scene::tilemap::query::custom_under;

let on_ladder = custom_under(tick.world, position, "ladder")
    .is_some_and(|(_layer, _cell, value)| value.truthy());
```

The platformer's level has both: a hill of slope tiles, and a ladder of
`Empty` tiles marked `ladder` that `games/platformer/src/ladder.rs` climbs.
In the scene file it reads:

```ron
tile_data: {0: (shape: Slope(left: 0.0, right: 0.5)), 1: (shape: Slope(left: 0.5, right: 1.0)),
            3: (shape: Empty, custom: {"ladder": true})},
```

## Joints

A `Joint2D` holds two bodies together, or one to the world. It lives on
any entity: `a` is the first body (none: the entity itself), `b` the
second (none: the world, and `anchor_b` is then a point in the world).
Physics ▸ Join selection hinges the selected bodies; the inspector's Joint
section picks the kind and edits every part of it.

- **`Revolute`** — a hinge. Optional `limit`, `motor` and `spring` (a
  `RestSpring` turning it back to `rest` radians: a door that closes
  itself, a flipper).
- **`Distance`** — a rod, a rope (`min` 0), or a spring about `length`.
- **`Weld`** — the two held as one.
- **`Wheel`** — put it on the wheel, `b` the chassis: the wheel's anchor
  stays on a line along `axis` (in the chassis's space), sprung by a
  suspension `RestSpring`, and turns freely; a `motor` drives the car,
  and a motor at speed zero is a brake.
- **`Prismatic`** — a slider: `a` moves only along `axis` (in `b`'s
  space), its angle held; `limit`, a `LinearMotor` and a `spring` along
  the axis. A lift, a piston, a drawer.
- **`Mouse`** — `a`'s anchor pulled to the target, `anchor_b`, by a
  spring no stronger than `max_force`. Move the target to drag the body.

A car's front wheel, in the scene file:

```ron
"Joint2D": (b: Some("…chassis id…"), anchor_b: (0.7, -0.6),
            kind: Wheel(axis: (0.0, 1.0),
                        spring: Some((hertz: 4.0, damping_ratio: 0.7)),
                        motor: Some((speed: -10.0, max_torque: 20.0))))
```

While the scene plays in the editor, dragging a dynamic body (anywhere
the gizmo is not) pulls it with a mouse joint, through the solver: it
swings, collides and drags what it is joined to.

**Breaking.** `break_force` (newtons) and `break_torque` (newton-metres)
on any joint — off by default — break it when the load passes them: the
`Joint2D` is removed and `tick.broken_joints` reports it once, as
`tick.events` reports a touch:

```rust
for broken in tick.broken_joints {
    // The joint is already gone; the plank it held now falls.
    log::info!("a joint on {:?} broke under {} N", broken.entity, broken.force);
}
```

A bridge of planks with `break_force` set gives way under a heavy enough
crate. Stopping play in the editor puts every broken joint back.

## Queries

`voltra_physics` answers ray casts, shape casts and point queries against
the world's colliders, filtered by `CollisionLayers`.

Physics ▸ Show colliders draws every shape and contact in the editor.
