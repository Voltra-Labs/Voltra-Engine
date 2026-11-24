# 9. Effects and animation

Particles, drawn lines and shapes, and characters made of bones. Every one
of them is a component, edited in the inspector, saved in the scene file and
drawn in painter's order with the sprites (`sort_order`, sorting layers,
lights).

## Particles

Scene ▸ Spawn particles makes a `ParticleEmitter`. Its inspector is laid
out in the order an effect is built:

- **A preset** — smoke, sparks, rain — to start from. It replaces the
  motion and the look, and keeps the texture and where it draws.
- **Rate and motion**: particles a second, lifetime, speed, direction and
  spread, acceleration (gravity, wind).
- **The emission shape**: a `Point`, a `Circle` or a `Ring`, a `Box`, an
  `Edge` (a line along the emitter's x), or the `Outline` of the entity's
  own `Collider` — sparks off the rim of whatever it is.
- **The look over a life**: a texture; a size, and a size curve and a speed
  curve multiplying it over the particle's life; start and end colours, or
  a gradient; an alpha curve; a rotation and a spin.
- **A flipbook**: frames of an atlas played over each particle's life
  (`OverLife { cycles }`) or at a rate (`PerSecond { fps }`) — flames,
  puffs, sparkles from one sheet. Leave the frame list empty for all of
  them, or type the ones to play: `0 1 2 5`.
- Simulation (on the CPU, or the GPU for big emitters), collisions and
  sorting.

Curves are graphs: drag a key, double-click to add one, right-click to
remove it; the selected key's time, value and ease are underneath. A
gradient bar works the same way with colour stops. Each drag is one undo
entry.

While editing, the selected emitter plays in the viewport — and only that
one, so a level full of fountains stays still while you tune one. The
preview is not an edit: nothing is saved by it. **Restart** and **Burst**
fire it again.

## Lines, polygons and trails

Scene ▸ Spawn line, Spawn polygon and Spawn trail make one of each:

- **`Line2D`** — a stroke through points (in the entity's space), open or
  `closed`: a `width` and a `width_curve` along it, a colour, joints
  (`Miter`, `Bevel`, `Round`), caps at the ends (`Butt`, `Square`,
  `Round`), and a texture stretched along it or tiled. A laser, a rope, a
  route on a map.
- **`Polygon2D`** — a filled outline, concave or not, with a colour and a
  texture stretched over it. Ground, water, a shadow.
- **`Trail2D`** — follows its entity while the game runs, a point dropped
  every `min_distance`, each gone after `lifetime`: a width curve and a
  gradient by age, fading to nothing when they are not set. A sword's
  swing, a comet's tail.

**Edit points** in a line's or a polygon's inspector puts handles on the
points in the viewport: drag one, press the middle of a side to add one
there, right-click one to remove it. `Esc` ends it.

## Bones

A skeleton is entities: a root marked **`Skeleton2D`**, and under it in the
hierarchy entities with a **`Bone2D`** — each bone's own `Transform` is its
pose relative to its parent, pointing along its x, and `length` is how far
its tip is. Scene ▸ Spawn skeleton makes a root and one bone; a bone's
inspector has **Add child bone** (at its tip), **Set rest** (the current
pose becomes the rest pose) and **Reset to rest**; the skeleton's has
**Reset pose**. Bones are drawn over the art, and a click on one selects it;
pose it with the rotate tool (`E`).

**Cut-out characters** hang sprites on bones: parent a sprite to a bone and
it follows. **Animate** a skeleton as anything else — an `AnimationPlayer`
on the root, its clip keying the bones' rotations by name in the Timeline.
**`TwoBoneIk`** on a lower bone (a shin, a forearm) bends it and its parent
to reach a `target`, named by path from the skeleton root, `flip` choosing
which way the joint bends; the inspector says why when a limb cannot be
solved.

The sample is a cut-out walker, its legs swung by a clip and its arm
reaching by IK:

```sh
cargo run -p voltra-player -- assets/scenes/bones.ron
```

## Skinned sprites

Hinged cut-out parts show their joints. A **`SkinnedSprite`** is one
picture bent by several bones: a mesh whose vertices each follow up to four
bones by weight. Scene ▸ Spawn skinned sprite makes one; under a skeleton,
in its inspector:

1. Pick the texture, and **Use the skeleton's bones**.
2. **Generate mesh**: the picture's outline traced and cut into triangles,
   each vertex weighted to its nearest bones.
3. **Paint weights** where the automatic ones are wrong: a drag in the
   viewport adds the chosen bone's weight under a soft brush, Shift takes
   it away; the dots show that bone's weight from blue to red. **Auto
   weights** starts over.

The mesh and its weights are saved in the scene, and the GPU bends it every
frame — in a browser too.
