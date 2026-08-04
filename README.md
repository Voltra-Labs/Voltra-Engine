# Voltra Engine

[![CI](https://github.com/Voltra-Labs/Voltra-Engine/actions/workflows/ci.yml/badge.svg)](https://github.com/Voltra-Labs/Voltra-Engine/actions/workflows/ci.yml)

A 2D/3D game engine written in Rust on top of [`wgpu`](https://github.com/gfx-rs/wgpu) (Vulkan, Metal, DX12, GL, WebGPU) and [`winit`](https://github.com/rust-windowing/winit).

> **Status: early rewrite.** Voltra was originally a C++/OpenGL engine. The Rust
> rewrite starts from an empty slate; the C++ tree is preserved on the `main`
> branch history and tagged `v0-cpp-final`. Today it runs an editor: an
> in-house ECS, textured sprite batching, a 2D camera, and an egui interface
> with a hierarchy, an inspector and a live viewport.

## Design stance

- **No engine framework dependencies.** No ECS crate, no scene-graph crate. Core
  systems are written in-house so the architecture stays ours. Well-understood
  leaf libraries (math, serialization, physics, UI widgets) are fair game — but
  the wgpu backend that draws egui is ours, because the official one is pinned
  to an older wgpu.
- **wgpu, not raw Vulkan.** One backend-agnostic API, five platforms, no
  hand-rolled swapchain code.
- **Workspace-first.** Every subsystem is its own crate with an explicit
  dependency direction. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Requirements

| Tool | Version |
| --- | --- |
| Rust | 1.97+ (stable) |
| Platform toolchain | MSVC Build Tools on Windows; `cc` + Vulkan drivers on Linux |

Install Rust with [rustup](https://rustup.rs). No other system dependency is
needed — everything else comes from Cargo.

## Build and run

```sh
cargo run -p voltra-editor      # launch the editor
cargo build --workspace         # build everything
cargo test --workspace          # run tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

The editor opens with a hierarchy on the left, an inspector on the right and the
scene in the middle. `Scene ▸ Spawn sprite` adds an entity; selecting one in the
hierarchy lets the inspector edit its transform and colour.

Camera controls, active whenever egui is not using the input itself:

| Input | Action |
| --- | --- |
| `W` `A` `S` `D` | Pan the camera |
| Scroll wheel | Zoom |
| `R` | Reset the camera |

Useful environment variables (read by `wgpu`):

```sh
WGPU_BACKEND=vulkan|dx12|gl     # force a backend
RUST_LOG=voltra_render=debug    # per-crate log filtering
```

## Layout

```
Voltra-Engine/
├── Cargo.toml            # virtual manifest: workspace + shared dep versions
├── CLAUDE.md             # instructions for AI agents working in this repo
├── assets/               # runtime assets (shaders, textures, scenes)
├── crates/
│   ├── voltra-ecs/       # entities and components — zero dependencies
│   ├── voltra-render/    # GPU layer: passes, render targets, egui backend
│   ├── voltra-scene/     # Transform, Sprite, and the geometry they become
│   ├── voltra-core/      # platform layer: event loop, window  (owns winit)
│   └── voltra-editor/    # binary: the editor and its panels
└── docs/
    ├── ARCHITECTURE.md   # layering, crate graph, planned crates
    └── CONVENTIONS.md    # naming, file/folder rules, code style
```

The workspace root is a *virtual manifest* — it has no `src/`, so no crate is
privileged and plain `cargo build` covers everything.

## Roadmap

| Stage | Scope | State |
| --- | --- | --- |
| 1 | Window + wgpu surface + clear pass | done |
| 2 | Shaders, render pipeline, first triangle | done |
| 3 | Vertex/index buffers | done |
| 4 | Uniforms, bind groups, 2D camera | done |
| 5 | Textures and samplers | done |
| 6 | In-house ECS | done |
| 7 | Transforms, sprites, batched rendering from the world | done |
| 8 | Editor UI: hierarchy, inspector, viewport | done |
| 9 | Scene serialization and the asset pipeline | next |
| 10 | Gizmos, picking, physics | planned |

## Contributing

Branch off `main`, keep `cargo clippy -- -D warnings` clean, and follow
[Conventional Commits](https://www.conventionalcommits.org/). Conventions are
documented in [docs/CONVENTIONS.md](docs/CONVENTIONS.md).

## License

MIT — see [LICENSE](LICENSE).
