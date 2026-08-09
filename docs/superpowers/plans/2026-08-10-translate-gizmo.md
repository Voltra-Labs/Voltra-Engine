# Translate Gizmo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Select a sprite and drag a handle to move it — stage 11a, on top of a
line pipeline that the viewport grid and 11b's collision overlay will reuse.

**Architecture:** `voltra_render::lines` uploads world-space segments carrying a
width in pixels; its vertex shader expands each into a screen-space quad, so a
line is the same thickness at any zoom. `pass::draw_lines` records a second pass
over the scene target that **loads** rather than clears. The editor's `gizmo`
module hit-tests handles in screen space, owns a drag, and emits segments — it
never touches a device, and `voltra-render` never learns what an axis is.

**Tech Stack:** Rust 2021, `wgpu` 30, `winit` 0.30, `egui` 0.35, `glam`,
`bytemuck`, `pollster` 1.0.

Design: [`docs/superpowers/specs/2026-08-10-translate-gizmo-design.md`](../specs/2026-08-10-translate-gizmo-design.md).

## Global Constraints

Copied from `CLAUDE.md`, `docs/ARCHITECTURE.md` and `docs/CONVENTIONS.md`.
Every task's requirements implicitly include this section.

- The engine is **2D only**. No depth buffer, no z-axis, no 3D scaffolding.
  Every pipeline keeps `depth_stencil: None` and every pass
  `depth_stencil_attachment: None`.
- Only `voltra-core` may depend on `winit`. Only `voltra-render` may depend on
  `wgpu`. Everything else goes through `voltra_render::wgpu`.
- All versions live in the root `[workspace.dependencies]`. This plan adds no
  dependency at all.
- No `unwrap()` outside tests. `expect("why this cannot fail")` when the
  invariant is real. Log through `log`, never `println!`.
- One concept per file. Split past roughly 300 lines or a second concept,
  `foo.rs` + `foo/`, never `foo/mod.rs`.
- **Verify wgpu 30 against the vendored source, never from memory:**
  `~/.cargo/registry/src/index.crates.io-*/wgpu-30.0.0/src/api/` and
  `.../wgpu-types-30.0.0/src/`. wgpu 30 broke almost every tutorial online.
- Acceptance for every task: `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings` clean, then
  `cargo test --workspace` green. All three, every task, before the commit.
- Conventional Commits, scope = crate without the `voltra-` prefix, imperative
  subject ≤50 chars.
- Branch: `feature/translate-gizmo`. Do not push; the dispatching session does.

## Research this plan is built on

- **There is no line width.** Verified in
  `wgpu-types-30.0.0/src/render.rs:385`: `PrimitiveState` carries `topology`,
  `strip_index_format`, `front_face`, `cull_mode`, `unclipped_depth`,
  `polygon_mode`, `conservative`, and nothing else. `PolygonMode::Line` is
  wireframe fill and needs an optional feature. So `LineList` means one pixel,
  and every wide line is a quad expanded in a shader — which is what Bevy
  adopted `bevy_polyline` into `bevy_gizmos` to do
  ([#8427](https://github.com/bevyengine/bevy/pull/8427)).
- **Hit-test in screen space, not world space.** The Unreal report
  ["gizmo has constant screenspace size, but hit detection scales in world
  space"](https://forums.unrealengine.com/t/gizmo-has-constant-screenspace-size-but-hit-detection-scales-in-world-space/489113)
  is exactly the bug that comes from testing where the handle *is* rather than
  where it is *drawn*.
- **Unity's persistent tool, not Blender's modal transform.** Godot 2D also
  follows Unity, with [a proposal](https://github.com/godotengine/godot-proposals/issues/1215)
  open to add Blender's on top. Blender's needs modal input capture that
  `Input` does not have.

## File Structure

**Created**

- `crates/voltra-render/src/lines.rs` — `LineVertex`, `LineBatch`.
- `crates/voltra-render/src/shaders/lines.wgsl` — the screen-space expansion.
- `crates/voltra-render/tests/headless_lines.rs` — width, zoom-invariance, and
  that the pass does not erase what was under it.
- `crates/voltra-editor/src/tool.rs` — `Tool`.
- `crates/voltra-editor/src/gizmo.rs` — `Gizmo`.
- `crates/voltra-editor/src/gizmo/handle.rs` — `Handle`, its screen geometry and
  hit test.
- `crates/voltra-editor/src/gizmo/drag.rs` — `Drag`.

**Modified**

- `crates/voltra-render/src/mesh.rs` — `Mesh::new` and `Mesh::indexed` become
  generic over the vertex type.
- `crates/voltra-render/src/{pipeline,pass,shader,lib}.rs` — `create_lines`,
  `draw_lines`, `LINES`, the module and re-exports.
- `crates/voltra-render/src/renderer.rs` — owns the line pipeline and the
  viewport uniform.
- `crates/voltra-core/src/app/{draw,ui_frame}.rs` — the overlay pass and the
  seam a panel submits segments through.
- `crates/voltra-editor/src/{editor,panels/viewport}.rs` — holds and drives it.
- `docs/ARCHITECTURE.md`, `README.md` — decisions and the roadmap.

## Execution waves

| Wave | Tasks | Why they cannot move |
| --- | --- | --- |
| 1 | Task 1 | Everything else needs `LineBatch` to exist. |
| 2 | Task 2, Task 4 | Task 2 is render-side wiring; Task 4 is pure editor geometry with no render dependency. Disjoint files. |
| 3 | Task 3 | Needs Task 2's pass and Task 1's batch. |
| 4 | Task 5 | Needs Task 3's seam and Task 4's geometry. |
| 5 | Task 6 | Documents what the rest decided. |

---

### Task 1: A line is a quad the shader widens

**Files:**
- Modify: `crates/voltra-render/src/mesh.rs` (`new`, `indexed`)
- Create: `crates/voltra-render/src/lines.rs`
- Create: `crates/voltra-render/src/shaders/lines.wgsl`
- Modify: `crates/voltra-render/src/shader.rs`, `crates/voltra-render/src/lib.rs`
- Test: unit tests inside `lines.rs`

**Interfaces:**
- Produces:
  - `voltra_render::lines::LineVertex` (`Pod`, `repr(C)`, with `LAYOUT`)
  - `voltra_render::lines::LineBatch::{default, push, clear, is_empty, len, upload}`
  - `LineBatch::push(&mut self, a: Vec2, b: Vec2, width_px: f32, color: [f32; 4])`
  - `LineBatch::upload(&self, device: &wgpu::Device) -> Option<Mesh>`
  - `voltra_render::shader::LINES`
  - re-exported as `voltra_render::{LineBatch, LineVertex}`
- Consumes: `Mesh::indexed`, which this task makes generic.

- [ ] **Step 1: Make `Mesh` generic over the vertex type**

`Mesh` stores buffers and a count; it has never needed to know what a vertex is
after the upload. In `crates/voltra-render/src/mesh.rs`, change both
constructors' signatures only — the bodies are unchanged, because
`bytemuck::cast_slice` already works on any `Pod`:

```rust
    pub fn new<V: Pod>(device: &wgpu::Device, label: &str, vertices: &[V]) -> Self {
```

```rust
    pub fn indexed<V: Pod>(
        device: &wgpu::Device,
        label: &str,
        vertices: &[V],
        indices: &[u32],
    ) -> Self {
```

Add to `Mesh::indexed`'s doc comment:

```rust
    /// Generic over the vertex type because a `Mesh` is buffers and a count:
    /// what the bytes mean is the pipeline's business, declared by whichever
    /// `VertexBufferLayout` it was built with. `Vertex` and `LineVertex` are
    /// two such meanings and there is no third structure to justify.
```

Every existing call site infers `V = Vertex` and needs no change.

- [ ] **Step 2: Write the failing tests**

Create `crates/voltra-render/src/lines.rs` with only this test module for now,
plus the `use` lines it needs:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

    #[test]
    fn a_segment_becomes_four_vertices_and_six_indices() {
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::new(1.0, 0.0), 2.0, WHITE);

        assert_eq!(batch.vertices.len(), 4);
        assert_eq!(batch.indices, vec![0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn every_vertex_carries_both_endpoints() {
        // The shader needs the whole segment at each corner: it takes the
        // perpendicular of the screen-space direction, which needs both ends
        // after projection, not just the one this vertex sits at.
        let a = Vec2::new(-1.0, 2.0);
        let b = Vec2::new(3.0, 4.0);
        let mut batch = LineBatch::default();
        batch.push(a, b, 1.0, WHITE);

        for vertex in &batch.vertices {
            assert_eq!(vertex.a, [a.x, a.y]);
            assert_eq!(vertex.b, [b.x, b.y]);
        }
    }

    #[test]
    fn the_four_corners_are_both_ends_on_both_sides() {
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::new(1.0, 0.0), 2.0, WHITE);

        let corners: Vec<[f32; 2]> = batch.vertices.iter().map(|v| v.corner).collect();
        assert!(corners.contains(&[0.0, -1.0]));
        assert!(corners.contains(&[0.0, 1.0]));
        assert!(corners.contains(&[1.0, 1.0]));
        assert!(corners.contains(&[1.0, -1.0]));
    }

    #[test]
    fn a_zero_length_segment_is_dropped() {
        // Both endpoints equal means the screen-space direction is undefined.
        // Normalising it gives NaN, and a NaN position takes the whole quad
        // with it — a triangle that renders as garbage rather than as nothing.
        let mut batch = LineBatch::default();
        batch.push(Vec2::splat(2.0), Vec2::splat(2.0), 2.0, WHITE);

        assert!(batch.is_empty());
    }

    #[test]
    fn a_segment_shorter_than_the_epsilon_is_dropped() {
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::new(1e-9, 0.0), 2.0, WHITE);

        assert!(batch.is_empty());
    }

    #[test]
    fn a_non_positive_width_is_dropped() {
        // A zero-width quad is four coincident vertices: no pixels, and one
        // more degenerate triangle for the rasteriser to chew on every frame.
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::X, 0.0, WHITE);
        batch.push(Vec2::ZERO, Vec2::X, -3.0, WHITE);

        assert!(batch.is_empty());
    }

    #[test]
    fn indices_of_the_second_segment_are_offset_by_its_own_base() {
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::X, 1.0, WHITE);
        batch.push(Vec2::ZERO, Vec2::Y, 1.0, WHITE);

        assert_eq!(batch.vertices.len(), 8);
        assert_eq!(batch.indices[6..], [4, 5, 6, 4, 6, 7]);
    }

    #[test]
    fn clearing_keeps_the_allocation_and_empties_the_batch() {
        // Rebuilt every frame, so the point of `clear` is to reuse the Vec.
        let mut batch = LineBatch::default();
        batch.push(Vec2::ZERO, Vec2::X, 1.0, WHITE);
        let capacity = batch.vertices.capacity();

        batch.clear();

        assert!(batch.is_empty());
        assert_eq!(batch.vertices.capacity(), capacity);
    }

    #[test]
    fn an_empty_batch_uploads_to_nothing() {
        // Not a zero-length buffer: wgpu rejects a zero-sized buffer, and the
        // pass already treats `None` as "record the pass, draw nothing".
        assert!(LineBatch::default().is_empty());
        assert_eq!(LineBatch::default().len(), 0);
    }
}
```

- [ ] **Step 3: Run them and watch them fail to compile**

Run: `cargo test -p voltra-render lines::`
Expected: FAIL — `cannot find type 'LineBatch' in this scope`.

- [ ] **Step 4: Write the batch**

Put this above the test module in `crates/voltra-render/src/lines.rs`:

```rust
//! Line segments with a width measured in pixels.
//!
//! Nothing here knows what the lines are *for*. The editor's translate gizmo is
//! the first caller; the viewport grid and a collision-shape overlay are the
//! reasons this is a pipeline in the render crate rather than a few calls to
//! `egui::Painter` in a panel.
//!
//! ## Why a line is a quad
//!
//! WebGPU has no line width, and neither does wgpu 30 — `PrimitiveState`
//! carries `topology`, `strip_index_format`, `front_face`, `cull_mode`,
//! `unclipped_depth`, `polygon_mode` and `conservative`, and nothing else.
//! `PrimitiveTopology::LineList` therefore draws one-pixel hairlines, which are
//! invisible on a high-DPI display and impossible to hit-test generously.
//!
//! So each segment is uploaded as a degenerate quad — four vertices all
//! carrying *both* endpoints — and `shaders/lines.wgsl` gives it its width
//! after projection, perpendicular to the direction the segment runs in on
//! screen. Bevy solved it the same way, by adopting `bevy_polyline` into
//! `bevy_gizmos`.
//!
//! Doing the widening after projection is what makes the width a number of
//! pixels rather than a number of world units: a gizmo has to stay the same
//! size on screen at every zoom, and a grid line has to stay hairline-thin when
//! the camera pulls back far enough to show a thousand of them.

use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use crate::mesh::Mesh;

/// Shortest segment worth drawing, in world units.
///
/// Below this the direction `normalize(b - a)` is numerically meaningless, and
/// a NaN there propagates into every vertex of the quad.
const MIN_LENGTH: f32 = 1e-6;

/// One corner of one segment's quad.
///
/// Both endpoints are repeated on all four corners on purpose: the shader has
/// to project *both* before it knows which way the segment runs on screen, and
/// a vertex shader cannot see its neighbours.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct LineVertex {
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// `.x` picks the endpoint — `0.0` for `a`, `1.0` for `b`. `.y` picks the
    /// side, `-1.0` or `+1.0`, which the shader multiplies by half the width.
    pub corner: [f32; 2],
    /// Width in logical pixels, applied after projection.
    pub width: f32,
    pub color: [f32; 4],
}

impl LineVertex {
    /// Attribute layout matching the `@location` bindings in `lines.wgsl`.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32x2, 3 => Float32, 4 => Float32x4
        ],
    };
}

/// Segments accumulated on the CPU, then uploaded as one mesh.
///
/// Rebuilt every frame by whoever draws an overlay, so [`Self::clear`] keeps
/// the allocation rather than dropping it.
#[derive(Debug, Default)]
pub struct LineBatch {
    vertices: Vec<LineVertex>,
    indices: Vec<u32>,
}

impl LineBatch {
    /// Adds one segment, in world units, `width` pixels thick.
    ///
    /// Silently drops a segment that cannot be drawn — zero length, or a width
    /// that is not positive. Both produce degenerate geometry rather than an
    /// error, and a caller emitting an overlay has nothing useful to do with a
    /// `Result` per segment.
    pub fn push(&mut self, a: Vec2, b: Vec2, width: f32, color: [f32; 4]) {
        if width <= 0.0 || a.distance_squared(b) < MIN_LENGTH * MIN_LENGTH {
            return;
        }

        let base = self.vertices.len() as u32;
        let ends = [a.to_array(), b.to_array()];
        for corner in [[0.0, -1.0], [0.0, 1.0], [1.0, 1.0], [1.0, -1.0]] {
            self.vertices.push(LineVertex {
                a: ends[0],
                b: ends[1],
                corner,
                width,
                color,
            });
        }

        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Empties the batch without giving back its memory.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// How many segments are held.
    pub fn len(&self) -> usize {
        self.vertices.len() / 4
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Uploads the batch, or `None` when there is nothing in it.
    ///
    /// `None` rather than an empty `Mesh`: wgpu rejects a zero-sized buffer,
    /// and the passes already treat `None` as "record the pass, draw nothing".
    pub fn upload(&self, device: &wgpu::Device) -> Option<Mesh> {
        if self.is_empty() {
            return None;
        }
        Some(Mesh::indexed(device, "lines", &self.vertices, &self.indices))
    }
}
```

- [ ] **Step 5: Write the shader**

Create `crates/voltra-render/src/shaders/lines.wgsl`:

```wgsl
// Widening happens here, after projection, so the width is in pixels and a
// line keeps its thickness at any zoom. See `lines.rs` for why a line is a
// quad at all: WebGPU has no line width.

struct Camera {
    view_proj: mat4x4<f32>,
};

struct Viewport {
    // Logical pixels. `.zw` is padding: a uniform buffer binding is aligned to
    // 16 bytes and WGSL will not lay out a bare vec2 the way Rust does.
    size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> viewport: Viewport;

struct VertexIn {
    @location(0) a: vec2<f32>,
    @location(1) b: vec2<f32>,
    @location(2) corner: vec2<f32>,
    @location(3) width: f32,
    @location(4) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    let half_size = viewport.size.xy * 0.5;

    // Both endpoints into the same half-pixel space, so the perpendicular
    // below is a screen direction rather than a world one. The camera is
    // orthographic, so w is 1 and the divide is a formality — written out
    // anyway, because it stops being one the day a projection is not ortho.
    let clip_a = camera.view_proj * vec4<f32>(in.a, 0.0, 1.0);
    let clip_b = camera.view_proj * vec4<f32>(in.b, 0.0, 1.0);
    let screen_a = clip_a.xy / clip_a.w * half_size;
    let screen_b = clip_b.xy / clip_b.w * half_size;

    let delta = screen_b - screen_a;
    // `len`, not `length`: shadowing the builtin makes the call below a type
    // error rather than a recursion, which is a confusing way to find out.
    let len = max(length(delta), 1e-6);
    let dir = delta / len;
    let normal = vec2<f32>(-dir.y, dir.x);

    let at = select(screen_a, screen_b, in.corner.x > 0.5);
    let widened = at + normal * in.corner.y * in.width * 0.5;

    var out: VertexOut;
    out.clip = vec4<f32>(widened / half_size, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
```

- [ ] **Step 6: Declare the shader and the module**

In `crates/voltra-render/src/shader.rs`, beside the other constants:

```rust
/// Source of the shader that widens line segments in screen space.
pub const LINES: &str = include_str!("shaders/lines.wgsl");
```

In `crates/voltra-render/src/lib.rs`, add `pub mod lines;` and
`pub use lines::{LineBatch, LineVertex};`, each in the file's existing
alphabetical position.

- [ ] **Step 7: Run the tests**

Run: `cargo test -p voltra-render lines::`
Expected: PASS, nine tests.

- [ ] **Step 8: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 9: Commit**

```sh
git add crates/voltra-render/src/lines.rs crates/voltra-render/src/shaders/lines.wgsl crates/voltra-render/src/shader.rs crates/voltra-render/src/lib.rs crates/voltra-render/src/mesh.rs
git commit -m "feat(render): add line segments with a pixel width"
```

---

### Task 2: The pipeline, and a pass that does not clear

**Files:**
- Modify: `crates/voltra-render/src/pipeline.rs`
- Modify: `crates/voltra-render/src/pass.rs`
- Create: `crates/voltra-render/tests/headless_lines.rs`

**Interfaces:**
- Consumes: `LineVertex::LAYOUT`, `shader::LINES` (Task 1),
  `CameraBinding::layout()` (`camera.rs:211`).
- Produces:
  - `pipeline::create_lines(device, format, camera_layout, viewport_layout) -> wgpu::RenderPipeline`
  - `pipeline::viewport_bind_group_layout(device) -> wgpu::BindGroupLayout`
  - `pass::draw_lines(encoder, view, pipeline, camera, viewport, mesh: Option<&Mesh>)`

- [ ] **Step 1: Write the failing GPU tests**

Create `crates/voltra-render/tests/headless_lines.rs`:

```rust
//! Lines, in pixels.
//!
//! The three questions unit tests cannot answer: does the width reach the
//! screen, is it a number of pixels rather than of world units, and does the
//! pass leave what was under it alone.
//!
//! Skips itself when no GPU adapter is available.

use voltra_render::wgpu;
use voltra_render::{lines::LineBatch, pass, pipeline, Camera2D, CameraBinding};
use voltra_render::glam::Vec2;
use voltra_testkit::{headless_device, read_texture, Rgba, CLEAR};

const SIZE: u32 = 64;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

macro_rules! device_or_skip {
    () => {
        match headless_device() {
            Some(pair) => pair,
            None => {
                eprintln!("no GPU adapter; skipping");
                return;
            }
        }
    };
}

/// Renders one batch over a cleared target and reads the frame back.
fn render(device: &wgpu::Device, queue: &wgpu::Queue, batch: &LineBatch, zoom: f32) -> Vec<Rgba> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless-lines-target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let camera_binding = CameraBinding::new(device);
    camera_binding.upload(queue, &Camera2D::new(Vec2::ZERO, zoom, 1.0));

    let viewport_layout = pipeline::viewport_bind_group_layout(device);
    let viewport = pipeline::viewport_binding(device, queue, &viewport_layout, SIZE, SIZE);
    let line_pipeline =
        pipeline::create_lines(device, FORMAT, camera_binding.layout(), &viewport_layout);

    let mesh = batch.upload(device);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-lines-encoder"),
    });
    // A clear first, in its own pass, because `draw_lines` deliberately does
    // not clear — that is what makes it an overlay.
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &line_pipeline,
        camera_binding.bind_group(),
        None,
        &[],
        CLEAR,
    );
    pass::draw_lines(
        &mut encoder,
        &view,
        &line_pipeline,
        camera_binding.bind_group(),
        &viewport,
        mesh.as_ref(),
    );
    queue.submit(Some(encoder.finish()));

    read_texture(device, queue, &target, SIZE, SIZE)
}

/// Pixels that are not the clear colour.
fn painted(pixels: &[Rgba]) -> usize {
    pixels.iter().filter(|px| !px.is_clear_ish()).count()
}

#[test]
fn a_wider_line_paints_more_pixels() {
    let (device, queue) = device_or_skip!();

    let mut thin = LineBatch::default();
    thin.push(Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0), 1.0, WHITE);
    let mut thick = LineBatch::default();
    thick.push(Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0), 5.0, WHITE);

    let thin_px = painted(&render(&device, &queue, &thin, 0.5));
    let thick_px = painted(&render(&device, &queue, &thick, 0.5));

    assert!(thin_px > 0, "the thin line drew nothing");
    assert!(
        thick_px > thin_px * 2,
        "5px must paint far more than 1px: {thick_px} vs {thin_px}"
    );
}

#[test]
fn the_width_is_pixels_not_world_units() {
    // The same segment at two zooms. Its *length* on screen changes; its
    // *thickness* must not, which is the whole reason the widening happens
    // after projection.
    let (device, queue) = device_or_skip!();

    let mut batch = LineBatch::default();
    batch.push(Vec2::new(0.0, -4.0), Vec2::new(0.0, 4.0), 4.0, WHITE);

    // Vertical and longer than the frame at both zooms, so it spans every row
    // in each and only the column count can differ.
    let near = render(&device, &queue, &batch, 0.5);
    let far = render(&device, &queue, &batch, 0.25);

    let row = (SIZE / 2) as usize;
    let width_of = |px: &[Rgba]| {
        (0..SIZE as usize)
            .filter(|x| !px[row * SIZE as usize + x].is_clear_ish())
            .count()
    };

    let near_w = width_of(&near);
    let far_w = width_of(&far);
    assert!(near_w > 0, "nothing drawn");
    assert_eq!(
        near_w, far_w,
        "thickness changed with zoom: {near_w} vs {far_w}"
    );
}

#[test]
fn the_line_pass_does_not_erase_what_was_under_it() {
    let (device, queue) = device_or_skip!();

    let mut batch = LineBatch::default();
    batch.push(Vec2::new(-0.5, 0.0), Vec2::new(0.5, 0.0), 3.0, WHITE);

    let pixels = render(&device, &queue, &batch, 0.5);

    // The clear colour has to still be there in the corners. If `draw_lines`
    // cleared, every pixel would be the clear colour and the line would be
    // gone; if it cleared to something else, the corners would not match.
    assert!(pixels[0].is_clear_ish(), "corner was overwritten");
    assert!(painted(&pixels) > 0, "the line itself is missing");
}

#[test]
fn an_empty_batch_draws_nothing_and_does_not_panic() {
    let (device, queue) = device_or_skip!();

    let pixels = render(&device, &queue, &LineBatch::default(), 0.5);

    assert_eq!(painted(&pixels), 0);
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p voltra-render --test headless_lines`
Expected: FAIL — `cannot find function 'create_lines' in module 'pipeline'`.

`voltra-render` needs `voltra-testkit` under `[dev-dependencies]`; it already
has it from stage 12b. Confirm rather than assume:
`grep voltra-testkit crates/voltra-render/Cargo.toml`.

- [ ] **Step 3: The viewport uniform and its layout**

Append to `crates/voltra-render/src/pipeline.rs`:

```rust
/// Layout of the viewport-size uniform the line shader widens with.
///
/// Group 1 — the slot the sprite pipeline gives its texture — so that group 0
/// is the camera in every pipeline this engine has.
pub fn viewport_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viewport-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

/// The viewport size, uploaded, with its bind group.
///
/// A `vec4` rather than a `vec2`: a uniform buffer binding is aligned to 16
/// bytes, and the two unused floats are cheaper than a padding field nobody
/// remembers the reason for.
pub fn viewport_binding(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
) -> wgpu::BindGroup {
    let size = [width.max(1) as f32, height.max(1) as f32, 0.0, 0.0];
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("viewport-uniform"),
        size: size_of::<[f32; 4]>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&size));

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viewport-bind-group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}
```

`width.max(1)`: a minimised window reports zero, and the shader divides by half
of it.

- [ ] **Step 4: The pipeline**

Also in `pipeline.rs`:

```rust
/// Builds the line pipeline.
///
/// Same shape as [`create_flat_color`] — `TriangleList`, no culling, no depth,
/// alpha blending — because a widened line *is* triangles. What differs is the
/// vertex layout and the shader that reads it.
pub fn create_lines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_layout: &wgpu::BindGroupLayout,
    viewport_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let module = shader::create_module(device, "lines", shader::LINES);

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("lines-layout"),
        bind_group_layouts: &[Some(camera_layout), Some(viewport_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("lines-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(LineVertex::LAYOUT)],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            // A widened quad's winding flips with the segment's direction, so
            // culling would drop every line that happened to run the other way.
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
```

Add `use crate::lines::LineVertex;` to the file's imports.

- [ ] **Step 5: The pass that loads**

Append to `crates/voltra-render/src/pass.rs`:

```rust
/// Records `mesh` over whatever is already in `view`.
///
/// The only pass in the engine that **loads** rather than clears. Every other
/// one is the single thing drawn into its target that frame; an overlay is by
/// definition the second, and clearing here would erase the scene it annotates.
///
/// `camera` is bound to group 0 and `viewport` to group 1, matching
/// [`pipeline::create_lines`](crate::pipeline::create_lines).
///
/// `None` records nothing at all — not even a pass. There is nothing to
/// preserve and nothing to draw, and a load-store pass over an untouched target
/// is pure bandwidth.
pub fn draw_lines(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    camera: &wgpu::BindGroup,
    viewport: &wgpu::BindGroup,
    mesh: Option<&Mesh>,
) {
    let Some(mesh) = mesh else {
        return;
    };

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("lines-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, camera, &[]);
    pass.set_bind_group(1, viewport, &[]);
    mesh.draw(&mut pass);
}
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p voltra-render --test headless_lines`
Expected: PASS, four tests.

If `the_width_is_pixels_not_world_units` fails with the two counts differing by
roughly the zoom ratio, the widening is happening before projection — the
`half_size` multiply is on the wrong side of the `view_proj` multiply in
`lines.wgsl`.

- [ ] **Step 7: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 8: Commit**

```sh
git add crates/voltra-render/src/pipeline.rs crates/voltra-render/src/pass.rs crates/voltra-render/tests/headless_lines.rs
git commit -m "feat(render): draw lines in a pass that loads"
```

---

### Task 3: The overlay reaches the frame

Wires the line pass into `App`'s redraw and gives a UI panel a seam to submit
segments through, so a panel never touches a device.

**Files:**
- Modify: `crates/voltra-render/src/renderer.rs`
- Modify: `crates/voltra-core/src/app/draw.rs`
- Modify: `crates/voltra-core/src/app/ui_frame.rs`
- Modify: `crates/voltra-core/src/app.rs`

**Interfaces:**
- Consumes: `pipeline::{create_lines, viewport_bind_group_layout, viewport_binding}`,
  `pass::draw_lines`, `LineBatch` (Tasks 1 and 2).
- Produces:
  - `Renderer::line_pipeline(&self) -> &wgpu::RenderPipeline`
  - `Renderer::viewport_layout(&self) -> &wgpu::BindGroupLayout`
  - `UiFrame::lines(&mut self) -> &mut LineBatch`

- [ ] **Step 1: `Renderer` owns the pipeline**

In `crates/voltra-render/src/renderer.rs`, add two fields beside the existing
pipeline and layout, build them in the constructor next to their neighbours, and
expose them:

```rust
    /// The line pipeline, built against the same format as the scene pipeline.
    line_pipeline: wgpu::RenderPipeline,
    /// Layout of the viewport-size uniform the line shader needs.
    viewport_layout: wgpu::BindGroupLayout,
```

```rust
    pub fn line_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.line_pipeline
    }

    pub fn viewport_layout(&self) -> &wgpu::BindGroupLayout {
        &self.viewport_layout
    }
```

Read the constructor before editing: build `viewport_layout` first, then
`line_pipeline` from it, the camera binding's layout, and the same
`TextureFormat` the flat-colour pipeline is given.

- [ ] **Step 2: `UiFrame` gains the seam**

In `crates/voltra-core/src/app/ui_frame.rs`, add the field and the accessor:

```rust
    /// Segments to draw over the scene this frame.
    ///
    /// A panel pushes into this and never sees a device: `App` uploads it and
    /// records the pass after the scene, in the same target, before egui
    /// samples it. Cleared at the start of every frame, so a panel that stops
    /// pushing stops drawing.
    pub(super) lines: &'a mut LineBatch,
```

```rust
    /// The overlay a panel draws into, in world units with pixel widths.
    pub fn lines(&mut self) -> &mut LineBatch {
        self.lines
    }
```

Every construction site of `UiFrame` gains the field; the compiler names them.

- [ ] **Step 3: `App` holds the batch and clears it per frame**

In `crates/voltra-core/src/app.rs`, add to `struct App`:

```rust
    /// Overlay segments, rebuilt by the UI every frame.
    lines: LineBatch,
```

`LineBatch` is `Default`, so `#[derive(Default)]` on `App` still holds.

- [ ] **Step 4: Record the pass**

In `crates/voltra-core/src/app/draw.rs`, after the scene pass is recorded into
the scene target and before the frame is submitted:

```rust
        // The overlay goes into the same target as the scene, after it, so egui
        // samples one image with the gizmo already in it. Widths are in pixels,
        // so the uniform carries the target's size rather than the window's.
        let viewport = pipeline::viewport_binding(
            device,
            queue,
            renderer.viewport_layout(),
            target.width(),
            target.height(),
        );
        let lines = self.lines.upload(device);
        pass::draw_lines(
            &mut encoder,
            target.raw_view(),
            renderer.line_pipeline(),
            camera_binding.bind_group(),
            &viewport,
            lines.as_ref(),
        );
```

Read `draw.rs` first: the names above (`encoder`, `target`, `camera_binding`)
are the ones the scene pass already uses, and this must sit inside the same
scope with the same encoder, not a new one.

Clear the batch at the point the UI is about to rebuild it — immediately before
the UI callback runs, not after the draw, so a frame that skips rendering does
not leave stale segments behind.

- [ ] **Step 5: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: green, and nothing visibly different yet — no panel pushes a segment
until Task 5.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-render/src/renderer.rs crates/voltra-core/src/app.rs crates/voltra-core/src/app/draw.rs crates/voltra-core/src/app/ui_frame.rs
git commit -m "feat(core): draw a line overlay over the scene"
```

---

### Task 4: The gizmo's geometry and its drag

Pure arithmetic. No GPU, no egui, no world — so all of it is unit-tested in the
files it lives in. Disjoint from Tasks 2 and 3.

**Files:**
- Create: `crates/voltra-editor/src/tool.rs`
- Create: `crates/voltra-editor/src/gizmo/handle.rs`
- Create: `crates/voltra-editor/src/gizmo/drag.rs`
- Modify: `crates/voltra-editor/src/main.rs` (`mod tool; mod gizmo;`)

**Interfaces:**
- Produces:
  - `tool::Tool` — `Translate`, `Default`
  - `gizmo::handle::{Handle, AXIS_LENGTH, CENTRE_HALF, GRAB_MARGIN, LINE_WIDTH}`
  - `Handle::at(cursor: Vec2, origin: Vec2) -> Option<Handle>` — screen space
  - `Handle::constrain(self, delta: Vec2) -> Vec2`
  - `gizmo::drag::Drag { entity, handle, grab: Vec2, start: Vec2 }`
  - `Drag::translation(&self, cursor_world: Vec2) -> Vec2`

- [ ] **Step 1: Write `Tool`**

Create `crates/voltra-editor/src/tool.rs`:

```rust
//! Which transform the next drag in the viewport performs.
//!
//! Unity's model rather than Blender's: the tool is persistent state, and the
//! gizmo on screen tells you what a drag will do before you start it. Blender
//! binds the transform to a gesture instead — `G`/`R`/`S` begin a modal
//! transform the mouse drives until a click confirms — which is faster once
//! learned and invisible until someone tells you the letter. It also needs
//! modal input capture that `Input` does not have: swallowing every key while
//! active, surviving a lost window focus, unwinding on `Esc`. That can be added
//! over a working gizmo; the reverse is harder. Godot 2D made the same call and
//! has a proposal open to add Blender's on top.

/// The active transform tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    /// Click to select, drag a handle to move. The only tool in stage 11a.
    #[default]
    Translate,
}
```

One variant rather than a `bool`, so `Rotate` and `Scale` arrive as variants
rather than as a rename.

- [ ] **Step 2: Write the failing handle tests**

Create `crates/voltra-editor/src/gizmo/handle.rs` with only this test module and
the `use` lines it needs:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The gizmo's origin in these tests, in screen pixels.
    const O: Vec2 = Vec2::new(100.0, 100.0);

    #[test]
    fn the_centre_square_is_hit_at_the_origin() {
        assert_eq!(Handle::at(O, O), Some(Handle::Both));
    }

    #[test]
    fn a_point_along_the_x_arm_hits_x() {
        // Screen y grows downward, so the X arm runs right and the Y arm runs
        // *up* — negative y. Getting that backwards puts the Y handle where
        // nobody points at it.
        let on_x = O + Vec2::new(AXIS_LENGTH * 0.75, 0.0);
        assert_eq!(Handle::at(on_x, O), Some(Handle::X));
    }

    #[test]
    fn a_point_along_the_y_arm_hits_y() {
        let on_y = O + Vec2::new(0.0, -AXIS_LENGTH * 0.75);
        assert_eq!(Handle::at(on_y, O), Some(Handle::Y));
    }

    #[test]
    fn the_arm_below_the_origin_is_not_the_y_handle() {
        // The Y arm points up only. A gizmo whose arms are bidirectional makes
        // the quadrant below-left ambiguous with nothing drawn there.
        let below = O + Vec2::new(0.0, AXIS_LENGTH * 0.75);
        assert_eq!(Handle::at(below, O), None);
    }

    #[test]
    fn just_outside_the_grab_margin_misses() {
        let near = O + Vec2::new(AXIS_LENGTH * 0.5, GRAB_MARGIN + 1.0);
        assert_eq!(Handle::at(near, O), None);
    }

    #[test]
    fn just_inside_the_grab_margin_hits() {
        // The margin is wider than the drawn line on purpose: a two-pixel line
        // is not a two-pixel target.
        let near = O + Vec2::new(AXIS_LENGTH * 0.5, GRAB_MARGIN - 1.0);
        assert_eq!(Handle::at(near, O), Some(Handle::X));
    }

    #[test]
    fn the_centre_wins_where_it_overlaps_an_arm() {
        // Both arms start at the origin, so the square overlaps both. Tested
        // last it would be unreachable, which is the smallest target in the
        // gizmo being the hardest to hit.
        let inside_square = O + Vec2::new(CENTRE_HALF * 0.5, 0.0);
        assert_eq!(Handle::at(inside_square, O), Some(Handle::Both));
    }

    #[test]
    fn a_point_beyond_the_arm_misses() {
        let past = O + Vec2::new(AXIS_LENGTH + GRAB_MARGIN + 1.0, 0.0);
        assert_eq!(Handle::at(past, O), None);
    }

    #[test]
    fn x_constrains_a_delta_to_its_axis() {
        assert_eq!(Handle::X.constrain(Vec2::new(3.0, 7.0)), Vec2::new(3.0, 0.0));
    }

    #[test]
    fn y_constrains_a_delta_to_its_axis() {
        assert_eq!(Handle::Y.constrain(Vec2::new(3.0, 7.0)), Vec2::new(0.0, 7.0));
    }

    #[test]
    fn both_passes_a_delta_through_untouched() {
        let delta = Vec2::new(3.0, 7.0);
        assert_eq!(Handle::Both.constrain(delta), delta);
    }
}
```

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test -p voltra-editor handle::`
Expected: FAIL — `cannot find type 'Handle' in this scope`.

- [ ] **Step 4: Write the handle**

Above the test module in `crates/voltra-editor/src/gizmo/handle.rs`:

```rust
//! Which part of the gizmo a point is over.
//!
//! Everything here is in **screen pixels**, and that is the point. The gizmo is
//! drawn at a constant size on screen, so testing a click against its world
//! position would test a target that grows and shrinks with the zoom while the
//! picture does not — the exact mismatch Unreal has a bug report for. Testing
//! where it is drawn is the only way the two agree.

use voltra_render::glam::Vec2;

/// Length of each axis arm, in pixels.
pub const AXIS_LENGTH: f32 = 60.0;

/// Half-side of the centre square, in pixels.
pub const CENTRE_HALF: f32 = 8.0;

/// How far from the drawn geometry still counts as a grab, in pixels.
///
/// Wider than [`LINE_WIDTH`] deliberately: a two-pixel line is a two-pixel
/// picture, not a two-pixel target, and every editor pads it.
pub const GRAB_MARGIN: f32 = 6.0;

/// Thickness of the drawn arms, in pixels.
pub const LINE_WIDTH: f32 = 2.0;

/// The part of the gizmo under the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    /// The X arm. A drag moves along world x only.
    X,
    /// The Y arm. A drag moves along world y only.
    Y,
    /// The centre square. A drag moves freely.
    Both,
}

impl Handle {
    /// The handle at `cursor`, both points in viewport-local pixels.
    ///
    /// The centre square is tested first because both arms begin inside it;
    /// tested last it could never be hit, and it is the smallest target there
    /// is.
    pub fn at(cursor: Vec2, origin: Vec2) -> Option<Self> {
        let d = cursor - origin;

        if d.x.abs() <= CENTRE_HALF && d.y.abs() <= CENTRE_HALF {
            return Some(Self::Both);
        }

        // Screen y grows downward, so the arm drawn upward is negative y.
        if (0.0..=AXIS_LENGTH + GRAB_MARGIN).contains(&d.x) && d.y.abs() <= GRAB_MARGIN {
            return Some(Self::X);
        }
        if (-(AXIS_LENGTH + GRAB_MARGIN)..=0.0).contains(&d.y) && d.x.abs() <= GRAB_MARGIN {
            return Some(Self::Y);
        }

        None
    }

    /// `delta` with the axes this handle does not move zeroed out.
    pub fn constrain(self, delta: Vec2) -> Vec2 {
        match self {
            Self::X => Vec2::new(delta.x, 0.0),
            Self::Y => Vec2::new(0.0, delta.y),
            Self::Both => delta,
        }
    }
}
```

- [ ] **Step 5: Write the failing drag tests**

Create `crates/voltra-editor/src/gizmo/drag.rs` with only its test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use voltra_ecs::Entity;

    fn drag(handle: Handle, grab: Vec2, start: Vec2) -> Drag {
        Drag {
            entity: Entity::from_raw(0, 0),
            handle,
            grab,
            start,
        }
    }

    #[test]
    fn a_free_drag_moves_by_the_cursor_delta() {
        let d = drag(Handle::Both, Vec2::new(10.0, 10.0), Vec2::new(2.0, 3.0));

        assert_eq!(
            d.translation(Vec2::new(14.0, 17.0)),
            Vec2::new(2.0 + 4.0, 3.0 + 7.0)
        );
    }

    #[test]
    fn the_entity_does_not_teleport_to_the_cursor() {
        // Grabbing the arm a long way from the sprite's origin and moving one
        // unit must move the sprite one unit — not jump its origin under the
        // cursor. Every first implementation of a gizmo gets this wrong.
        let d = drag(Handle::Both, Vec2::new(50.0, 50.0), Vec2::ZERO);

        assert_eq!(d.translation(Vec2::new(51.0, 50.0)), Vec2::new(1.0, 0.0));
    }

    #[test]
    fn an_x_drag_leaves_y_exactly_alone() {
        let d = drag(Handle::X, Vec2::ZERO, Vec2::new(5.0, 5.0));

        assert_eq!(d.translation(Vec2::new(3.0, 99.0)), Vec2::new(8.0, 5.0));
    }

    #[test]
    fn a_y_drag_leaves_x_exactly_alone() {
        let d = drag(Handle::Y, Vec2::ZERO, Vec2::new(5.0, 5.0));

        assert_eq!(d.translation(Vec2::new(99.0, 3.0)), Vec2::new(5.0, 8.0));
    }

    #[test]
    fn a_drag_that_has_not_moved_changes_nothing() {
        let grab = Vec2::new(7.0, -2.0);
        let start = Vec2::new(1.0, 1.0);
        let d = drag(Handle::Both, grab, start);

        assert_eq!(d.translation(grab), start);
    }
}
```

`Entity::from_raw` may not exist under that name — read
`crates/voltra-ecs/src/entity.rs` and use whatever constructs an `Entity` in its
own tests. If nothing public does, spawn one from a `World` in the test instead;
do not add a constructor to `voltra-ecs` for a test's convenience.

- [ ] **Step 6: Run them and watch them fail**

Run: `cargo test -p voltra-editor drag::`
Expected: FAIL — `cannot find type 'Drag' in this scope`.

- [ ] **Step 7: Write the drag**

Above the test module in `crates/voltra-editor/src/gizmo/drag.rs`:

```rust
//! A grab in progress.
//!
//! Both anchors are in **world** units, unlike [`Handle`], which is screen
//! pixels. That split is deliberate: the hit test has to match the picture, and
//! the movement has to survive the picture changing. A drag anchored in screen
//! space would make the sprite jump when the viewport is resized or the camera
//! zoomed mid-drag.

use voltra_ecs::Entity;
use voltra_render::glam::Vec2;

use super::handle::Handle;

/// The entity being moved, and where the movement started from.
#[derive(Debug, Clone, Copy)]
pub struct Drag {
    /// Held rather than re-picked each frame: a drag follows the entity it
    /// began on, even when the cursor leaves it or leaves the viewport.
    pub entity: Entity,
    pub handle: Handle,
    /// Cursor position, in world units, when the grab began.
    pub grab: Vec2,
    /// The entity's translation when the grab began.
    pub start: Vec2,
}

impl Drag {
    /// Where the entity belongs with the cursor at `cursor`, in world units.
    ///
    /// `start + (cursor − grab)`, not `cursor`. Setting the translation to the
    /// cursor teleports the sprite's origin under the pointer the moment a drag
    /// begins anywhere but dead centre.
    pub fn translation(&self, cursor: Vec2) -> Vec2 {
        self.start + self.handle.constrain(cursor - self.grab)
    }
}
```

- [ ] **Step 8: Declare the modules**

In `crates/voltra-editor/src/main.rs`, add `mod gizmo;` and `mod tool;` to the
existing `mod` list, alphabetically. Create `crates/voltra-editor/src/gizmo.rs`
holding only its module declarations and doc comment for now; Task 5 fills it:

```rust
//! The translate gizmo: what is drawn over the selection, and what a drag does.

pub mod drag;
pub mod handle;
```

- [ ] **Step 9: Run the tests**

Run: `cargo test -p voltra-editor`
Expected: PASS, sixteen new tests.

- [ ] **Step 10: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 11: Commit**

```sh
git add crates/voltra-editor/src/tool.rs crates/voltra-editor/src/gizmo.rs crates/voltra-editor/src/gizmo crates/voltra-editor/src/main.rs
git commit -m "feat(editor): add gizmo handles and drag arithmetic"
```

---

### Task 5: The gizmo on screen, and the drag that moves a sprite

**Files:**
- Modify: `crates/voltra-editor/src/gizmo.rs`
- Modify: `crates/voltra-editor/src/editor.rs`
- Modify: `crates/voltra-editor/src/panels/viewport.rs`

**Interfaces:**
- Consumes: `Handle`, `Drag`, `Tool` (Task 4); `UiFrame::lines` (Task 3);
  `Camera2D::{world_to_viewport, viewport_to_world}` (`camera.rs:70`, `:81`);
  `picking::clicked_entity`.
- Produces:
  - `Gizmo::update(&mut self, response: &Response, frame: &mut UiFrame<'_>, selected: Option<Entity>) -> bool`
  - `Gizmo::draw(&self, frame: &mut UiFrame<'_>, response: &Response, selected: Option<Entity>)`

- [ ] **Step 1: Write the gizmo**

Fill `crates/voltra-editor/src/gizmo.rs`:

```rust
//! The translate gizmo: what is drawn over the selection, and what a drag does.
//!
//! Split three ways because they fail differently. `handle` is screen-space
//! geometry, `drag` is world-space arithmetic, and this file is the frame: it
//! reads the response, decides whether a press begins a drag, and asks the
//! world for the transform. The first two are pure and unit-tested; this one
//! needs egui and a world, and is verified by driving the editor.

pub mod drag;
pub mod handle;

use voltra_core::egui::{PointerButton, Response};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::Transform;

use drag::Drag;
use handle::{Handle, AXIS_LENGTH, CENTRE_HALF, LINE_WIDTH};

/// Colours, matching the axis convention every editor uses: x red, y green.
const X_COLOR: [f32; 4] = [0.90, 0.25, 0.25, 1.0];
const Y_COLOR: [f32; 4] = [0.35, 0.85, 0.35, 1.0];
const CENTRE_COLOR: [f32; 4] = [0.95, 0.95, 0.95, 1.0];

/// The translate gizmo's per-frame state.
#[derive(Debug, Default)]
pub struct Gizmo {
    /// `Some` only between a press on a handle and the release that ends it.
    drag: Option<Drag>,
}

impl Gizmo {
    /// Applies one frame of interaction.
    ///
    /// Returns whether the gizmo consumed the interaction, so the caller knows
    /// not to also treat the click as a selection. A handle drawn on top of a
    /// sprite has to win, or the gizmo is unusable exactly when the sprite
    /// fills the viewport.
    pub fn update(
        &mut self,
        response: &Response,
        frame: &mut UiFrame<'_>,
        selected: Option<Entity>,
    ) -> bool {
        let viewport = viewport_size(response);
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            // A minimised window. The conversions below divide by this.
            self.drag = None;
            return false;
        }

        if response.drag_stopped() || !response.is_pointer_button_down_on() {
            // Released anywhere, including outside the viewport: the drag is
            // owned by the gizmo, not by the cursor still being over a handle.
            if self.drag.take().is_some() {
                return true;
            }
        }

        let Some(pointer) = response.interact_pointer_pos().or(response.hover_pos()) else {
            return false;
        };
        let local = Vec2::new(pointer.x - response.rect.min.x, pointer.y - response.rect.min.y);
        let cursor_world = frame.camera.viewport_to_world(local, viewport);

        if let Some(drag) = self.drag {
            let Some(transform) = frame.world.get_mut::<Transform>(drag.entity) else {
                // Despawned mid-drag, or its Transform removed. Ending the drag
                // is the whole response: there is nothing left to move.
                self.drag = None;
                return true;
            };
            transform.translation = drag.translation(cursor_world);
            return true;
        }

        if !response.drag_started_by(PointerButton::Primary)
            && !response.clicked_by(PointerButton::Primary)
        {
            return false;
        }

        let Some(entity) = selected else {
            return false;
        };
        let Some(origin) = self.origin(frame, response, entity) else {
            return false;
        };
        let Some(handle) = Handle::at(local, origin) else {
            return false;
        };
        let Some(transform) = frame.world.get::<Transform>(entity) else {
            return false;
        };

        self.drag = Some(Drag {
            entity,
            handle,
            grab: cursor_world,
            start: transform.translation,
        });
        true
    }

    /// Pushes the gizmo's segments for this frame.
    pub fn draw(&self, frame: &mut UiFrame<'_>, response: &Response, selected: Option<Entity>) {
        let viewport = viewport_size(response);
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            return;
        }
        let Some(entity) = selected else {
            return;
        };
        let Some(transform) = frame.world.get::<Transform>(entity) else {
            return;
        };

        // The arms are a length in *pixels*, so they are laid out in screen
        // space and converted back — the only way an arm is the same length at
        // every zoom.
        let origin = transform.translation;
        let screen = frame.camera.world_to_viewport(origin, viewport);
        let to_world = |offset: Vec2| {
            frame
                .camera
                .viewport_to_world(screen + offset, viewport)
        };

        let x_tip = to_world(Vec2::new(AXIS_LENGTH, 0.0));
        let y_tip = to_world(Vec2::new(0.0, -AXIS_LENGTH));
        let corner = to_world(Vec2::splat(CENTRE_HALF));
        let half = (corner - origin).abs();

        let lines = frame.lines();
        lines.push(origin, x_tip, LINE_WIDTH, X_COLOR);
        lines.push(origin, y_tip, LINE_WIDTH, Y_COLOR);

        // The centre square, as four segments. A filled quad would need the
        // sprite pipeline and a texture; four lines need nothing new.
        let (lo, hi) = (origin - half, origin + half);
        for (a, b) in [
            (Vec2::new(lo.x, lo.y), Vec2::new(hi.x, lo.y)),
            (Vec2::new(hi.x, lo.y), Vec2::new(hi.x, hi.y)),
            (Vec2::new(hi.x, hi.y), Vec2::new(lo.x, hi.y)),
            (Vec2::new(lo.x, hi.y), Vec2::new(lo.x, lo.y)),
        ] {
            lines.push(a, b, LINE_WIDTH, CENTRE_COLOR);
        }
    }

    /// The selection's origin in viewport-local pixels.
    fn origin(&self, frame: &UiFrame<'_>, response: &Response, entity: Entity) -> Option<Vec2> {
        let transform = frame.world.get::<Transform>(entity)?;
        Some(
            frame
                .camera
                .world_to_viewport(transform.translation, viewport_size(response)),
        )
    }
}

fn viewport_size(response: &Response) -> Vec2 {
    Vec2::new(response.rect.width(), response.rect.height())
}
```

`World::get` and `World::get_mut` may be named differently — read
`crates/voltra-ecs/src/world.rs` and use what is there. Do not add a method to
`voltra-ecs` for this; a component lookup by entity certainly already exists,
because the inspector edits one.

- [ ] **Step 2: `Editor` holds the tool and the gizmo**

In `crates/voltra-editor/src/editor.rs`, add two fields to `Editor`:

```rust
    /// Which transform a viewport drag performs. Persistent, Unity-style.
    pub tool: Tool,
    pub gizmo: Gizmo,
```

`Editor` derives `Default` and both are `Default`, so nothing else changes.

- [ ] **Step 3: The viewport drives it**

In `crates/voltra-editor/src/panels/viewport.rs`, the selection currently
happens on click. Put the gizmo in front of it:

```rust
    // The gizmo gets first refusal, for the same reason egui gets it before
    // the scene: a handle drawn over a sprite must be grabbable.
    let consumed = editor.gizmo.update(&response, frame, editor.selected);
    if !consumed && response.clicked() {
        editor.selected = picking::clicked_entity(&response, frame);
    }
    editor.gizmo.draw(frame, &response, editor.selected);
```

Read the file first: it is 39 lines and the existing selection call is the one
being wrapped, not replaced.

`W` selects the translate tool. Bind it beside the camera's keys, gated the same
way `ViewportCamera::navigate` gates its own — on `response.hovered()` and on
`Context::egui_wants_keyboard_input` — so typing a `w` into the inspector cannot
switch tools:

```rust
    if response.hovered() && !ui.ctx().wants_keyboard_input() {
        ui.input(|i| {
            if i.key_pressed(Key::W) {
                editor.tool = Tool::Translate;
            }
        });
    }
```

- [ ] **Step 4: fmt, clippy, test**

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 5: Drive the editor**

The editor is a GUI app with an infinite event loop. Never run it in the
foreground: launch it detached, give it a few seconds, check the log, kill it.

Launch it, then confirm by eye, in this order:

1. A red arm to the right and a green arm upward appear over the selected
   sprite, with a white square at its centre. Nothing appears with no selection.
2. Zooming in and out does not change the arms' length or thickness on screen.
3. Dragging the red arm moves the sprite horizontally only; the green arm,
   vertically only; the square, freely.
4. Grabbing an arm near its tip does not teleport the sprite under the cursor.
5. Clicking empty space still changes the selection, and clicking a handle does
   not.

Kill it and report which of the five held. If the arms rotate with the sprite,
`draw` is using the sprite's rotated basis rather than the world axes — the
gizmo in 11a is world-aligned.

- [ ] **Step 6: Commit**

```sh
git add crates/voltra-editor/src/gizmo.rs crates/voltra-editor/src/editor.rs crates/voltra-editor/src/panels/viewport.rs
git commit -m "feat(editor): move a sprite with a translate gizmo"
```

---

### Task 6: Record the decisions

**Files:**
- Modify: `docs/ARCHITECTURE.md`
- Modify: `README.md`
- Modify: this plan

- [ ] **Step 1: Add the decisions**

In `docs/ARCHITECTURE.md`, under "Decisions", after the hot-reload entries:

```markdown
### A line is a quad the shader widens

**WebGPU has no line width, so `voltra_render::lines` uploads each segment as a
quad carrying both endpoints, and `shaders/lines.wgsl` gives it its thickness
after projection.** Verified against the vendored source rather than recalled:
`PrimitiveState` in `wgpu-types-30.0.0/src/render.rs:385` has `topology`,
`strip_index_format`, `front_face`, `cull_mode`, `unclipped_depth`,
`polygon_mode` and `conservative`, and nothing else. `PolygonMode::Line` is
wireframe fill behind an optional feature, not a width.

Widening after projection is what makes the width a number of **pixels**. A
gizmo must be the same size on screen at every zoom, and doing the conversion at
each call site means every caller recomputes pixels-per-world-unit and one of
them gets it wrong. Bevy reached the same shape by adopting `bevy_polyline` into
`bevy_gizmos`.

`pass::draw_lines` is the engine's only pass that **loads** rather than clears.
Every other pass is the single thing drawn into its target that frame; an
overlay is the second.

#### Rejected

- **`PrimitiveTopology::LineList`.** One pixel, always — invisible on a
  high-DPI display and impossible to hit-test generously.
- **Expanding on the CPU.** Fine for a gizmo's twenty segments, and it spreads
  screen-space arithmetic across every caller.
- **`egui::Painter`.** No GPU code at all, and the overlay would live in the UI
  layer: unreachable for a game, in a different coordinate system from the
  scene, and useless to the collision-shape overlay that needs the same camera.

### The gizmo is a persistent tool, and it hit-tests in screen space

**`Tool` is editor state and the gizmo on screen says what a drag will do.**
Unity's model; Godot 2D's too. Blender binds the transform to a gesture instead
— `G`/`R`/`S` start a modal transform — which is faster once learned, invisible
until told, and needs modal input capture `Input` does not have. It layers onto
a working gizmo later; the reverse does not.

**Handles are tested against the pixels they were drawn at**, with a grab margin
wider than the line. Testing in world space gives a target that scales with zoom
while the picture does not, which is a standing Unreal bug report. The centre
square is tested before the arms, because both arms begin inside it and the
smallest target must not be the unreachable one.

**A drag stores the grab point and the original translation, both in world
units,** and each frame sets `start + (cursor − grab)`. Setting the translation
to the cursor teleports the sprite's origin under the pointer the moment the
grab is anywhere but dead centre. World units rather than screen so that
resizing the viewport or zooming mid-drag does not move the sprite.
```

- [ ] **Step 2: Fix the roadmap**

In `README.md`, replace the stage 11 row:

```markdown
| 11a | Translate gizmo, and a line pipeline to draw it | done |
| 11b | Physics: bodies, integration, collision | planned |
```

- [ ] **Step 3: Tick this plan and verify**

Change every `- [ ]` in this file to `- [x]`, having checked each against
`git log --oneline main..HEAD`. Then:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 4: Commit**

```sh
git add docs/ARCHITECTURE.md README.md docs/superpowers/plans/2026-08-10-translate-gizmo.md
git commit -m "docs: record the translate gizmo decisions"
```

---

## Definition of done

- Selecting a sprite draws a red x arm, a green y arm and a white centre square
  over it, at a constant size on screen at every zoom.
- Dragging an arm moves the sprite along that axis only; the square moves it
  freely; neither teleports it to the cursor.
- Clicking empty space still selects; clicking a handle does not change the
  selection.
- Despawning the selection mid-drag ends the drag instead of panicking.
- A minimised window draws nothing and divides by nothing.
- `voltra-render` contains no mention of a gizmo, an axis or a handle;
  `voltra-editor` contains no `wgpu` type.
- `docs/ARCHITECTURE.md` carries both decisions with their rejected
  alternatives; the README shows 11a done and 11b planned.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` are clean.

## Spec coverage

| Spec section | Task |
| --- | --- |
| Line pipeline in `voltra-render`, gizmo-agnostic | Tasks 1, 2 |
| Width in the vertex shader, in pixels | Tasks 1, 2 |
| The pass loads, does not clear | Task 2 |
| The overlay reaches the frame | Task 3 |
| `Tool` is persistent editor state | Task 4 |
| Screen-space hit test, centre wins | Task 4 |
| Drag by delta from a grab offset | Task 4 |
| Gizmo drawn and driven from the viewport | Task 5 |
| Edge cases: no selection, despawn, resize, zero viewport, degenerate segment | Tasks 1, 4, 5 |
| Decisions recorded | Task 6 |
