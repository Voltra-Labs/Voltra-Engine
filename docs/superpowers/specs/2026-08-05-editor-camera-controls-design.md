# Editor camera controls

Date: 2026-08-05
Branch: `feature/editor-camera-controls`
Status: approved, not yet implemented

## Problem

The scene camera has two owners and neither of them is the editor alone.

`App::update` in `crates/voltra-core/src/app.rs` drives the camera from `WASD`,
the scroll wheel and `R`. `panels.rs` drives the same camera from a middle-drag
in the viewport. That split causes four defects:

1. **The platform layer decides how a camera flies.** `voltra-core` owns the
   event loop and the window; it has no business knowing what a pan gesture is.
   ARCHITECTURE.md principle 2 says lower layers never know about higher ones,
   and "how the editor navigates a scene" is the editor's concern. A shipped
   game — which takes the no-UI path through `App` — inherits editor controls it
   never asked for.
2. **Zoom is not anchored to the cursor.** `camera.zoom *= ZOOM_STEP.powf(scroll)`
   scales about the camera's centre, so the thing under the pointer slides away
   as you zoom. Anchored zoom is the expected gesture in every 2D editor.
3. **Zoom is unbounded.** Nothing clamps it. Enough scrolling reaches denormals
   at one end and overflow at the other, and `half_extents` divides by `zoom`.
4. **Input is not scoped to the viewport.** Scrolling over an empty part of the
   hierarchy panel zooms the scene, because `App` reads the wheel before knowing
   where the pointer was.

Underneath all four is one missing primitive: there is no screen-to-world
mapping anywhere in the codebase. `pan()` in `panels.rs` open-codes half of one.
Anchored zoom needs the whole thing, and so will picking, gizmos and a grid.

## Prior art

Checked rather than recalled, per the rule in CLAUDE.md.

- **Unity** puts the scene camera in `Editor/Mono/SceneView/SceneView.cs` in the
  `UnityEditor` assembly; `SceneView` extends `EditorWindow`. The runtime
  `Camera` has no navigation of its own.
- **Unreal** navigates through `FEditorViewportClient`, an editor-module type.
- **Godot** handles 2D navigation in `CanvasItemEditor`, an editor plugin, not in
  `Camera2D`.
- **Bevy** ships no camera controller in `bevy_render`; they are third-party or
  editor plugins.

The pattern is unanimous: **the render layer exposes a camera, the tool decides
how it moves.** That is the change this spec makes.

Two details worth copying from Godot's implementation:

- `EditorZoomWidget::set_zoom` clamps: `CLAMP(p_zoom, min_zoom, max_zoom)`. We
  have the clamp nowhere.
- Its zoom steps are multiplicative — `Math::pow(2.f, index / 12.f)` — so a notch
  feels the same at any magnification. Our existing `ZOOM_STEP.powf(scroll)`
  already has this property and is kept.

Godot's 2D editor exposes roughly `1/32`..`16`. Our `zoom` means something
different (a scale factor where `1.0` shows two world units vertically, not a
pixel ratio), so the numbers do not transfer, but the shape — a bounded range of
about four decades — does.

## Design

### 1. `Camera2D` gains the screen-to-world primitive

In `crates/voltra-render/src/camera.rs`. Pure maths, no new dependencies, fully
testable without a GPU.

```rust
impl Camera2D {
    /// World point under a viewport pixel. `point` is viewport-local and
    /// Y-down, which is what every UI toolkit reports.
    pub fn viewport_to_world(&self, point: Vec2, viewport: Vec2) -> Vec2;

    /// The inverse. Needed by gizmos and by the round-trip test.
    pub fn world_to_viewport(&self, world: Vec2, viewport: Vec2) -> Vec2;

    /// Scales `zoom` by `factor` while keeping the world point currently under
    /// `anchor` under `anchor` afterwards.
    pub fn zoom_around(&mut self, anchor: Vec2, viewport: Vec2, factor: f32);

    /// Clamped assignment. The only way to change zoom from outside.
    pub fn set_zoom(&mut self, zoom: f32);
}

pub const MIN_ZOOM: f32 = 1.0 / 128.0;
pub const MAX_ZOOM: f32 = 128.0;
```

`zoom_around` lives here and not in the editor because it is the only layer that
knows the projection. Written anywhere else it is written from a guess.

`zoom` becomes private with a getter, so the clamp cannot be bypassed. `position`
stays public — it has no invariant to protect.

Robustness requirements, each pinned by a test:

- `set_zoom` clamps to `[MIN_ZOOM, MAX_ZOOM]` and rejects `NaN` by leaving the
  previous value in place. Zoom can never be zero, negative, `NaN` or infinite,
  because `half_extents` divides by it.
- A viewport of zero width or height must not divide by zero. Guard with
  `.max(1.0)` on each axis, as `pan` already does.

### 2. The editor is split into one file per panel

`panels.rs` is 216 lines holding four unrelated concepts — the menu bar, the
hierarchy, the inspector and the viewport — which CONVENTIONS.md already forbids
("one concept per file; if a file needs *and* to describe it, split it"). Adding
camera navigation to it makes a fifth. The split happens here, before the new
code lands, not after:

```
crates/voltra-editor/src/
  main.rs            window config, app wiring
  editor.rs          the Editor struct: shared selection, frame layout
  panels/
    menu_bar.rs
    hierarchy.rs
    inspector.rs
    viewport.rs      the scene image and its Response
  camera.rs          ViewportCamera — navigation state and bindings
```

`panels.rs` becomes `panels/` alongside a `panels.rs` that only declares the
submodules, per the `foo.rs` + `foo/` preference in CONVENTIONS.md. The moves are
pure — no behaviour changes in the same commit as the split, so the diff stays
reviewable.

`camera.rs` sits beside `panels/` rather than inside it: navigation is not a
panel, and `viewport.rs` should read as layout only, delegating input to it.

### 3. `ViewportCamera` in the editor

New module `crates/voltra-editor/src/camera.rs`, holding whatever navigation
state outlives a frame.

```rust
pub struct ViewportCamera { /* … */ }

impl ViewportCamera {
    /// Applies one frame of navigation. `response` is the viewport image's.
    pub fn update(&mut self, ui: &Ui, response: &Response, camera: &mut Camera2D);
}
```

Bindings:

| Input | Action |
| --- | --- |
| Middle-drag | Pan |
| Scroll wheel | Zoom about the cursor |
| `W` `A` `S` `D` | Pan, only while the viewport is hovered |
| `R` | Reset position and zoom |

Input scoping is delegated to egui rather than reimplemented, which is what
fixes defect 4 for free:

- `InputState::smooth_scroll_delta` is documented as being zeroed by whichever
  `ScrollArea` consumed it, so a wheel event spent on the hierarchy cannot also
  zoom the scene.
- Keyboard reaches us only when no widget holds focus, so `WASD` typed into a
  future name field will not fly the camera.
- `Response::hover_pos` gives the anchor for zoom in viewport-local points
  already, so no coordinate plumbing is invented.

The existing `pan()` free function moves into this module unchanged in
behaviour.

### 4. `voltra-core::App` stops touching the camera

Delete the camera block from `App::update`, along with the `PAN_SPEED` and
`ZOOM_STEP` constants and the now-unused imports.

`Input` stays exactly as it is: public, re-exported, still fed by window events
and still covered by its own tests. It is what a game will read. What goes away
is `voltra-core` having an opinion about what those inputs mean.

**Stated consequence:** with no UI callback installed there is now no camera
movement at all. That is correct — a game moves its own camera — but it is a
behaviour change, not an oversight. No `App::with_update` hook is added, because
no code would use it yet (ARCHITECTURE.md principle 4).

### 5. Documentation

- ARCHITECTURE.md gains a "Decisions" entry: who owns the editor camera, the
  prior art above, and why input scoping is egui's job.
- README.md's camera-controls table is updated — it currently documents the
  controls this spec removes.

## Testing

The maths carries the risk and all of it is headless:

- `viewport_to_world` ∘ `world_to_viewport` round-trips to within `1e-5` at
  several zooms, aspects and viewport sizes.
- The centre pixel maps to `camera.position`; the corners map to
  `position ± half_extents`, with Y flipped.
- `zoom_around` leaves the anchor's world point fixed — asserted for an off-centre
  anchor, which is the only case that can distinguish it from centred zoom.
- `zoom_around` at the clamp boundary does not move the camera, so a user
  scrolling at maximum zoom does not drift sideways.
- `set_zoom` clamps both ends and ignores `NaN`.
- A zero-sized viewport returns a finite result.

`ViewportCamera` is thin glue over egui and is verified by running the editor
detached and reading the log, as CLAUDE.md prescribes.

## Out of scope

Click-to-select picking, gizmos, a grid overlay, and framing the selection with
`F`. All of them want `viewport_to_world`, which is why it is built here, but
each is its own change.

`egui_backend.rs` is 633 lines and holds four concepts — buffer growth, texture
deltas, the pipeline and the pass. It breaches the same convention as `panels.rs`
and should be split into `egui_backend/`, but nothing in this spec touches it, so
it is left for its own `refactor/` branch.
