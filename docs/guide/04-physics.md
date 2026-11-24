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

## Queries

`voltra_physics` answers ray casts, shape casts and point queries against
the world's colliders, filtered by `CollisionLayers`.

Physics ▸ Show colliders draws every shape and contact in the editor.
