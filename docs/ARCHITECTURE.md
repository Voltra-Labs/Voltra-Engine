# Architecture

## Principles

1. **One responsibility per crate.** If a subsystem can be described without
   mentioning another subsystem, it is its own crate.
2. **Dependencies point one way.** Lower layers never know about higher ones.
   `voltra-render` must never learn what a window or an entity is.
3. **Depend on the abstraction wgpu already gives you.** `voltra-render` takes a
   `wgpu::SurfaceTarget`, not a `winit::Window`. That single choice is what keeps
   the render layer testable and headless-capable.
4. **Grow crates on demand.** A new crate is created when code exists to fill it,
   never in advance. Empty scaffolding rots.

## Layers

```
        ┌──────────────────┐
        │  voltra-editor   │  binary — wires everything together
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │   voltra-core    │  platform: event loop, window, input, time
        │  (owns winit)    │
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │  voltra-render   │  GPU: device, surface, pipelines, passes
        │  (owns wgpu)     │
        └──────────────────┘
```

**Rule:** exactly one crate may depend on `winit` (`voltra-core`) and exactly one
may depend on `wgpu` (`voltra-render`). Everything else consumes them through
re-exports, so a version bump is a one-line change.

### Current crates

| Crate | Owns | Key types |
| --- | --- | --- |
| `voltra-render` | GPU device, swapchain, frame recording | `GpuContext`, `Renderer` |
| `voltra-core` | Event loop, OS window, application lifecycle | `App`, `WindowConfig` |
| `voltra-editor` | Editor binary | `main` |

### Planned crates

Added only when there is code to put in them:

| Crate | Purpose | Blocked on |
| --- | --- | --- |
| `voltra-ecs` | In-house entity/component storage | stage 6 |
| `voltra-scene` | Scene graph, hierarchy, serialization | stage 6 |
| `voltra-assets` | Loading, caching, hot reload | stage 8 |
| `xtask` | Repo automation written in Rust instead of shell | when scripts appear |

## Frame flow

```
winit event loop  (voltra-core::App)
  │
  ├─ Resumed          → create Window, build Renderer
  ├─ Resized(size)    → Renderer::resize → GpuContext reconfigures surface
  └─ RedrawRequested  → Renderer::render
                          │
                          ├─ GpuContext::acquire   → Option<SurfaceTexture>
                          ├─ record command encoder (clear pass today)
                          ├─ queue.submit
                          └─ GpuContext::present
                          then request_redraw → continuous loop
```

`GpuContext::acquire` returns `Option` on purpose: surface loss, resize races
and minimised windows are *normal*, not errors. `Outdated` and `Lost`
reconfigure and skip the frame; `Timeout` and `Occluded` skip silently.

## Decisions

### No ECS crate (`hecs`, `bevy_ecs`, …)

Deliberate. Writing our own is the point of the project. The trap is that a
Bevy-style archetype ECS in Rust needs `UnsafeCell`, `TypeId` erasure and manual
aliasing proofs in every query — a subproject, not a module.

**Therefore the first ECS is the simple one:** generational `Entity(index,
generation)` handles plus one dense storage per component type, zero `unsafe`.
Archetypes come later, driven by profiler output, not by aesthetics.

### `glam` instead of a `voltra-math` crate

The no-frameworks rule targets engine architecture — ECS, scene graph, render
graph — not leaf libraries. `glam` is a leaf: SIMD vectors and matrices with no
opinion about how a game is structured, so depending on it costs no design
freedom. Hand-written matrix maths would teach nothing and is prime territory
for silent sign and transpose errors.

It is used directly in `voltra-render` and re-exported from there. A
`voltra-math` facade wrapping it would be a crate with no code in it.

### wgpu over raw Vulkan or OpenGL

The C++ engine was OpenGL-only. wgpu gives Vulkan/DX12/Metal/GL/WebGPU from one
codebase, enforces resource lifetimes through `Drop`, and validates at API level
— which removes the whole class of "forgot to delete the GPU object" bugs that
the C++ tree kept hitting.

### wgpu 30 API notes

wgpu 30 broke almost every tutorial published online (they target v25 and older).
Verified against the crate source, not from memory:

| Thing | wgpu 30 |
| --- | --- |
| Instance | `Instance::new(InstanceDescriptor::new_without_display_handle_from_env())` — takes the descriptor **by value**, and it has no `Default` |
| Adapter/device | `request_adapter` / `request_device` return `Result`, and `request_device` takes only the descriptor |
| Frame acquire | `get_current_texture()` returns the `CurrentSurfaceTexture` enum, not a `Result` |
| Present | `queue.present(frame)` — `SurfaceTexture::present` was removed |
| Render pass | `RenderPassDescriptor` requires `multiview_mask`; `RenderPassColorAttachment` requires `depth_slice` |
| Pipeline layout | `bind_group_layouts` is `&[Option<&BindGroupLayout>]`, and `push_constant_ranges` is replaced by `immediate_size: u32` |
| Pipeline | `VertexState::buffers` is `&[Option<VertexBufferLayout>]` — the entries are wrapped in `Option` |
| Device poll | `PollType::Wait` is a struct variant; use `PollType::wait_indefinitely()` |
| Buffer readback | `Buffer::get_mapped_range` returns `Result` |

When touching wgpu, read the vendored source under
`~/.cargo/registry/src/index.crates.io-*/wgpu-30.0.0/src/api/` or query Context7
rather than trusting a tutorial.
