# Translate gizmo and the line pipeline — design

Date: 2026-08-10
Status: approved
Stage: 11a of 11a/11b

## What this is, and what it is not

Stage 11 was "gizmos and physics". Those are two subsystems with different
interfaces and different failure modes, so they are split the way stage 12 was:

| | Delivers | Visible |
| --- | --- | --- |
| **11a**, this spec | A line pipeline in `voltra-render`, and a translate gizmo in the editor | Yes |
| **11b** | Physics: bodies, integration, collision | Later, its own spec |

Physics is not designed here and not scaffolded here. It appears in this
document exactly twice: as the second consumer that justifies the line pipeline
being in `voltra-render` rather than in the editor, and as the reason the pass
that draws lines does not clear.

## The state it starts from

- `pipeline::create_flat_color` is the only pipeline (`pipeline.rs:11`). Group 0
  is the camera, group 1 a texture.
- `pass::draw_mesh` and `pass::draw_mesh_batches` both begin their pass with
  `LoadOp::Clear` (`pass.rs:39`, `pass.rs:98`). There is no pass that draws on
  top of what is already in a target.
- `Mesh` and `Vertex` are position + colour + uv, `u32` indices (`mesh.rs`).
- The editor already picks a sprite by clicking: `picking::sprite_at`, and
  `Editor::selected` holds an `Option<Entity>`.
- `voltra-editor/src/camera.rs` owns the viewport camera and the screen↔world
  conversion the gizmo needs.
- Everything the engine draws is a textured quad. Nothing draws a line.

## How the established editors do it

**The tool model.** Unity has separate Move, Rotate and Scale tools; the gizmo
drawn depends on which tool is active, and the tool is persistent editor state.
Blender has no persistent tool: `G`/`R`/`S` start a modal transform that the
mouse drives until a click confirms or `Esc` cancels, and changing the letter
mid-gesture carries the accumulated movement over. Godot 2D follows Unity, and
has [an open proposal](https://github.com/godotengine/godot-proposals/issues/1215)
to add Blender's.

**Unity's model is what this takes.** The persistent tool is discoverable — the
gizmo on screen tells you what the next drag will do — and it composes with the
click-to-select that already exists. Blender's is faster once learned but is
invisible until someone tells you the letter, and it needs modal input capture
that `Input` does not have. That capture is a real subsystem: it has to swallow
every key while active, survive a lost window focus, and unwind cleanly on
`Esc`. It can be added later on top of a working gizmo; the reverse is harder.

**Line width.** WebGPU has no line width, and neither does `wgpu` 30: verified
against the vendored source, `PrimitiveState` (`wgpu-types-30.0.0/src/render.rs:385`)
has `topology`, `strip_index_format`, `front_face`, `cull_mode`,
`unclipped_depth`, `polygon_mode` and `conservative` — and nothing else.
`PolygonMode::Line` is wireframe fill, not width, and needs an optional feature.
`PrimitiveTopology::LineList` therefore draws one-pixel hairlines, full stop.

Every engine works around this the same way: expand each segment into a quad
whose width is applied in screen space by the vertex shader. Bevy did it by
adopting `bevy_polyline` into `bevy_gizmos`
([#8427](https://github.com/bevyengine/bevy/pull/8427)). That is what "line
pipeline" means below.

## Decisions

### The line pipeline lives in `voltra-render` and knows nothing of gizmos

`voltra_render::lines`, beside `mesh`. It draws *line segments with a width in
pixels*. It has never heard of a gizmo, a handle or an axis.

That boundary is the whole reason this is not five lines of `egui::Painter` in
the editor. The gizmo is the first consumer; the viewport grid and 11b's
collision-shape overlay are the second and third, and neither can reach an
`egui::Painter` — they need to be in the scene, under the same camera, drawn
into the same target.

### Width is applied in the vertex shader, in pixels

A segment is uploaded as its two world-space endpoints, a width in **logical
pixels**, and a colour. The vertex shader projects both endpoints, takes the
perpendicular of the screen-space direction, and offsets by half the width.

Pixels rather than world units, and on the GPU rather than the CPU, for one
reason: a gizmo must be the same size on screen at every zoom. Doing it on the
CPU means every caller recomputes pixels-per-world-unit from the camera and the
viewport size, and gets it subtly wrong somewhere. Doing it once in the shader
means a caller says "two pixels" and is done.

The shader needs the viewport's pixel size to convert. That arrives in its own
small uniform at **group 1** — the slot the sprite pipeline uses for its
texture — so group 0 stays the camera for every pipeline in the engine.

#### Rejected

- **`PrimitiveTopology::LineList`.** One pixel, always. Invisible on a high-DPI
  display and impossible to hit-test generously.
- **Expanding on the CPU.** Cheap for the twenty segments a gizmo needs, and it
  puts screen-space knowledge into every call site. The grid would repeat it,
  and 11b would repeat it again.
- **Drawing the gizmo with `egui::Painter`.** No new GPU code at all, and the
  gizmo would live in the UI layer rather than the scene — unreachable for a
  game, unreachable for the grid, and drawn in a different coordinate system
  from the thing it is manipulating.

### The line pass loads, it does not clear

`pass::draw_lines` begins with `LoadOp::Load`. Every existing pass clears,
because until now exactly one thing was drawn into a target per frame. An
overlay is by definition the second thing.

This is the piece 11b needs unchanged: a collision-shape overlay is another
load-don't-clear pass over the same target.

### The gizmo is editor state, in its own module directory

`crates/voltra-editor/src/gizmo/`, not a crate. It reads the editor camera, the
selection, `Input`, and `Transform` from the world — it cannot be described
without naming four other modules, which is the repo's test for "not yet its own
crate".

`Tool` is persistent editor state with one variant today:

```rust
pub enum Tool {
    /// Click to select, drag a handle to move. The only tool in 11a.
    Translate,
}
```

One variant rather than none, because the alternative is a `bool` that gets
renamed when Rotate arrives. `W` selects it; `E` and `R` are not bound, so they
do nothing rather than pretending.

### Hit-testing is in screen space, and the gizmo wins over the sprite

A click is tested against the gizmo handles **before** `picking::sprite_at`. A
handle drawn on top of a sprite must be grabbable, or the gizmo is unusable
exactly when the sprite fills the screen.

Handles are tested in screen space, against the same pixel geometry that was
drawn, with a grab margin wider than the line. Testing in world space is the
Unreal bug report this avoids: the drawn size is constant on screen but the
tested size scales with zoom, so the handle stops matching its own picture.

Order within the gizmo: centre square first, then the axis arrows. The square is
the smallest target and sits where the two arrows meet, so testing it last would
make it unreachable.

### A drag moves by delta, from a grab offset

On press, the gizmo stores the world position of the cursor and the entity's
translation. Each move sets the translation to `start + (cursor - grab)`,
constrained to the axis for an axis handle.

Not "set the translation to the cursor": that teleports the sprite so its origin
jumps to wherever the cursor grabbed, which every editor avoids and every
first implementation gets wrong.

A drag that begins is finished by the mouse release, wherever it happens —
including outside the viewport. The gizmo keeps its own `Option<Drag>` rather
than asking whether the cursor is still over a handle.

## Files

**Created**

| File | Concept |
| --- | --- |
| `crates/voltra-render/src/lines.rs` | `LineVertex`, `LineBatch` — segments with a pixel width, and their buffers |
| `crates/voltra-render/src/shaders/lines.wgsl` | The expansion, and nothing else |
| `crates/voltra-editor/src/gizmo.rs` | `Gizmo` — the state, and what a frame does to it |
| `crates/voltra-editor/src/gizmo/handle.rs` | `Handle` — which part of the gizmo, its screen geometry, its hit test |
| `crates/voltra-editor/src/gizmo/drag.rs` | `Drag` — a grab in progress, and the translation it produces |
| `crates/voltra-editor/src/tool.rs` | `Tool` |
| `crates/voltra-render/tests/headless_lines.rs` | Pixels: a line is drawn, at the right width, over what was there |

**Modified**

| File | Change |
| --- | --- |
| `crates/voltra-render/src/pipeline.rs` | `create_lines` |
| `crates/voltra-render/src/pass.rs` | `draw_lines`, loading rather than clearing |
| `crates/voltra-render/src/shader.rs` | `LINES` |
| `crates/voltra-render/src/lib.rs` | the module and its re-exports |
| `crates/voltra-render/src/renderer.rs` | owns the line pipeline and the viewport uniform |
| `crates/voltra-core/src/app/draw.rs` | records the overlay pass after the scene |
| `crates/voltra-core/src/app/ui_frame.rs` | the seam a panel uses to submit lines for this frame |
| `crates/voltra-editor/src/editor.rs` | holds `Tool` and `Gizmo` |
| `crates/voltra-editor/src/panels/viewport.rs` | drives the gizmo from the viewport's rect |

## Data flow

```
viewport panel, laying out
  │
  ├─ gizmo.update(input, camera, viewport_rect, selection, world)
  │    ├─ no drag, press?   → hit-test handles in screen space
  │    │                       hit  → begin Drag { handle, grab, start }
  │    │                       miss → fall through to picking::sprite_at
  │    ├─ dragging, moved?  → transform.translation = start + delta, axis-constrained
  │    └─ dragging, release?→ end the drag
  │
  └─ gizmo.lines(camera, viewport_rect, selection, world) → Vec<LineSegment>
       │
       └─ UiFrame::draw_lines(segments)   ← the seam; the panel never touches wgpu
            │
            └─ App::redraw: scene pass (clears) → line pass (loads) → egui
```

The gizmo produces segments and reads the world. It never touches a device, a
queue or a pipeline, and `voltra-render` never learns what an axis is.

## Errors and edge cases, decided now rather than discovered later

- **Nothing selected** — no lines, no hit test. `update` returns immediately.
- **The selection is despawned mid-drag** — the drag ends. The `Drag` holds an
  `Entity`, and a missing `Transform` ends it rather than panicking.
- **The viewport is resized mid-drag** — the grab is stored in *world* space, so
  the sprite does not jump. Only the screen-space hit geometry is rebuilt.
- **Zero-area viewport** — a minimised window gives a rect of zero width. The
  screen↔world conversion divides by it, so `update` returns early and `lines`
  returns empty.
- **A degenerate segment** — both endpoints equal. Its screen-space direction is
  undefined, so the shader would emit NaNs and the GPU would draw a black hole.
  The batch drops zero-length segments on push.
- **Two entities at the same position** — irrelevant here: the gizmo follows the
  selection, and the selection is one entity.

## Tests

**No GPU.** `handle.rs` and `drag.rs` are pure geometry and pure arithmetic, so
they are unit-tested where they live:

- a click on the X arrow hits the X handle, one pixel outside the margin misses
- the centre square wins over an arrow where they overlap
- a drag on the X handle changes only x, however the cursor moves
- a drag from a grab offset does not teleport the entity to the cursor
- the translation after a drag equals `start + (cursor - grab)`, exactly
- a drag whose entity lost its `Transform` ends instead of panicking

**GPU**, `crates/voltra-render/tests/headless_lines.rs`, skipping without an
adapter as the others do:

- a horizontal segment of width 5 paints roughly five rows of pixels, and a
  segment of width 1 paints fewer — the width reaches the screen
- the same segment paints the same pixel count at two different camera zooms —
  the width is in pixels, not world units
- the line pass does not erase what the pass before it drew
- a zero-length segment draws nothing and does not produce NaNs

## Out of scope, stated so it is not quietly added

No rotate handle, no scale handle, no `E`/`R` bindings. No multi-selection, so
no pivot rules. No grid snapping, no numeric entry, no undo — undo is its own
decision and is not made cheaper by being bolted onto a drag. No viewport grid,
though the line pipeline is what it will use. No physics, no collision shapes,
no debug draw beyond what the gizmo itself emits.
