# Changelog

All notable changes, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/) — before 1.0, a minor version may
break the API.

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

[0.1.0]: https://github.com/Voltra-Labs/Voltra-Engine/releases/tag/v0.1.0
