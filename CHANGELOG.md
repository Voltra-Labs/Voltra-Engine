# Changelog

All notable changes, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/) — before 1.0, a minor version may
break the API.

## [Unreleased]

### Fixed
Found by a pass on Windows with a real GPU (stage 119):
- A process opening wgpu devices from several threads could deadlock in
  NVIDIA's drivers; devices now open one at a time.
- `Game::start` in a game's own binary ran before its starting scene was
  loaded; the scene now opens before the first tick.
- The editor's hierarchy, and deleting or copying a parent, were quadratic
  in its children (7 s a frame for a parent of 100k).
- RON files saved on Windows had CRLF line ends.
- Seven editor icons showed as empty boxes.
- The web page logged a `favicon.ico` 404 in desktop browsers.
- `cargo xtask guide` ran WSL's bash on Windows.

## [0.3.0] — 2026-11-24

The plan split this into 0.2 (2D polish, stages 88–110) and 0.3
(networking, 111–118); both landed together, so they ship as one release.
Every stage is a row in the README's roadmap and a decision in
`docs/ARCHITECTURE.md`; the guide (`docs/guide/`) covers each from a
user's side.

### 2D polish
- **Options and windows:** rebound controls saved on every platform, the
  web included; fullscreen, window size, vsync and monitor modes from a
  tick; a reusable options screen (display, audio, rebinding) that new
  games get from `cargo xtask new-game`.
- **Text:** shaped with HarfBuzz (`rustybuzz`) — ligatures, Arabic,
  Devanagari — with a fallback font chain per language, bidirectional
  order, Unicode line breaking, and rich text markup (`[b]`, `[color]`,
  `[size]`, `[font]`, `[icon]`).
- **UI:** dropdowns, tooltips, tabs, and themes (`.theme.ron`) swapped at
  run time.
- **Assets and levels:** a texture packer (`cargo xtask pack-atlas`, also in
  the editor) with trimmed regions and pivots; an atlas editor; a tile
  palette with stamps, rotation, flood fill and an eyedropper; per-tile
  collision shapes (slopes, halves, polygons) and custom data.
- **Effects and animation:** particles with curves, gradients, emitter
  shapes, spin and flipbooks on CPU and GPU, edited live; `Line2D`,
  `Polygon2D` and `Trail2D`; 2D bones with two-bone IK; skinned sprite
  meshes with painted weights.
- **Audio:** streamed clips with loop points, equal-power music
  crossfades, and bus effects — filters, delay, reverb, ducking — laid out
  in `default.buses.ron`.
- **Editor:** prefab edit mode, and search in the hierarchy and assets
  with Unity's `t:` syntax.
- **Scale:** a benchmark suite (`voltra-bench`, `cargo xtask bench`) with a
  regression check in CI, the optimisations it pointed at, and big worlds
  streamed by region around anchors, with loading screens.

### Networking
- **`voltra-net`:** UDP and WebSocket transports (the browser included),
  connections with a version handshake, reliable and unreliable channels,
  fragmentation, typed messages; untrusted input bounded throughout.
- **`voltra-server`:** a headless authoritative server; host, join and
  listen servers from a tick.
- **`voltra-replication`:** replicated components with snapshots, deltas
  and a per-client byte budget; interpolation through a jitter buffer;
  prediction and reconciliation of the local player.
- **Editor:** play with up to three clients, link conditions changed live,
  net counters in the profiler.
- **`games/arena`:** a networked sample for two to four players, native and
  on the web, played end to end in CI.

## [0.1.0] — 2026-11-24

The first release of the Rust rewrite: a 2D engine an outsider can make and
ship a game with. The README's roadmap lists every stage; in short:

### Rendering
- Batched sprites with atlases, flips and frame animation; tilemaps with
  layers, animated tiles and auto-tiling (16-frame and 47-tile blob).
- Sorting layers and painter's-order drawing; parallax layers, repeating
  backgrounds; per-quad and per-chunk culling.
- 2D lights: point, spot and global lights, shadows from colliders, polygons
  or sprite silhouettes, normal maps, light layers and cookies.
- A render graph for post-processing (grading, bloom); CPU and GPU particles
  with collisions.
- Sprite materials: a game's own WGSL fragment with uniforms.
- Several cameras: split screen, render textures, pixel-perfect views.
- Runs on Vulkan, Metal, DirectX 12, OpenGL, WebGPU and WebGL2.

### Gameplay
- An in-house ECS; RON scenes, prefabs with overrides, additive scene loads,
  `Persistent` entities, save slots.
- Physics: box, circle, capsule and convex polygon bodies, layers, sensors,
  collision events, ray/shape/point queries, joints, continuous collision,
  sleeping islands, `CharacterBody2D` with slopes and steps, one-way
  platforms.
- Input actions from a map file: keys, pads, analog dead zones, rebinding,
  touch (fingers as pointers, virtual sticks and buttons).
- Audio: a mixer with buses, sources with distance, one-shots, WAV, OGG,
  FLAC and MP3.
- Property animation clips, state machines, tweens and timers; a camera rig
  with follow and shake; grid navigation with A*.
- Screen-space UI: text with wrapping and string tables per language,
  images and 9-slice panels, rows and columns, buttons, toggles, sliders,
  text fields with IME, scrolling lists — one focus for mouse, keys, pads
  and touch.

### Games and tools
- A game is a crate: the editor loads its library, plays it, and hot
  reloads it mid-play; `cargo xtask new-game` makes one.
- The editor: hierarchy, inspector, asset browser, gizmos with grid
  snapping, multi-selection, copy and paste, undo, a timeline, a profiler
  with GPU pass times, a console and project settings.
- The player binary; `cargo xtask dist` for native builds and
  `cargo xtask dist --web` for a static folder any host serves.
- An MCP server (`voltra-mcp`) and a live editor bridge for AI clients.
- A user guide (`docs/guide/`), checked by CI; three sample games:
  `platformer`, `breakout` and `dodge`; every public item documented.

[0.3.0]: https://github.com/Voltra-Labs/Voltra-Engine/releases/tag/v0.3.0
[0.1.0]: https://github.com/Voltra-Labs/Voltra-Engine/releases/tag/v0.1.0
