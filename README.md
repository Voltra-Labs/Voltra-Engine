# Voltra Engine

[![CI](https://github.com/Voltra-Labs/Voltra-Engine/actions/workflows/ci.yml/badge.svg)](https://github.com/Voltra-Labs/Voltra-Engine/actions/workflows/ci.yml)

A 2D/3D game engine written in Rust on top of [`wgpu`](https://github.com/gfx-rs/wgpu) (Vulkan, Metal, DX12, GL, WebGPU) and [`winit`](https://github.com/rust-windowing/winit).

> **Status: early rewrite.** Voltra was originally a C++/OpenGL engine. The Rust
> rewrite starts from an empty slate; the C++ tree is preserved on the `main`
> branch history and tagged `v0-cpp-final`. Today it runs an editor: an
> in-house ECS, textured sprite batching, a 2D camera, and an egui interface
> with a hierarchy, an inspector, an asset browser and a live viewport.

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
- **Prior art beats invention.** Writing the core systems in-house does not mean
  guessing at them. Where Unity, Unreal, Godot or Bevy have already settled a
  question — where the editor camera lives, how input is scoped to a viewport,
  what a scene file holds — that answer is researched first and adopted or
  rejected for a stated reason. The reasons live in the "Decisions" section of
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

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
cargo run -p voltra-player -- assets/scenes/sandbox.ron   # run a scene, no editor
cargo build --workspace         # build everything
cargo test --workspace          # run tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

The editor opens with a hierarchy on the left, an inspector on the right and the
scene in the middle. `Scene ▸ Spawn sprite` adds an entity; selecting one in the
hierarchy lets the inspector edit its transform and colour. An entity can also
be selected by clicking it in the viewport; clicking empty space clears the
selection. `Scene ▸ Save` and `Scene ▸ Open` write and read
`assets/scenes/scene.ron`.

The hierarchy lists the scene as a tree. An entity's `Name` is what a row says —
the inspector renames it, and an unnamed one falls back to its index. Dragging a
row onto another makes it that entity's child; dragging it onto the empty space
below the tree makes it a root again. A child's transform is **relative to its
parent**, so moving, turning or scaling a parent carries its children with it,
and `Delete` on a parent deletes everything under it. A reparent keeps the
entity where it is on screen, and is one undo entry.

The inspector edits what a collider touches: a `sensor` checkbox and a toggle
per collision layer, on the row it is on and on the rows it looks at. Sensors
are drawn in their own colour in the debug overlay, since a trigger volume is
otherwise an invisible rectangle.

Physics and parenting do not mix yet: an entity carrying a `RigidBody` or a
`Collider` cannot be parented, because the solver reads its transform as a
world-space value. The refusal is logged rather than silent.

The bottom panel is the asset browser: everything under the asset root, one
tile per entry, a path bar to walk into a directory and `..` to leave it. A PNG
shows its own pixels; a file this engine has no loader for is listed but dimmed.
Dragging a texture onto the scene adds a sprite where it was dropped, named after
the file and sized to the texture's aspect ratio; dragging it onto the
inspector's `Texture` field assigns it to the selected sprite. Both are one undo
entry. The listing re-reads itself once a second, so a file saved from an image
editor shows up without a refresh.

The left of the toolbar holds the transform tools, and the gizmo drawn over the
selection says what a drag will do before it is started. The gizmos work in
world space, so a child's handles sit where the child is drawn.

| Tool | Key | Gizmo | Drag |
| --- | --- | --- | --- |
| Move | `W` | Two arms on the world axes | An arm moves along it, the centre square moves freely |
| Rotate | `E` | A ring | Anywhere on the ring turns the entity about its own origin |
| Scale | `R` | Two arms on the entity's own axes, square-tipped | An arm grows that local axis, the centre square grows both |

A scale is a ratio of distances from the origin, so dragging a handle to twice
its distance doubles the entity whatever size it already was, and dragging it
through the origin mirrors. A rotate keeps a running total, so a drag that
carries past half a turn keeps turning instead of snapping back. Each drag is
one undo entry, whatever it passes through on the way.

The middle of the toolbar is the transport. The editor starts in edit mode
and simulates nothing until Play is pressed; the viewport is tinted while it is
not editing, so the mode is never in doubt.

| Button | Action |
| --- | --- |
| ▶ / ⏸ | Play, or pause without leaving play |
| ⏭ | Run exactly one fixed physics step (while paused) |
| ⏹ | Stop: put the scene back as it was when play began |

Stop discards every change made while playing — it is a restore, not an undo.
While editing, `Ctrl+Z` / `Ctrl+Y` undo and redo scene edits. `Scene ▸ Open`,
`Scene ▸ Clear` and `Scene ▸ Save` stop play first, so none of them acts on a
mid-flight scene.

Camera controls. These belong to the editor, not the engine, and a game built
on `voltra-core` moves its own camera. Scroll and the keys act only while the
pointer is over the scene; a middle-drag pan, once started, keeps going even
after the pointer leaves it. `WASD` pans only with the right mouse button held,
as it does in Unity and Unreal, because the bare keys belong to the tools.

| Input | Action |
| --- | --- |
| Middle-drag | Pan |
| Scroll wheel | Zoom about the cursor |
| Right-hold + `W` `A` `S` `D` | Pan |
| `F` | Reset the camera |

The player is what a shipped game is: a scene, a window, and no editor in the
binary at all. It takes the scene as its one argument, simulates from the first
frame, and draws through the `Camera` the scene authored — with no active
camera it keeps the default framing and says so in the log.

```sh
voltra-player [--asset-root DIR] [--title TEXT] [--size WxH] <SCENE>
```

A game is written against the loop with two hooks. `with_update` runs once per
frame and is where input belongs — an edge is only seen once there.
`with_fixed_update` runs before each physics step, with `delta` always the fixed
step, and is where velocities and forces belong. Both are handed a `Tick`: the
world, the input, the delta that applies, the last step's contacts, and the
collision events neither hook has been given yet.

```rust
App::new(config)
    .with_simulation()
    .with_update(|tick| { /* input, cameras, anything per frame */ })
    .with_fixed_update(|tick| { /* velocities, forces, events, per step */ })
    .run();
```

What a collider touches is two bitmasks, `CollisionLayers { layers, mask }`: a
pair interacts only when each side is on a layer the other looks at, so there
is no way for one of them to detect the other and be walked through in return.
A collider marked `Sensor` is detected and never solved — it reports what
enters it and stops nothing, which is what a coin, a checkpoint or a damage
zone is. Both are absent by default, meaning "every layer, and solid".

`Tick::events` is the stream of what began and ended touching, sensors
included. Each hook is handed every event exactly once, so a pickup can be
taken in the fixed tick without arriving twice on a fast frame. `Tick::contacts`
stays the state — what the solver resolved this step — and never holds sensors.

The world also answers questions without being touched:

```rust
let hit = query::ray(tick.world, foot, Vec2::NEG_Y, 0.7, QueryFilter::new().excluding(player));
let inside = query::overlap_aabb(tick.world, min, max, QueryFilter::new());
```

A ray reports the nearest hit — its point, its normal and its distance — and a
ray that starts inside a shape hits it at distance zero rather than reporting
thin air. Queries skip sensors unless asked for them.

```sh
cargo run -p voltra-core --example platformer   # A/D to walk, Space to jump
```

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
│   ├── voltra-assets/    # handles, paths, the texture cache, hot reload
│   ├── voltra-render/    # GPU layer: passes, render targets, egui backend
│   ├── voltra-scene/     # Transform, Sprite, and the geometry they become
│   ├── voltra-physics/   # rigid bodies, integration, contact detection
│   ├── voltra-core/      # platform layer: event loop, window  (owns winit)
│   ├── voltra-editor/    # binary: the editor and its panels
│   └── voltra-player/    # binary: runs a scene with no editor in it
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
| 9 | Scene serialization | done |
| 10 | Picking: click to select, stable draw order | done |
| 11a | Translate gizmo, and a line pipeline to draw it | done |
| 11b-1 | Rigid bodies, fixed-step integration, contact detection | done |
| 11b-2 | The contact solver: bodies stop overlapping and stack | done |
| 11b-3 | Rotation, oriented boxes, two-point manifolds | done |
| 12a | Asset store: handles, paths, cache | done |
| 12b | Textures per sprite, batched by texture | done |
| 12c | Hot reload: watch, debounce, swap under a stable handle | done |
| 13 | Play mode: snapshot on Play, restore on Stop | done |
| 14 | Undo and redo: per-entity records, one entry per interaction | done |
| 15 | Rotate and scale gizmos, `W`/`E`/`R` tools | done |
| 16 | Names, parent/child transforms, the hierarchy as a tree | done |
| 17 | Asset browser: a dock of the asset root, drag to place | done |
| 18 | The game camera: a scene component, and a game view | done |
| 19 | The player: a second binary that runs a scene with no editor | done |
| 20 | The game's turn: a per-frame tick, a fixed tick, and input | done |
| 21 | Gameplay physics: layers, sensors, collision events, queries | done |

## Contributing

Branch off `main`, keep `cargo clippy -- -D warnings` clean, and follow
[Conventional Commits](https://www.conventionalcommits.org/). Conventions are
documented in [docs/CONVENTIONS.md](docs/CONVENTIONS.md).

## License

MIT — see [LICENSE](LICENSE).
