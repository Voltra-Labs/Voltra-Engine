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

## Queries

`voltra_physics` answers ray casts, shape casts and point queries against
the world's colliders, filtered by `CollisionLayers`.

Physics ▸ Show colliders draws every shape and contact in the editor.
