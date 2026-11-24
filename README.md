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
cargo run -p voltra-player -- assets/scenes/lights.ron    # 2D lights and shadows
cargo xtask dist                # package build.ron into dist/<name>/ to hand out
cargo xtask dist --manifest platformer.build.ron   # a game with its own code
cargo xtask new-game my-game                      # start a game of your own: games/my-game
cargo xtask dist --web          # the same build for the browser (docs/WEB.md)
cargo run -p voltra-mcp -- --project .             # MCP server for an AI client (docs/MCP.md)
cargo run -p voltra-editor -- --bridge             # editor an MCP client can drive live
cargo build --workspace         # build everything
cargo test --workspace          # run tests
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

The editor opens with a hierarchy on the left, an inspector on the right and the
scene in the middle. `Scene ▸ Spawn sprite` adds an entity; selecting one in the
hierarchy lets the inspector edit its transform and colour. An entity can also
be selected by clicking it in the viewport; clicking empty space clears the
selection. Ctrl-click adds or removes one, Shift-click in the hierarchy takes
the rows between, and a left drag on empty ground draws a box that selects
what it encloses; the gizmo, on the last one picked, moves, turns and scales
them all about it, the inspector edits that one, and `Delete` (the button or
the key) removes them all as one undo entry. `Ctrl+D` duplicates the selection
in place, and `Ctrl+C` / `Ctrl+X` / `Ctrl+V` copy, cut and paste it — through
the system clipboard, so a copy pastes into another scene or editor window.

`Scene ▸ New`, `Open…`, `Save` and `Save As…` (`Ctrl+N`, `Ctrl+O`, `Ctrl+S`,
`Ctrl+Shift+S`) work on any scene under the asset root; double-clicking a
scene in the asset browser opens it, and `voltra-editor scenes/level.ron`
starts with one open. The window title names the scene, with a `*` while it
has unsaved changes — undoing back to the last save clears it — and New,
Open and closing the window ask "save changes?" first.

`Window ▸ Profiler` shows what each frame costs: a graph of frame times
against the 60 and 30 fps lines, and a tree of the engine's scopes (update,
tick, physics and its steps, draw, ui, present) with calls, average, worst
and own milliseconds, each render pass's time on the GPU (where the
adapter can time it), then counters such as entities and contacts. Press
Play to profile the game rather than the editor at rest.

A prefab dropped from the asset browser stays linked to its file: editing the
file (on disk or with the inspector's **Apply to prefab** on an edited instance)
updates every instance, except for the components an instance has changed — the
inspector lists those as overrides, each with a **Revert**.

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
entry. An `.atlas.ron` — a sheet's slicing — shows its first frame as its
tile and drops in as a sprite on frame 0 with the sheet it names already
assigned. The inspector picks the frame, mirrors it on either axis, sizes it
from a pixels-per-unit, and adds an animation: a list of frames and a rate that
plays while the scene is playing. The listing re-reads itself once a second,
so a file saved from an image editor shows up without a refresh.

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
While editing, `Ctrl+Z` / `Ctrl+Y` undo and redo scene edits. `Scene ▸ New`,
`Open`, `Clear` and `Save` stop play first, so none of them acts on a
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

A `Joint2D` holds two bodies together, or one to the world: a revolute hinge
(with an optional angle limit and motor), a distance joint (a rod, a spring, or
a rope that goes slack), or a weld. `Physics ▸ Join selection` hinges the last
picked body to the other selected one or to the world, and the inspector edits
the rest; the collider overlay draws every joint.

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
cargo run -p voltra-platformer                   # A/D to walk, Space to jump
# The same game in the editor: built together, then loaded and played there.
cargo build -p voltra-editor -p voltra-platformer
cargo run -p voltra-editor -- --game platformer.build.ron scenes/platformer.ron
# Edit games/platformer/src while it plays: it is rebuilt and swapped in.
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
├── build.ron             # the build `cargo xtask dist` packages
├── assets/               # runtime assets (shaders, textures, scenes)
├── crates/
│   ├── voltra-ecs/       # entities and components — zero dependencies
│   ├── voltra-jobs/      # worker pool, scoped jobs that borrow — std only
│   ├── voltra-profile/   # where a frame's time goes: scopes, counters — std only
│   ├── voltra-assets/    # handles, paths, the texture cache, hot reload
│   ├── voltra-render/    # GPU layer: passes, render targets, egui backend
│   ├── voltra-scene/     # Transform, Sprite, and the geometry they become
│   ├── voltra-physics/   # rigid bodies, integration, contact detection
│   ├── voltra-nav/       # navigation grids baked from colliders, A* paths
│   ├── voltra-bridge/    # commands to a running editor over a loopback socket
│   ├── voltra-mcp/       # binary: the MCP server, a project's tools for an AI
│   ├── voltra-core/      # platform layer: event loop, window  (owns winit)
│   ├── voltra-editor/    # binary: the editor and its panels
│   ├── voltra-player/    # binary: runs a scene, or the build beside it
│   └── xtask/            # `cargo xtask dist`: packages a build
├── games/
│   └── platformer/       # sample game crate: the editor loads it, its binary ships it
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
| 22 | Sprite sheets: atlases, regions, flips, frame animation | done |
| 23 | Audio: mixer, buses, sources, hot reload, audition | done |
| 24 | Prefabs: spawn from a tick, author in the editor | done |
| 25 | Tilemaps: layers, generated colliders, paint tool | done |
| 26 | Text and screen-space UI: fonts, glyph atlas, `UiRect` HUD | done |
| 27a | Input actions: map file, action edges, rebinding | done |
| 27b | Gamepads: pad bindings, analog dead zones | done |
| 28 | Sweep-and-prune broad phase, sleeping islands | done |
| 29 | Export: `build.ron` manifest and `cargo xtask dist` | done |
| 30 | Scene changes and additive loading from a tick, `Persistent` | done |
| 31 | Save games: typed slots in a per-user folder | done |
| 32 | Parallax layers, drawn and picked through the camera | done |
| 33 | Camera culling: per quad, per tilemap chunk | done |
| 34 | Vertex alpha, particle emitters | done |
| 35 | UI buttons: pointer, focus navigation, `tick.ui` | done |
| 36 | `dist` for games with their own binary; shared logging | done |
| 37 | Repeating parallax backgrounds | done |
| 38 | Animated tiles | done |
| 39 | Auto-tiling: 16-frame terrains, painted from the editor | done |
| 40 | Auto-tiling: the 47-tile blob with corners | done |
| 41 | Sorting layers: named bands above `sort_order` | done |
| 42 | Sorting layers from `default.layers.ron`; project files hot reload | done |
| 43 | Post-processing: a graded pass between the world and the HUD | done |
| 44 | Tilemap chunk mesh cache; the frame batch keeps its buffers | done |
| 45 | GPU particles: compute-stepped rings, drawn in painter's order | done |
| 46 | Job system (`voltra-jobs`); batch sort keyed once | done |
| 47 | Render graph for the post chain; bloom | done |
| 48 | Dense `WorldTransforms`; the parallel batch pre-pass measured and removed | done |
| 49 | Scene textures decoded across the job pool | done |
| 50 | GPU particle emitters culled by a conservative bound | done |
| 51 | Particle collisions against the scene's colliders, CPU and GPU | done |
| 52 | 2D lights: point lights and an ambient, a light map in the render graph | done |
| 53 | 2D shadows: casters from colliders or polygons, a polar shadow map per light | done |
| 54 | A lit sample scene, drawn by a test that checks its shadows | done |
| 55 | Shadow caster polygons edited in the viewport | done |
| 56 | Spot lights: a cone on a point light, aimed by its entity | done |
| 57 | Normal maps on sprites, shaded by lights with a height | done |
| 58 | Light layers: a light lights only what shares one of its bits | done |
| 59 | Shadows from a sprite's silhouette, traced from its alpha | done |
| 60a | Spot lights culled, drawn and shadowed by their cone's reach | done |
| 60b | Normal maps on tilemap layers, laid out as the sheet | done |
| 60c | Light layers on text and particle emitters, GPU ones included | done |
| 61a | Shadow casters block only the lights of their light layers | done |
| 61b | Global lights: the ambient lifted for the pixels of their layers | done |
| 62 | Light cookies: an image over a light that tints and shapes it | done |
| 63a | Multi-selection: Ctrl/Shift clicks, box select, outlines, Delete | done |
| 63b | The gizmo moves, turns and scales the whole selection | done |
| 63c | Duplicate, copy and paste, through the system clipboard as text | done |
| 64 | Prefab instances follow their file: overrides, Apply, Revert | done |
| 65 | Joints: revolute (limit, motor), distance (rod, spring, rope), weld | done |
| 66a | Property animation: `.anim.ron` clips of eased keys, `AnimationPlayer` | done |
| 66b | Timeline: tracks, keys on a time strip, eases, preview pose, Save/Revert | done |
| 67 | Shape casts; `CharacterBody2D` and `move_and_slide`: slopes, steps, snap | done |
| 68 | One-way platforms for bodies and characters; characters ride moving floors | done |
| 69 | Camera rig: follow (smoothing, dead zone, look-ahead, bounds) and trauma shake | done |
| 70 | Animation state machines: `.animator.ron` parameters, states, transitions | done |
| 71 | Navigation (`voltra-nav`): grids baked from colliders, A* paths, a top-down chaser | done |
| M1 | Engine prep for an MCP server: components by name with templates, headless sessions, screenshots, captured logs | done |
| M2 | `voltra-mcp`: scenes, assets, headless runs with screenshots, build and test — over stdio | done |
| M3 | The live editor bridge: an AI edits, plays and screenshots a running editor, each edit one undo entry | done |
| 72 | Scenes in the editor: New, Open, Save As, unsaved-changes prompt, title, CLI scene | done |
| 73 | Profiler (`voltra-profile`): frame scopes and counters, the editor's Profiler window, MCP profiles | done |
| 74 | GPU time per render pass in the profiler, read back without waiting | done |
| W1 | The engine builds for `wasm32`; every asset read through one funnel | done |
| W2 | A scene runs in a browser: the asset pack, WebGPU with a WebGL2 fallback | done |
| W3 | Sound and input in the browser; games with their own code on the web | done |
| W4 | `cargo xtask dist --web`: a folder any static server hosts | done |
| 75a | Capsule and convex polygon colliders, in every query, a fit to the sprite | done |
| 75b | Continuous collision: fast bodies stop at walls, bullets at bodies too | done |
| 76 | The game as a plugin: the editor loads a game crate's library and Play runs it | done |
| 77 | Hot reload: an edit to the game's code is rebuilt and swapped in mid-play | done |
| 78 | `cargo xtask new-game`: a game crate to play, edit in the editor and package | done |
| 79 | UI images: 9-slice panels, image buttons, rows and columns | done |
| 80 | UI controls: slider, toggle, text field with IME, scrolling lists | done |
| 81 | Text wrapping and localization: string tables per language | done |
| 82 | Tweens and timers from the tick, gone with their entity | done |
| 83 | Sprite materials: a WGSL fragment and its uniforms, on every backend | done |

## Contributing

Branch off `main`, keep `cargo clippy -- -D warnings` clean, and follow
[Conventional Commits](https://www.conventionalcommits.org/). Conventions are
documented in [docs/CONVENTIONS.md](docs/CONVENTIONS.md).

## License

MIT — see [LICENSE](LICENSE).
