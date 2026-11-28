# Changelog

All notable changes, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/) — before 1.0, a minor version may
break the API.

## [Unreleased]

0.5 adds 3D beside 2D (`docs/superpowers/specs/0.5-3d.md`). Notes kept as
stages land, for the 0.5 entry.

### Breaking
- **Paired names** (stage 132): `Transform`, the scene's `Camera`,
  `Sprite`, `RigidBody`, `Collider` and `WorldTransforms` are
  `Transform2D`, `Camera2D`, `Sprite2D`, `RigidBody2D`, `Collider2D` and
  `WorldTransforms2D`; the hierarchy's `world_matrix`, `world_transform`,
  `set_world_transform` and `parent_matrix` end in `_2d`; in
  `voltra-render`, `Camera2D` is `View2D`, `Vertex` `Vertex2D`, `Mesh`
  `Mesh2D`. The old names compile for 0.5 with a deprecation warning
  (`View2D`'s excepted) and go in 0.6. Clip properties
  `Translation2D`/`Rotation2D`/`Scale2D` and
  `PointLight2DColor`/`Intensity`/`Radius` read their old names too.
- **Scene format version 2**: components under their new names. A
  version 1 scene, prefab or clip still loads, renamed as it is read
  (overrides included); `cargo xtask migrate [--check]` rewrites a project.
  A 0.4 file holding a game's own component under a name 0.5 gives the
  engine (`Sprite2D`, …) is refused with a message to rename it.
- **Replication wire version 4**: the schema sends the new names, so a 0.4
  peer is refused at the handshake instead of connecting and matching no
  transform.
- **Replication wire version 5** (stage 134): `Transform3D` replicates,
  interpolated, by default, so a peer of the version before is refused
  rather than leaving every 3D entity where it spawned.
  `set_interest_points` takes `Vec2`s, `Vec3`s or `InterestPoint`s, and
  `interest_points` answers `InterestPoint`s; `InterestGrid::relevant`
  takes `InterestPoint`s.
- **A component name registered twice is refused**: the first type keeps
  it, the second is not registered, an error is logged, and
  `ComponentRegistry::refused` lists it.

## [0.4.0] — 2026-11-27

0.4 closes, stage by stage (119–131), the gaps `docs/ARCHITECTURE.md`
listed under each subsystem's "Not yet" that a game author hits first.
Every stage is a row in the README's roadmap and a decision in
`docs/ARCHITECTURE.md`; the guide (`docs/guide/`) covers each from a
user's side. 3D is not part of it: it comes next, with a plan of its own.

### 2D
- **Animation:** animator transitions crossfade over a `duration`, an
  interrupted fade fading on from what shows, and `exit_time` counts
  loops; blend trees (`Blend1D`, `Blend2D` simple directional) whose
  clips play in step; animator layers with masks, a weight a game can
  change, and override or additive blending. Clips are sampled apart
  from writing, so every mix goes through one N-input blend.
- **Timeline:** record mode keys what is edited under the previewing
  player; keys copied, cut and pasted at the playhead; the clip's own
  undo history.
- **Physics:** wheel, prismatic and mouse joints, a revolute's return
  spring, and `break_force` / `break_torque` on every joint, reported
  once through `Tick::broken_joints`. In play the editor drags bodies
  with a mouse joint. Characters push dynamic bodies (`push_force`),
  keep their speed on slopes, and turn with a turning floor;
  `Tick::gravity` for games moving them.
- **Camera:** the view turns with its camera's entity — culling,
  picking, parallax and the editor's viewport included; `CameraShake`
  turns too (`max_angle`) and moves only the view; `CameraFollow2D`
  frames several targets and zooms to fit them, its dead zone and bounds
  drawn in the viewport.
- **Editor:** the inspector edits the fields a selection shares, a field
  that differs shown mixed and an edit writing only that field, as one
  undo step; duplicates and pastes are numbered `Name (1)`, `Name (2)`.
- **Prefabs:** variants (a prefab that is another with overrides,
  "Create variant" in the asset browser); a prefab inside another follows
  its own file without the outer one being saved again; a prefab that
  would hold itself is refused before saving.
- `games/platformer`'s walker is a `CharacterBody2D` that pushes crates
  and climbs a hill; the `character` example blends idle, walk and run
  by speed with a landing squash on an additive layer.

### Networking
- **Interest management:** each client is sent what is within a radius
  of what it watches (its own entities, or points the game sets),
  answered by a grid rebuilt each tick, with hysteresis; always-relevant
  entities and a radius per entity. The Network window shows each
  client's relevant entities.
- **Lag compensation:** `LagCompensated` colliders' poses are kept for a
  second; `tick.net.rewound` answers ray, cast, point and overlap queries
  as the shooter saw them, bounded by that player's measured lag and
  `LagCompensation::max_rewind`. The arena's zap uses it.
- **Lobby and LAN discovery:** a server-held lobby (names, slots, ready,
  a host who starts, late joiners refused or let in), its own channel,
  replication only to its players; LAN servers found by a UDP probe
  (native only — the web joins by address). A lobby screen prefab in
  every `cargo xtask new-game`; the arena readies up in it before its
  match; `voltra-server` gains `--lobby` and the discovery flags.
- **Wire:** the replication wire is versioned in the handshake
  (`REPLICATION_WIRE_VERSION` 3); a 0.3 peer is refused with
  `ProtocolMismatch` at connect.

### Fixed
- A networked build opened its level — first or changed to, with
  `load_scene` or `change_scene` — under fresh ids, so no client entity
  was the server's: the arena's coin had no sprite and its bot stood
  still. With replication on, the level keeps its file's ids; a client
  sets the level's replicated records aside — on `load_scene` as on
  `change_scene` — and spawns the server's copy of each from them when it
  arrives, so a coin keeps its sprite and one the server no longer has
  (taken) stays gone. A copy a change swept early is asked for again
  until it arrives — once a round trip, so a lost ask is not the end of
  it — and the server answers an ask for one it no longer has with a
  despawn.
  `ReplicaStats::level_waiting` counts what is set aside apart from
  `out_of_interest`.
- A level reloaded under its ids no longer spawns a twin of a
  `Persistent` entity it holds.
- A region unloading on a replication client let go of every entity the
  first snapshot swept, not only its own; a region now notes its
  replicated ids as each spawns, and a scene replaced lets go of the old
  level's unsent entries.
- The MCP `run_scene` screenshot drew HUD buttons unlaid out; the run now
  lays the HUD out on the shot's size, or on `WindowConfig::default()`'s.
- Test scratch folders are removed when their test ends, and those of
  ended processes swept — never one under a minute old
  (`VOLTRA_KEEP_SCRATCH` keeps them).
- The new microsecond benchmarks (`keyframes/sample`, `keyframes/blend`,
  `lag/*`, `interest/grid`) take three times the samples
  (`Config::steady`) and keep their limits; only
  `interest/server-frame-without` went from 15 to 20 %.

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
