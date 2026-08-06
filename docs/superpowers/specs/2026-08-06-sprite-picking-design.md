# Sprite picking and draw order

Date: 2026-08-06
Branch: `feature/sprite-picking`
Status: approved, not yet implemented

This is 2D, per the scope section at the top of CLAUDE.md. Nothing here leaves
room for 3D: picking in 3D is a ray cast rather than a point-in-quad test, and
sorting in 3D is a depth test rather than an ordered draw. Both are separate
subsystems for later, not parameters of this one.

## Problem

Two problems, and the second is not the one that prompted this.

**The visible one.** An entity can only be selected from the Hierarchy list.
Clicking it in the viewport does nothing. `Camera2D::viewport_to_world` was built
for exactly this and still has no caller.

**The one picking uncovers.** Draw order is currently the iteration order of
`World::query2`, which is sparse-set storage order. That order is not stable:
`SparseSet` fills the hole left by a removal with its last element, so despawning
one entity can silently reorder two unrelated overlapping sprites. With no depth
buffer, draw order is the *only* thing deciding what covers what, and alpha
blending is order-dependent — so this is already a rendering bug. It is invisible
only because nothing in the demo scene overlaps.

Picking cannot be built without confronting it. "Which sprite did I click" and
"which sprite is on top" have to be the same question, or the click selects
something other than what the user sees.

## Prior art

Checked, not recalled.

- **Unity** — `SpriteRenderer.sortingOrder`, an `int`. Sorting layer, then order
  within the layer. In 2D it has no relation to the transform's Z.
- **Godot** — `CanvasItem.z_index`, an `int` clamped to ±4096. Same idea.
- **Bevy** — `bevy_sprite`'s picking backend tests **opaque pixels** rather than
  the quad, and sorts hits by a depth read from the transform.
- **Godot again**, for the case a sort key cannot settle: a viewport button,
  "Show list of selectable nodes at position clicked" (Alt+R), lists every
  candidate under the cursor.

Two adopted, one rejected:

- **Adopted — an integer sort key.** Unity and Godot both use an `int`, not a
  float. Exact comparison, and no tie whose outcome depends on float
  representation.
- **Adopted — topmost wins, silently.** Godot's Alt+R list exists because a sort
  key is sometimes not enough, but it is an addition to topmost-wins, not a
  replacement.
- **Rejected — pixel-accurate hit-testing.** Bevy can do it because every sprite
  carries its own texture and therefore its own alpha. Here `Sprite` holds a
  colour and the renderer binds one texture for the whole batch, so there is no
  per-sprite alpha to test. A quad test is not an approximation of the right
  answer; it *is* the right answer until sprites get their own textures.

## Design

### 1. `Sprite` gains `sort_order: i32`, defaulting to `0`

Not `z_index`, which is Godot's name for it. CLAUDE.md's 2D section forbids
naming a 2D concept after an axis it is not, and this is the exact case it cites:
`z_index` is a sorting key with no relation to any Z, and when a real Z exists
the collision is permanent. Unity's `sortingOrder` is the better model.

The doc comment states it directly: a sorting key, not a coordinate. Higher draws
later, and therefore on top.

### 2. `SpriteBatch::from_world` sorts before it batches

It currently walks `query2` and pushes as it goes. It becomes: collect, sort,
push. The key is the pair `(sort_order, entity.index())`.

The second half of that key carries its weight. Without it, sprites sharing a
`sort_order` fall back to storage order and the instability above survives
untouched — the change would look complete while fixing nothing for the common
case where everything sits on layer 0. `Entity::index` is stable for as long as
the entity lives, so despawning an unrelated third entity cannot reorder a pair.

The cost is one `Vec` and one sort per frame. `from_world` already allocates two
`Vec`s per frame, so this is not a new class of cost, and the batch is the right
place to pay it. The alternative — keeping the ECS storage sorted — is a much
larger decision that nothing else is asking for.

### 3. `voltra-scene` gains the hit test

New module `crates/voltra-scene/src/pick.rs`:

```rust
/// The topmost sprite whose quad contains `point`, in world space.
pub fn sprite_at(world: &World, point: Vec2) -> Option<Entity>;
```

`voltra-scene` is, per ARCHITECTURE.md, the only crate that knows about both
entities and geometry, and mapping a world point to an entity is precisely that.
It belongs nowhere else: `voltra-ecs` must not learn what a quad is, and
`voltra-render` must not learn what an entity is.

The test is short because the geometry is already the right shape. Every sprite
is the same unit quad centred on the origin, so rather than building an oriented
bounding box, invert the transform, carry the world point into the sprite's local
space, and compare against `[-0.5, 0.5]` on each axis. Rotation and non-uniform
scale come out exact, with no second code path.

Among hits, the winner is the maximum by the same `(sort_order, entity.index())`
key the batch sorts by. One definition used twice — if the two ever diverge, the
click selects something other than what is on top.

**A degenerate case handled when the code is written, not after.** A `Transform`
with zero scale on either axis has a singular matrix. `Mat3::inverse` does not
panic on one; it returns infinities and `NaN`, and every comparison against `NaN`
is false. A collapsed sprite would therefore be either unpickable or pickable
everywhere, depending on how the comparison is written. The determinant is
checked and a non-invertible transform is skipped, so a sprite with no area
cannot be clicked — which is also the intuitive answer.

### 4. The editor routes the click

New module `crates/voltra-editor/src/picking.rs`.

The scene image's sense widens from `Sense::drag()` to `Sense::click_and_drag()`.
On `Response::clicked()`, the pointer becomes viewport-local by subtracting
`response.rect.min`, then world-space through `Camera2D::viewport_to_world`, then
an entity through `sprite_at`. Clicking empty space clears the selection, which
is what Unity, Unreal, Godot and Blender all do.

Its own module rather than more code in `panels/viewport.rs`, for the reason that
file was split out in the first place: `viewport.rs` is layout and delegates what
the pointer means. Picking will also grow — marquee selection and a Godot-style
list of candidates under the cursor are both obvious next steps.

### 5. Nothing changes in `voltra-render`

Worth stating because it is easy to assume otherwise. The renderer draws the
vertex buffer in the order it is given, so sorting on the CPU before upload is
the entire mechanism. No depth buffer, no pipeline change, no shader change —
and per CLAUDE.md, no depth attachment added "ready for 3D".

## Testing

All headless. The risk is arithmetic and ordering; neither needs a GPU or a
window.

For `sprite_at`:

- An empty world returns `None`, and a click outside every sprite returns `None`.
- A point inside a sprite returns that sprite rather than a neighbour.
- **Rotation.** A point inside the axis-aligned bounding box of a 45°-rotated
  sprite but outside the rotated quad must miss. This is the one test that
  separates a correct implementation from an AABB approximation, so it is written
  first.
- **Non-uniform scale.** A point inside on the wide axis and outside on the
  narrow one misses.
- **Overlap.** Two sprites covering the same point: the higher `sort_order` wins,
  regardless of which was spawned first.
- **Tie.** Equal `sort_order`: the higher entity index wins, and the answer does
  not change when an unrelated third entity is despawned.
- **Zero scale.** A sprite scaled to zero on one axis is never returned, and the
  call produces no `NaN`.

For `SpriteBatch::from_world`:

- Sprites are emitted in `(sort_order, entity.index())` order, asserted against
  the vertex buffer rather than an internal list, because the vertex buffer is
  what the GPU receives.
- Despawning an unrelated entity does not change the relative order of two
  others. This is the regression test for the bug that prompted the field.

The wiring in `picking.rs` is thin glue over egui and is checked by running the
editor. All of its arithmetic lives in the two functions above.

## Out of scope

- Multi-selection. `Editor::selected` stays `Option<Entity>`. Nothing asks for
  more yet, and when gizmos do, the compiler will name every site.
- Marquee selection, and a Godot-style Alt+click list of overlapping candidates.
- Pixel-accurate hit-testing — meaningless until sprites carry their own
  textures.
- Sorting layers as a second axis above `sort_order`. Unity has both; one is
  enough until something needs two.
- Anything 3D.
