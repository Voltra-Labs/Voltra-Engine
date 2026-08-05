# Viewport Camera Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move scene navigation out of the platform layer into the editor, and give `Camera2D` the screen-to-world primitive that anchored zoom, picking and gizmos all need.

**Architecture:** `voltra-render::Camera2D` gains pure-maths viewport↔world conversion plus a clamped `zoom`. `voltra-editor` gains a `ViewportCamera` that reads egui's already-scoped input and drives the camera. `voltra-core::App` stops touching the camera at all. Along the way `panels.rs` is split one-concept-per-file, because the new code would otherwise be its fifth concept.

**Tech Stack:** Rust, `glam` 0.31 (via `voltra_render::glam`), `egui` 0.35 (via `voltra_core::egui`), `wgpu` 30.

Spec: [docs/superpowers/specs/2026-08-05-editor-camera-controls-design.md](../specs/2026-08-05-editor-camera-controls-design.md)

## Global Constraints

- Branch is `feature/viewport-camera`, already created off `main`. Never commit to `main`.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` must all pass before any commit. Clippy warnings are errors.
- No `unwrap()` outside `#[cfg(test)]`. Use `expect("why this cannot fail")` when the invariant is real.
- Log through `log`, never `println!`.
- Only `voltra-core` may depend on `winit`; only `voltra-render` may depend on `wgpu`. If a change makes `voltra-render` import `winit` or `egui`, it is wrong.
- No new entries in any member crate's `[dependencies]` with a literal version. This plan adds no dependencies at all.
- Unit tests live in the file they test, inside `#[cfg(test)] mod tests`.
- Conventional Commits, scope = crate minus the `voltra-` prefix, imperative subject ≤50 chars.
- Every commit message ends with the trailer `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- The editor is a GUI app with an infinite loop. Never run `cargo run -p voltra-editor` in the foreground — launch it detached, wait a few seconds, read the log, kill it.

### Coordinate conventions used throughout

These are the definitions every task depends on. Get them wrong and everything type-checks and misbehaves.

- **Viewport-local points**: origin at the viewport image's top-left corner, `+x` right, **`+y` down**, measured in egui *points* (not physical pixels). This is what egui reports.
- **World units**: `+y` **up**. `Camera2D::position` is the world point at the centre of the viewport.
- At `zoom == 1.0` the camera shows two world units vertically, so `half_extents().y == 1.0 / zoom`.
- `Camera2D::aspect` is assumed equal to `viewport.x / viewport.y`. `App` already keeps it so (`renderer.camera.aspect = target.aspect()`). Points and physical pixels differ by a scale factor that cancels in a ratio, so the points-vs-pixels mismatch is not a bug.

---

### Task 1: Viewport↔world conversion and a clamped zoom

Adds the primitive. `zoom` stays a public field for now — Task 5 privatises it, once nothing outside the crate writes to it.

**Files:**
- Modify: `crates/voltra-render/src/camera.rs` (add to `impl Camera2D`, add consts, extend `mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces, all as associated items on `Camera2D` — so they are written `Camera2D::MIN_ZOOM`, not `camera::MIN_ZOOM`:
  - `pub const MIN_ZOOM: f32 = 1.0 / 128.0;` and `pub const MAX_ZOOM: f32 = 128.0;`, inside the `impl Camera2D` block
  - `pub fn viewport_to_world(&self, point: Vec2, viewport: Vec2) -> Vec2`
  - `pub fn world_to_viewport(&self, world: Vec2, viewport: Vec2) -> Vec2`
  - `pub fn set_zoom(&mut self, zoom: f32)`
  - `pub fn zoom_around(&mut self, anchor: Vec2, viewport: Vec2, factor: f32)`
  - All `Vec2` are `glam::Vec2`.

- [ ] **Step 1: Write the failing tests**

Append these to the existing `mod tests` at the bottom of `crates/voltra-render/src/camera.rs`. Do not remove the tests already there.

```rust
    /// A viewport that is not square and not a round number, so an axis mix-up
    /// cannot pass by coincidence.
    fn viewport() -> Vec2 {
        Vec2::new(800.0, 600.0)
    }

    fn camera_for(viewport: Vec2) -> Camera2D {
        Camera2D::new(Vec2::new(3.0, -2.0), 1.5, viewport.x / viewport.y)
    }

    #[test]
    fn viewport_centre_is_the_camera_position() {
        let viewport = viewport();
        let camera = camera_for(viewport);
        let world = camera.viewport_to_world(viewport * 0.5, viewport);
        assert!(
            (world - camera.position).length() < 1e-5,
            "centre mapped to {world}, expected {}",
            camera.position
        );
    }

    #[test]
    fn viewport_top_left_is_up_and_to_the_left() {
        let viewport = viewport();
        let camera = camera_for(viewport);
        let half = camera.half_extents();
        // Top-left in a Y-down viewport is minimum x and *maximum* y in world
        // space. Getting this flip wrong is the classic silent bug.
        let expected = camera.position + Vec2::new(-half.x, half.y);
        let world = camera.viewport_to_world(Vec2::ZERO, viewport);
        assert!((world - expected).length() < 1e-5, "got {world}, want {expected}");
    }

    #[test]
    fn viewport_and_world_round_trip() {
        let viewport = viewport();
        for zoom in [Camera2D::MIN_ZOOM, 0.25, 1.0, 7.5, Camera2D::MAX_ZOOM] {
            let camera = Camera2D::new(Vec2::new(-4.0, 9.0), zoom, viewport.x / viewport.y);
            for point in [
                Vec2::ZERO,
                viewport,
                viewport * 0.5,
                Vec2::new(123.0, 456.0),
            ] {
                let back = camera.world_to_viewport(
                    camera.viewport_to_world(point, viewport),
                    viewport,
                );
                // A twentieth of a point, not a thousandth. `world_to_viewport`
                // subtracts two world coordinates of magnitude ~9 to get a
                // difference of ~0.008 at MAX_ZOOM, then divides by an equally
                // small half-extent. f32 carries about seven digits, so that
                // cancellation costs real precision — the observed error at the
                // ceiling is ~0.006 points. This is inherent to f32 world
                // coordinates, not to the algorithm, and a twentieth of a pixel
                // is far below anything visible.
                assert!(
                    (back - point).length() < 0.05,
                    "zoom {zoom}: {point} round-tripped to {back}"
                );
            }
        }
    }

    #[test]
    fn zoom_around_keeps_the_anchor_world_point_still() {
        let viewport = viewport();
        let mut camera = camera_for(viewport);
        // Off-centre on both axes: a centred zoom would pass a centred anchor.
        let anchor = Vec2::new(150.0, 500.0);
        let before = camera.viewport_to_world(anchor, viewport);

        camera.zoom_around(anchor, viewport, 1.7);

        let after = camera.viewport_to_world(anchor, viewport);
        assert!(
            (after - before).length() < 1e-4,
            "anchor drifted from {before} to {after}"
        );
        assert!(camera.zoom > 1.5, "zoom did not increase: {}", camera.zoom);
    }

    #[test]
    fn set_zoom_clamps_both_ends() {
        let mut camera = Camera2D::default();
        camera.set_zoom(f32::INFINITY);
        assert_eq!(camera.zoom, Camera2D::MAX_ZOOM);
        camera.set_zoom(0.0);
        assert_eq!(camera.zoom, Camera2D::MIN_ZOOM);
        camera.set_zoom(-5.0);
        assert_eq!(camera.zoom, Camera2D::MIN_ZOOM);
    }

    #[test]
    fn set_zoom_ignores_nan() {
        let mut camera = Camera2D::default();
        camera.set_zoom(2.0);
        camera.set_zoom(f32::NAN);
        assert_eq!(camera.zoom, 2.0, "NaN must leave the previous zoom alone");
    }

    #[test]
    fn zoom_around_at_the_clamp_does_not_drift() {
        let viewport = viewport();
        let mut camera = Camera2D::new(Vec2::ZERO, Camera2D::MAX_ZOOM, viewport.x / viewport.y);
        let anchor = Vec2::new(10.0, 20.0);

        camera.zoom_around(anchor, viewport, 2.0);

        // Zoom was already at the ceiling, so nothing may move. Without this
        // the camera slides sideways every notch once the user hits the limit.
        assert_eq!(camera.zoom, Camera2D::MAX_ZOOM);
        assert!(camera.position.length() < 1e-6, "drifted to {}", camera.position);
    }

    #[test]
    fn a_degenerate_viewport_stays_finite() {
        let camera = Camera2D::default();
        let world = camera.viewport_to_world(Vec2::ZERO, Vec2::ZERO);
        assert!(world.is_finite(), "got {world}");
    }
```

- [ ] **Step 2: Run the tests and confirm they fail**

```sh
cargo test -p voltra-render --lib camera
```

Expected: compile error, `no function or associated item named 'viewport_to_world' found`, plus the same for `world_to_viewport`, `set_zoom`, `zoom_around`, `MIN_ZOOM` and `MAX_ZOOM`.

- [ ] **Step 3: Add the constants and the conversions**

Insert into the existing `impl Camera2D` block in `crates/voltra-render/src/camera.rs`, after `half_extents`.

```rust
    /// Floor and ceiling for [`Self::set_zoom`].
    ///
    /// Unbounded zoom is not a theoretical problem: `half_extents` divides by
    /// `zoom`, so zero produces infinities and a large enough value produces
    /// denormals. Godot clamps its editor zoom for the same reason.
    pub const MIN_ZOOM: f32 = 1.0 / 128.0;
    pub const MAX_ZOOM: f32 = 128.0;

    /// The world point under a viewport pixel.
    ///
    /// `point` is viewport-local and **Y-down**, which is what every UI toolkit
    /// reports; world space is Y-up, hence the flip on that axis alone.
    ///
    /// Assumes `self.aspect` matches `viewport`'s. `App` keeps it so; if it
    /// ever did not, the image on screen would already be stretched.
    pub fn viewport_to_world(&self, point: Vec2, viewport: Vec2) -> Vec2 {
        let viewport = Self::guard(viewport);
        let half = self.half_extents();
        // Normalised to 0..1 across the viewport, then to -1..1 about its
        // centre, then scaled by what the camera covers.
        let normalised = point / viewport;
        let centred = Vec2::new(normalised.x * 2.0 - 1.0, 1.0 - normalised.y * 2.0);
        self.position + centred * half
    }

    /// Inverse of [`Self::viewport_to_world`].
    pub fn world_to_viewport(&self, world: Vec2, viewport: Vec2) -> Vec2 {
        let viewport = Self::guard(viewport);
        let half = self.half_extents();
        let centred = (world - self.position) / half;
        let normalised = Vec2::new(centred.x * 0.5 + 0.5, 0.5 - centred.y * 0.5);
        normalised * viewport
    }

    /// Sets `zoom`, clamped to [`Self::MIN_ZOOM`]..=[`Self::MAX_ZOOM`].
    ///
    /// `NaN` leaves the previous value alone: it compares false against every
    /// bound, so `clamp` would panic and a bare assignment would poison the
    /// projection matrix for every later frame.
    pub fn set_zoom(&mut self, zoom: f32) {
        if zoom.is_nan() {
            return;
        }
        self.zoom = zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
    }

    /// Scales the zoom by `factor`, keeping the world point under `anchor`
    /// under `anchor` afterwards.
    ///
    /// Measuring the same anchor before and after is what makes this correct at
    /// the clamp too: if `set_zoom` refused the change, the two samples are
    /// equal and the camera does not move.
    pub fn zoom_around(&mut self, anchor: Vec2, viewport: Vec2, factor: f32) {
        let before = self.viewport_to_world(anchor, viewport);
        self.set_zoom(self.zoom * factor);
        let after = self.viewport_to_world(anchor, viewport);
        self.position += before - after;
    }

    /// A viewport no smaller than one point per axis.
    ///
    /// A panel dragged shut reports zero, and both conversions divide by this.
    fn guard(viewport: Vec2) -> Vec2 {
        viewport.max(Vec2::ONE)
    }
```

- [ ] **Step 4: Run the tests and confirm they pass**

```sh
cargo test -p voltra-render --lib camera
```

Expected: PASS, including the five tests that were already in the file.

- [ ] **Step 5: Check the whole workspace still builds clean**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: no warnings, all tests pass.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-render/src/camera.rs
git commit -m "feat(render): add viewport-world mapping and zoom clamp

Anchored zoom, picking and gizmos all need the same screen-to-world
conversion, and none of them can be written correctly outside the layer that
owns the projection.

set_zoom is the clamped door onto a field that half_extents divides by.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Split `panels.rs` into one file per panel

Pure move. **No behaviour changes in this commit** — that is what keeps the diff reviewable. `panels.rs` is 216 lines holding four concepts, and CONVENTIONS.md allows one.

**Files:**
- Create: `crates/voltra-editor/src/editor.rs`
- Create: `crates/voltra-editor/src/panels.rs` (replaced: becomes module declarations only)
- Create: `crates/voltra-editor/src/panels/menu_bar.rs`
- Create: `crates/voltra-editor/src/panels/hierarchy.rs`
- Create: `crates/voltra-editor/src/panels/inspector.rs`
- Create: `crates/voltra-editor/src/panels/viewport.rs`
- Modify: `crates/voltra-editor/src/main.rs:1-3`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:
  - `editor::Editor` with `pub fn ui(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>)` and `pub(crate) selected: Option<Entity>`
  - `panels::menu_bar::show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>)`
  - `panels::hierarchy::show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>)`
  - `panels::inspector::show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>)`
  - `panels::viewport::show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>)`

Free functions taking `&mut Editor` rather than methods, so each panel file is readable without the struct definition in front of you.

- [ ] **Step 1: Create `editor.rs`**

`crates/voltra-editor/src/editor.rs`:

```rust
//! The editor's shared state and its frame layout.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;

use crate::panels;

/// Editor state that outlives a frame.
///
/// egui is immediate mode and remembers nothing between frames, so anything the
/// editor still needs next frame — the selection — has to be kept here.
#[derive(Default)]
pub struct Editor {
    pub(crate) selected: Option<Entity>,
}

impl Editor {
    /// Lays out the whole editor. Called once per frame with the root `Ui`.
    pub fn ui(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>) {
        panels::menu_bar::show(self, ui, frame);
        panels::hierarchy::show(self, ui, frame);
        panels::inspector::show(self, ui, frame);
        // Last, so it takes whatever room the docked panels left rather than
        // the other way round.
        panels::viewport::show(self, ui, frame);
    }
}
```

- [ ] **Step 2: Replace `panels.rs` with module declarations**

Overwrite `crates/voltra-editor/src/panels.rs` entirely:

```rust
//! One module per docked panel. Layout order lives in [`crate::editor`].

pub mod hierarchy;
pub mod inspector;
pub mod menu_bar;
pub mod viewport;
```

- [ ] **Step 3: Create `panels/menu_bar.rs`**

```rust
//! Top menu bar: scene commands and the frame's counters.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{Sprite, Transform};

use crate::editor::Editor;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::top("menu").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Scene", |ui| {
                if ui.button("Spawn sprite").clicked() {
                    editor.selected = Some(spawn_sprite(frame));
                    ui.close();
                }
                if ui.button("Clear").clicked() {
                    let all: Vec<Entity> = frame.world.query::<Sprite>().map(|(e, _)| e).collect();
                    for entity in all {
                        frame.world.despawn(entity);
                    }
                    editor.selected = None;
                    ui.close();
                }
            });

            ui.separator();
            let (width, height) = frame.viewport_size();
            ui.label(format!("viewport {width}x{height}"));
            ui.separator();
            ui.label(format!("{} entities", frame.world.entity_count()));
        });
    });
}

/// Drops a white unit sprite at the origin, ready to be moved.
fn spawn_sprite(frame: &mut UiFrame<'_>) -> Entity {
    let entity = frame.world.spawn();
    frame
        .world
        .insert(entity, Transform::default().with_scale(Vec2::splat(0.4)));
    frame.world.insert(entity, Sprite::default());
    entity
}
```

- [ ] **Step 4: Create `panels/hierarchy.rs`**

```rust
//! Left panel: every entity in the scene, and which one is selected.

use voltra_core::egui::{self, RichText, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::Sprite;

use crate::editor::Editor;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::left("hierarchy")
        .default_size(200.0)
        .show(ui, |ui| {
            ui.heading("Hierarchy");
            ui.separator();

            // Collected before the loop: selecting inside it would hold a
            // borrow of the world while the buttons want to mutate it.
            let mut entities: Vec<Entity> = frame.world.query::<Sprite>().map(|(e, _)| e).collect();
            // Storage order is not list order. A sparse set fills the hole
            // left by a removal with its last element, so deleting one row
            // would otherwise make another jump across the list.
            entities.sort_by_key(Entity::index);

            if entities.is_empty() {
                ui.label(RichText::new("nothing in the scene").italics().weak());
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                for entity in entities {
                    let label = format!("Entity {}", entity.index());
                    if ui
                        .selectable_label(editor.selected == Some(entity), label)
                        .clicked()
                    {
                        editor.selected = Some(entity);
                    }
                }
            });
        });
}
```

- [ ] **Step 5: Create `panels/inspector.rs`**

```rust
//! Right panel: the components of the selected entity.

use voltra_core::egui::{self, Color32, DragValue, RichText, Ui};
use voltra_core::UiFrame;
use voltra_scene::{Sprite, Transform};

use crate::editor::Editor;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::right("inspector")
        .default_size(240.0)
        .show(ui, |ui| {
            ui.heading("Inspector");
            ui.separator();

            // A stale handle is normal, not a bug: the entity may have been
            // despawned since it was selected.
            let Some(entity) = editor.selected.filter(|e| frame.world.is_alive(*e)) else {
                ui.label(RichText::new("nothing selected").italics().weak());
                return;
            };

            ui.label(format!(
                "Entity {} (gen {})",
                entity.index(),
                entity.generation()
            ));
            ui.separator();

            if let Some(transform) = frame.world.get_mut::<Transform>(entity) {
                transform_ui(ui, transform);
            }
            if let Some(sprite) = frame.world.get_mut::<Sprite>(entity) {
                ui.separator();
                sprite_ui(ui, sprite);
            }

            ui.separator();
            if ui
                .button(RichText::new("Delete").color(Color32::LIGHT_RED))
                .clicked()
            {
                frame.world.despawn(entity);
                editor.selected = None;
            }
        });
}

fn transform_ui(ui: &mut Ui, transform: &mut Transform) {
    ui.label(RichText::new("Transform").strong());

    egui::Grid::new("transform").num_columns(2).show(ui, |ui| {
        ui.label("position");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut transform.translation.x).speed(0.01));
            ui.add(DragValue::new(&mut transform.translation.y).speed(0.01));
        });
        ui.end_row();

        ui.label("rotation");
        ui.drag_angle(&mut transform.rotation);
        ui.end_row();

        ui.label("scale");
        ui.horizontal(|ui| {
            ui.add(DragValue::new(&mut transform.scale.x).speed(0.01));
            ui.add(DragValue::new(&mut transform.scale.y).speed(0.01));
        });
        ui.end_row();
    });
}

fn sprite_ui(ui: &mut Ui, sprite: &mut Sprite) {
    ui.label(RichText::new("Sprite").strong());
    ui.horizontal(|ui| {
        ui.label("colour");
        ui.color_edit_button_rgba_unmultiplied(&mut sprite.color);
    });
}
```

- [ ] **Step 6: Create `panels/viewport.rs`**

Carries the existing `pan` function unchanged. Task 3 moves it out again — that is deliberate, so this commit changes no behaviour.

```rust
//! Central panel: the rendered scene, and the pointer interaction on it.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;

pub fn show(_editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::CentralPanel::default()
        // The scene brings its own background; the panel's would only show
        // as a border around the image.
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let available = ui.available_size();
            // egui lays out in logical points while the target is sized in
            // physical pixels. Missing this conversion renders the scene at
            // half resolution on a 200% display.
            let scale = ui.ctx().pixels_per_point();
            frame.request_viewport_size(
                (available.x * scale) as u32,
                (available.y * scale) as u32,
            );

            let scene = ui.add(
                egui::Image::new(egui::load::SizedTexture::new(frame.viewport(), available))
                    // Without this the image is inert decoration and the
                    // pointer never reaches the camera.
                    .sense(egui::Sense::drag()),
            );

            if scene.dragged_by(egui::PointerButton::Middle) {
                pan(frame, scene.drag_delta(), available.y);
            }
        });
}

/// Drags the scene under the pointer by `delta`, in egui points.
fn pan(frame: &mut UiFrame<'_>, delta: egui::Vec2, height_in_points: f32) {
    // One scalar rather than one per axis: the camera's aspect *is* the panel's
    // aspect, so world units per point come out the same either way. If they
    // ever differed, the image would already be stretched.
    let world_per_point = 2.0 * frame.camera.half_extents().y / height_in_points.max(1.0);

    // Grab-and-drag, so the camera travels opposite the pointer. Y is negated
    // on top of that because egui counts points downwards and the world counts
    // them up.
    frame.camera.position.x -= delta.x * world_per_point;
    frame.camera.position.y += delta.y * world_per_point;
}
```

- [ ] **Step 7: Point `main.rs` at the new module**

In `crates/voltra-editor/src/main.rs`, replace lines 1-3:

```rust
mod panels;

use panels::Editor;
```

with:

```rust
mod editor;
mod panels;

use editor::Editor;
```

Leave the rest of the file alone.

- [ ] **Step 8: Verify nothing changed but the file layout**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: no warnings, all tests pass.

Then launch the editor detached and confirm it still behaves — hierarchy lists the three demo sprites, the inspector edits them, middle-drag pans:

```sh
cargo run -p voltra-editor > editor.log 2>&1 &
sleep 8
cat editor.log
pkill -f voltra-editor
```

Expected in the log: no `ERROR` lines and no panic backtrace.

- [ ] **Step 9: Commit**

```sh
git add crates/voltra-editor/src
git commit -m "refactor(editor): split panels into one file each

panels.rs held the menu bar, the hierarchy, the inspector and the viewport,
which is four concepts past what CONVENTIONS.md allows, and camera navigation
was about to be the fifth. Move only — no behaviour changes.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: `ViewportCamera` — navigation owned by the editor

**Files:**
- Create: `crates/voltra-editor/src/camera.rs`
- Modify: `crates/voltra-editor/Cargo.toml` (add `log`)
- Modify: `crates/voltra-editor/src/panels/viewport.rs` (delete `pan`, call the controller)
- Modify: `crates/voltra-editor/src/editor.rs` (hold a `ViewportCamera`)
- Modify: `crates/voltra-editor/src/main.rs` (declare the module)

**Interfaces:**
- Consumes from Task 1: `Camera2D::{viewport_to_world, set_zoom, zoom_around, MIN_ZOOM, MAX_ZOOM}`.
- Consumes from Task 2: `panels::viewport::show`, `editor::Editor`.
- Produces:
  - `camera::ViewportCamera`, `Default`, with public fields `key_pan_speed: f32`, `zoom_per_scroll_point: f32`, `home_position: Vec2`, `home_zoom: f32`
  - `pub fn navigate(&self, ui: &Ui, response: &Response, camera: &mut Camera2D)`

Tunables are fields rather than constants so a future preferences panel has something to bind to without a rewrite.

- [ ] **Step 1: Add the `log` dependency**

`voltra-editor` currently depends only on `env_logger`, which installs a logger
but does not provide the `log::info!` macro. `crates/voltra-editor/Cargo.toml`
gains one line in `[dependencies]`, in the workspace form — never a literal
version:

```toml
log.workspace = true
```

`log = "0.4.33"` is already in the root `[workspace.dependencies]`, so nothing
is added there.

- [ ] **Step 2: Create `camera.rs`**

`crates/voltra-editor/src/camera.rs`:

```rust
//! Scene navigation: how the viewport drives the camera.
//!
//! This lives in the editor, not in `voltra-core`, because how a scene is
//! navigated is a property of the tool. Unity keeps its scene camera in the
//! `UnityEditor` assembly, Unreal in `FEditorViewportClient` and Godot in
//! `CanvasItemEditor`, all for the same reason: a shipped game moves its own
//! camera and must not inherit an editor's bindings.
//!
//! Input scoping is delegated to egui rather than reimplemented.
//! `smooth_scroll_delta` is already zeroed by whichever `ScrollArea` consumed
//! it, and keys never arrive while a widget holds focus, so neither a scroll
//! over the hierarchy nor typing into a field can reach the camera.

use voltra_core::egui::{Key, PointerButton, Response, Ui};
use voltra_render::glam::Vec2;
use voltra_render::Camera2D;

/// Bindings and tunables for navigating the scene viewport.
pub struct ViewportCamera {
    /// World units per second under keyboard pan, at `zoom == 1.0`.
    pub key_pan_speed: f32,
    /// Zoom multiplier applied per point of scroll.
    ///
    /// Multiplicative, so one notch feels the same at any magnification —
    /// the property Godot's `2^(index/12)` steps have and an additive step
    /// does not.
    pub zoom_per_scroll_point: f32,
    /// Where `R` sends the camera.
    pub home_position: Vec2,
    pub home_zoom: f32,
}

impl Default for ViewportCamera {
    fn default() -> Self {
        Self {
            key_pan_speed: 1.5,
            // A wheel notch is around 50 points, so this is ~1.1 per notch.
            zoom_per_scroll_point: 1.002,
            home_position: Vec2::ZERO,
            home_zoom: 1.0,
        }
    }
}

impl ViewportCamera {
    /// Applies one frame of navigation. `response` is the scene image's.
    pub fn navigate(&self, ui: &Ui, response: &Response, camera: &mut Camera2D) {
        let viewport = Vec2::new(response.rect.width(), response.rect.height());

        if response.dragged_by(PointerButton::Middle) {
            let delta = response.drag_delta();
            self.pan(camera, Vec2::new(delta.x, delta.y), viewport);
        }

        // `hover_pos` is in global screen points; the camera works in
        // viewport-local ones, so the panel's own corner comes off first.
        if let Some(pointer) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let local = pointer - response.rect.min;
                camera.zoom_around(
                    Vec2::new(local.x, local.y),
                    viewport,
                    self.zoom_per_scroll_point.powf(scroll),
                );
            }
        }

        if !response.hovered() {
            return;
        }

        let (dt, axis, reset) = ui.input(|i| {
            (
                // Clamped, so a stalled frame cannot teleport the camera.
                i.stable_dt.min(0.1),
                Vec2::new(
                    axis(i.key_down(Key::D), i.key_down(Key::A)),
                    axis(i.key_down(Key::W), i.key_down(Key::S)),
                ),
                i.key_pressed(Key::R),
            )
        });

        if axis != Vec2::ZERO {
            // Normalised so diagonal movement is not faster than axis-aligned,
            // and scaled by the visible height so a keypress covers the same
            // fraction of the screen at any zoom.
            let speed = self.key_pan_speed * camera.half_extents().y;
            camera.position += axis.normalize() * speed * dt;
        }

        if reset {
            camera.position = self.home_position;
            camera.set_zoom(self.home_zoom);
            log::info!("camera reset");
        }
    }

    /// Grab-and-drag: the camera travels opposite the pointer.
    fn pan(&self, camera: &mut Camera2D, delta: Vec2, viewport: Vec2) {
        // Anchoring on the world point under the pointer rather than scaling by
        // a hand-derived factor means pan and zoom cannot disagree about what a
        // point is worth.
        let centre = viewport * 0.5;
        let from = camera.viewport_to_world(centre, viewport);
        let to = camera.viewport_to_world(centre - delta, viewport);
        camera.position += to - from;
    }
}

/// `1.0`, `-1.0` or `0.0` from a pair of opposing keys.
fn axis(positive: bool, negative: bool) -> f32 {
    f32::from(positive) - f32::from(negative)
}
```

- [ ] **Step 3: Declare the module in `main.rs`**

In `crates/voltra-editor/src/main.rs`, the module list becomes:

```rust
mod camera;
mod editor;
mod panels;
```

- [ ] **Step 4: Give `Editor` the controller**

In `crates/voltra-editor/src/editor.rs`, add the import and the field:

```rust
use crate::camera::ViewportCamera;
```

```rust
#[derive(Default)]
pub struct Editor {
    pub(crate) selected: Option<Entity>,
    pub(crate) camera: ViewportCamera,
}
```

`ViewportCamera` implements `Default`, so the derive still works.

- [ ] **Step 5: Hand the viewport's `Response` to the controller**

Replace the whole body of `crates/voltra-editor/src/panels/viewport.rs` with:

```rust
//! Central panel: the rendered scene image.
//!
//! Layout only. What the pointer and keyboard do with it is
//! [`crate::camera::ViewportCamera`]'s job.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::CentralPanel::default()
        // The scene brings its own background; the panel's would only show
        // as a border around the image.
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let available = ui.available_size();
            // egui lays out in logical points while the target is sized in
            // physical pixels. Missing this conversion renders the scene at
            // half resolution on a 200% display.
            let scale = ui.ctx().pixels_per_point();
            frame.request_viewport_size(
                (available.x * scale) as u32,
                (available.y * scale) as u32,
            );

            let scene = ui.add(
                egui::Image::new(egui::load::SizedTexture::new(frame.viewport(), available))
                    // Without this the image is inert decoration and the
                    // pointer never reaches the camera.
                    .sense(egui::Sense::drag()),
            );

            editor.camera.navigate(ui, &scene, frame.camera);
        });
}
```

The `pan` free function is gone from this file — its replacement is `ViewportCamera::pan`.

- [ ] **Step 6: Build and lint**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: no warnings, all tests pass.

- [ ] **Step 7: Drive the editor and confirm each binding**

```sh
cargo run -p voltra-editor > editor.log 2>&1 &
sleep 8
cat editor.log
pkill -f voltra-editor
```

Check by hand, since this is glue over a GUI and no unit test can reach it:

1. Middle-drag inside the scene pans it, and the sprite under the pointer stays under the pointer.
2. Scrolling over the scene zooms toward the cursor — the sprite under the pointer does not slide away.
3. Scrolling over the **hierarchy** list scrolls the list and does **not** zoom the scene. This is the defect being fixed; check it explicitly.
4. `WASD` pans while the pointer is over the scene, and does nothing while it is over the inspector.
5. `R` recentres. The log shows `camera reset`.
6. Scrolling hard in one direction stops at a limit instead of collapsing the scene, and the camera does not drift sideways once it stops.

- [ ] **Step 8: Commit**

```sh
git add crates/voltra-editor/Cargo.toml crates/voltra-editor/src
git commit -m "feat(editor): own the viewport camera navigation

Pan, cursor-anchored zoom, hover-scoped WASD and reset, driven from the
viewport's own Response. Scoping is egui's: a scroll the hierarchy consumed
never reaches the camera, which it used to.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: `voltra-core::App` stops driving the camera

**Files:**
- Modify: `crates/voltra-core/src/app.rs:21-24` (delete both constants)
- Modify: `crates/voltra-core/src/app.rs:108-138` (empty out `update`)
- Modify: `crates/voltra-core/src/app.rs:1-19` (drop imports that fall unused)

**Interfaces:**
- Consumes from Task 3: nothing directly — Task 3 must land first so the bindings exist somewhere before they are removed here.
- Produces: `App::update` no longer touches `Renderer::camera`. `Input` keeps its full public surface.

- [ ] **Step 1: Reduce `update` to the clock tick**

Replace the whole of `fn update` in `crates/voltra-core/src/app.rs` — currently lines 108-138 — with:

```rust
    /// One simulation step, between the events arriving and `Input::end_frame`
    /// wiping the per-frame sets.
    ///
    /// Deliberately empty of camera work. How a scene is navigated is the
    /// editor's business, not the platform layer's; `voltra-editor` does it
    /// from the viewport panel. A game reads [`Input`] and moves its own
    /// camera.
    fn update(&mut self) {
        let _dt = self.clock.tick().as_secs_f32();
    }
```

The tick must stay: `Clock::tick` advances the frame clock, and dropping it would freeze `Clock::delta` for every later reader.

- [ ] **Step 2: Delete the two constants**

Remove lines 21-24 of `crates/voltra-core/src/app.rs`:

```rust
/// World units the camera pans per second.
const PAN_SPEED: f32 = 1.5;
/// Multiplier applied per line of scroll wheel.
const ZOOM_STEP: f32 = 1.1;
```

- [ ] **Step 3: Drop the two imports that are now unused**

`Vec2` and `KeyCode` appear in `app.rs` only at lines 114-133 — the block Step 1
deleted. Both imports go:

```rust
use voltra_render::glam::Vec2;   // line 7 — delete
use winit::keyboard::KeyCode;    // line 13 — delete
```

Keep `use voltra_render::{Camera2D, Filter, RenderTarget, Renderer};`.
`Camera2D` is still the type of `UiFrame::camera` and the other three are still
used by the redraw path.

Confirm with:

```sh
cargo clippy -p voltra-core --all-targets -- -D warnings
```

Expected: clean. If it reports an unused import you did not remove, remove
exactly that one; if it reports `cannot find type` you removed one too many.

- [ ] **Step 4: Confirm the build and the tests**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: no warnings, all tests pass. `input.rs`'s tests must still pass untouched — `Input` was not changed.

- [ ] **Step 5: Confirm the editor is unaffected**

```sh
cargo run -p voltra-editor > editor.log 2>&1 &
sleep 8
cat editor.log
pkill -f voltra-editor
```

Expected: navigation still works exactly as at the end of Task 3, because it now comes from the editor. No `ERROR` lines.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-core/src/app.rs
git commit -m "refactor(core): stop flying the camera from App

The platform layer owned the event loop and also decided what a pan gesture
was, so a game taking the no-UI path inherited editor bindings. Input stays;
the opinion about what those inputs mean goes to the editor.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Close the zoom invariant

Now that nothing outside `voltra-render` assigns to `zoom`, the field becomes private and `set_zoom` becomes the only way in. Until Task 4 landed this was impossible — `App::update` wrote it directly.

**Files:**
- Modify: `crates/voltra-render/src/camera.rs` (field visibility, add getter, clamp in `new`, adjust tests)

**Interfaces:**
- Consumes from Task 4: no external writers of `Camera2D::zoom` remain.
- Produces: `pub fn zoom(&self) -> f32`. `Camera2D::zoom` is no longer a public field. `Camera2D::new` clamps.

- [ ] **Step 1: Write the failing test**

Append to `mod tests` in `crates/voltra-render/src/camera.rs`:

```rust
    #[test]
    fn new_clamps_its_zoom() {
        // The constructor is a second door onto the same invariant. Leaving it
        // unclamped means `Camera2D::new(pos, 0.0, 1.0)` produces infinities in
        // half_extents with no warning anywhere.
        assert_eq!(Camera2D::new(Vec2::ZERO, 0.0, 1.0).zoom(), Camera2D::MIN_ZOOM);
        assert_eq!(
            Camera2D::new(Vec2::ZERO, 1e9, 1.0).zoom(),
            Camera2D::MAX_ZOOM
        );
    }
```

- [ ] **Step 2: Run it and confirm it fails**

```sh
cargo test -p voltra-render --lib camera::tests::new_clamps_its_zoom
```

Expected: compile error, `no method named 'zoom' found for struct 'Camera2D'`.

- [ ] **Step 3: Privatise the field, add the getter, clamp the constructor**

In `crates/voltra-render/src/camera.rs`, the struct becomes:

```rust
pub struct Camera2D {
    pub position: Vec2,
    /// Private because `half_extents` divides by it. Write through
    /// [`Self::set_zoom`], which clamps.
    zoom: f32,
    /// Viewport width divided by height. Keeps squares square.
    pub aspect: f32,
}
```

`new` gains the clamp, and a getter joins the impl:

```rust
    pub fn new(position: Vec2, zoom: f32, aspect: f32) -> Self {
        let mut camera = Self {
            position,
            zoom: 1.0,
            aspect,
        };
        camera.set_zoom(zoom);
        camera
    }

    /// Current zoom. Always within [`Self::MIN_ZOOM`]..=[`Self::MAX_ZOOM`].
    pub fn zoom(&self) -> f32 {
        self.zoom
    }
```

- [ ] **Step 4: Update the call sites the compiler names**

Inside `camera.rs` the field is still reachable, so `half_extents`, `set_zoom` and `zoom_around` need no change. The tests written in Task 1 read `camera.zoom` — change those reads to `camera.zoom()`:

- `zoom_around_keeps_the_anchor_world_point_still`: `camera.zoom` → `camera.zoom()` (twice: the assert and its message)
- `set_zoom_clamps_both_ends`: three `camera.zoom` → `camera.zoom()`
- `set_zoom_ignores_nan`: one
- `zoom_around_at_the_clamp_does_not_drift`: one

Then let the compiler find any others:

```sh
cargo clippy --workspace --all-targets -- -D warnings
```

Expected after fixing: clean. If it names a site outside `voltra-render`, Task 4 was incomplete — go back rather than making the field public again.

- [ ] **Step 5: Run the tests**

```sh
cargo test --workspace
```

Expected: all pass, including `new_clamps_its_zoom`.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-render/src/camera.rs
git commit -m "refactor(render): make zoom private behind set_zoom

half_extents divides by zoom, so a public field is an invariant anyone can
break silently. Nothing outside the crate writes it any more.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Record the decision and fix the docs that now lie

`README.md` currently documents the controls this branch removed from `voltra-core`, and ARCHITECTURE.md has no entry for who owns the editor camera.

**Files:**
- Modify: `README.md:49-55` (the camera controls table)
- Modify: `docs/ARCHITECTURE.md` (new entry under "Decisions"; the egui 0.35 table)

**Interfaces:**
- Consumes: the behaviour settled in Tasks 3-5.
- Produces: nothing code depends on.

- [ ] **Step 1: Correct the README table**

Replace lines 49-55 of `README.md`:

```markdown
Camera controls, active whenever egui is not using the input itself:

| Input | Action |
| --- | --- |
| `W` `A` `S` `D` | Pan the camera |
| Scroll wheel | Zoom |
| `R` | Reset the camera |
```

with:

```markdown
Camera controls. These belong to the editor, not the engine: they act only
while the pointer is over the scene, and a game built on `voltra-core` moves
its own camera.

| Input | Action |
| --- | --- |
| Middle-drag | Pan |
| Scroll wheel | Zoom about the cursor |
| `W` `A` `S` `D` | Pan |
| `R` | Reset the camera |
```

- [ ] **Step 2: Add the decision to ARCHITECTURE.md**

Append to the "Decisions" section of `docs/ARCHITECTURE.md`, after the `egui` entry:

```markdown
### The editor owns the editor camera

`voltra-core` used to read `WASD`, the wheel and `R` in `App::update` and fly
`Renderer::camera` with them. That made the platform layer decide what a pan
gesture is, and handed editor bindings to any game taking the no-UI path.

Checked against the engines that have already answered this:

- Unity's scene camera is `Editor/Mono/SceneView/SceneView.cs`, in the
  `UnityEditor` assembly; `SceneView` extends `EditorWindow`.
- Unreal navigates through `FEditorViewportClient`, an editor-module type.
- Godot handles 2D navigation in `CanvasItemEditor`, an editor plugin, not in
  `Camera2D`.
- Bevy ships no controller in `bevy_render`.

Unanimous: **the render layer exposes a camera, the tool decides how it moves.**
Navigation therefore lives in `voltra-editor::camera::ViewportCamera`, and
`App::update` touches no camera at all.

Two consequences worth stating:

- **Input scoping is egui's job, not ours.** `InputState::smooth_scroll_delta`
  is zeroed by whichever `ScrollArea` consumed it, and keys do not arrive while
  a widget holds focus. Reimplementing that in `Input` would duplicate a
  resolution egui has already made — and get it wrong, as the old code did:
  scrolling the hierarchy zoomed the scene.
- **Zoom is clamped, in the layer that divides by it.** `Camera2D::zoom` is
  private behind `set_zoom`, which clamps to `MIN_ZOOM`..=`MAX_ZOOM` and refuses
  `NaN`. Godot does the same (`CLAMP` in `EditorZoomWidget::set_zoom`); so does
  every editor that has shipped a zoom control. Steps are multiplicative for the
  same reason theirs are — a notch should feel the same at any magnification.

`viewport_to_world` / `world_to_viewport` sit on `Camera2D` rather than in the
editor because the projection lives there. Picking, gizmos and a grid overlay
will all want them.
```

- [ ] **Step 3: Add the egui rows found along the way**

Append to the egui 0.35 table at the bottom of `docs/ARCHITECTURE.md`:

```markdown
| Widget input | `egui::Image` is inert unless given `.sense(Sense::drag())`; without it the `Response` reports no drag and no hover |
| Scroll | Read `InputState::smooth_scroll_delta`, not the raw one — a `ScrollArea` zeroes it once it has consumed it, which is what scopes the wheel to a panel |
| Pointer position | `Response::hover_pos` is in global screen points; subtract `response.rect.min` for widget-local ones |
```

- [ ] **Step 4: Commit**

```sh
git add README.md docs/ARCHITECTURE.md
git commit -m "docs: record who owns the editor camera

The README documented controls this branch removed. ARCHITECTURE.md gains the
prior art the decision was checked against and the two egui behaviours the
implementation leans on.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Final verification

- [ ] **Full clean run**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three must be clean. `--check` on fmt, so a stray format is a failure rather than a silent fixup.

- [ ] **Grep for leftovers**

```sh
git grep -n "PAN_SPEED\|ZOOM_STEP" -- crates
```

Expected: no output. Both constants belonged to the deleted code.

```sh
git grep -n "camera.zoom\b" -- crates
```

Expected: only matches inside `crates/voltra-render/src/camera.rs`.

- [ ] **Editor smoke test**

```sh
cargo run -p voltra-editor > editor.log 2>&1 &
sleep 8
cat editor.log
pkill -f voltra-editor
```

Expected: no `ERROR`, no panic. Re-check the six behaviours listed in Task 3 Step 6.

- [ ] **Push and open the PR** (only when asked — see CLAUDE.md)

```sh
git push -u origin feature/viewport-camera
```
