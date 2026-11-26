# 9. Effects and animation

Particles, drawn lines and shapes, state machines that crossfade clips
and blend them in trees, and characters made of bones. Every one
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

## State machines and crossfades

An **`Animator`** chooses which clip an entity's `AnimationPlayer` plays.
It runs a state machine, a `*.animator.ron` file: parameters the game sets
(`set_float`, `set_bool`, `trigger`), states that each play a clip, and
transitions between them. Give a transition a `duration` and the animator
crossfades instead of cutting: for that many seconds the clip leaving keeps
playing, the new one starts, and each property is written with their mix.

```ron
transitions: [
    (from: Some("idle"), to: "run", conditions: [Greater("speed", 0.1)], duration: 0.15),
    (from: Some("run"), to: "idle", conditions: [Less("speed", 0.1)], duration: 0.2),
    (from: Some("idle"), to: "fidget", exit_time: Some(3.0)),
],
```

Positions, scales, colours and intensities mix in proportion, rotations
the short way round, and a sprite frame switches half way. A property only
one of the two clips animates keeps that clip's value. A transition that
fires mid-fade fades on from what is showing, so nothing jumps. No
`duration` is `0`: a cut, as before.

`exit_time` is how much of the state's clip must play first: below `1.0` a
point in each loop (`0.5`, half way through any loop), from `1.0` up a
number of loops of a clip that loops — the `fidget` above waits for three
idles. On a clip that does not loop, `1.0` and above wait for its end; a
clip of no length (one key) never makes a transition wait.

The inspector lists a machine's transitions under the animator, each with
its fade in seconds; **Save** writes those durations into the file —
only them: a transition the editor ignored for a typo stays in the file
for you to fix (written as plain RON, without its comments) — and
**Revert** reads it back. Every entity running the machine
fades the new way at once.

## Blend trees

A state can play a **blend tree** instead of one clip: several clips at
once, each weighted by where a float parameter puts the game among them.
A `Blend1D` places clips along one parameter — between two thresholds the
two clips there mix in proportion, past either end the end clip plays:

```ron
(name: "move", tree: Blend1D(parameter: "speed", children: [
    (threshold: 0.0, clip: Path("animations/hero/idle.anim.ron")),
    (threshold: 1.5, clip: Path("animations/hero/walk.anim.ron")),
    (threshold: 3.5, clip: Path("animations/hero/run.anim.ron")),
])),
```

A `Blend2D` places them at points over two parameters, `x` and `y` — a
stick's direction, say, with an idle at `(0.0, 0.0)` and a walk each way
at `(1.0, 0.0)`, `(0.0, 1.0)` and so on. The game's point mixes the three
clips of the triangle around it; outside every triangle, the two of the
nearest edge. The weights follow the parameters every frame, with no
transition.

A tree's clips play **in step**: each is the same fraction through its own
cycle, and the tree's length is their weighted average, so a walk sliding
into a run keeps its stride. A state's `exit_time` counts the tree's loops.
Transitions into and out of a tree state crossfade as any other does.

A tree whose parameter is not a float, or not there, reads `0` and says so
in the log; an empty tree plays nothing. The inspector shows each tree
under **blend trees**: pick its parameters, drag thresholds and points,
drop a clip from the browser onto a row to change it or onto **add** to
add one, **Clear** a row to take its clip out — **Save** writes the trees
with the fades. `crates/voltra-core/examples/character.rs` blends idle,
walk and run by speed on the ground and crossfades to a jump or a fall
(`assets/animations/hero.animator.ron`).

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
