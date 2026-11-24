# Stages 88 onward — 2D polish (0.2) and networking (0.3)

Working file (CLAUDE.md): one section per stage, deleted as each lands and
its decisions reach ARCHITECTURE.md. A stage that grows past one review is
split (95a, 95b) rather than merged half-done. 3D comes after 0.3, as a plan
of its own.

## Where the engine stands

Version 0.1 (stages 1–87, W1–W4; README roadmap and CHANGELOG.md), and since then the stages the README marks done: a 2D
engine an outsider can make and ship a game with, on native and the web.

What a game still cannot do, found by reading the tree (not guessed):

- keep a player's rebound controls on the web: `ActionMap::save` writes
  through `std::fs` (`input/map.rs`), which the browser does not have, while
  `Saves` already reaches `localStorage`;
- switch to fullscreen, change resolution or vsync from a tick:
  `window_requests.rs` has none of them;
- show Arabic, Hebrew, Devanagari or ligatures right: text is laid out by
  `fontdue` glyph by glyph, with no shaping, no bidirectional order and one
  fallback font (`voltra-assets/src/fonts.rs`); CJK wraps only at spaces;
- colour or embolden part of a text; offer a dropdown, a tooltip or tabs;
  style controls from one theme;
- pack sprites into an atlas: every `.atlas.ron` is written by hand;
- paint tilemaps with a palette, stamps, flood fill or an eyedropper (the
  tools are brush, rectangle and eraser), or give one tile its own collision
  shape (collision is one `bool` per layer);
- change a particle's size or colour along a curve, emit from a shape,
  spin particles or animate them from a sheet (`particles.rs`: start/end
  colour, a size range, a point emitter);
- draw a line, a filled polygon or a trail; animate a character by bones;
- stream a long piece of music (clips decode whole at load, `clip.rs`),
  crossfade tracks, or put an effect on a bus (`bus.rs` says so itself);
- edit a prefab on its own, or search the hierarchy and the assets;
- know how fast it is: there is no benchmark, so no regression is caught;
- load a big world in parts, or show a loading screen;
- play with someone else: there is no networking at all.

## The order, and why

### 0.2 — 2D polish

| Stage | Scope | Why here |
| --- | --- | --- |
| 88 | Rebound controls saved everywhere, the web included | A real bug in 0.1; small; first |
| 89 | Window settings from a tick: fullscreen, resolution, vsync | Every shipped game needs an options screen; pairs with 88 |
| 90 | An options-screen prefab: display, audio, controls rebinding | Uses 88–89 and stage 80's controls; what every game copies |
| 91 | Text shaping and a fallback chain per language | Correct text before more text features |
| 92 | Bidirectional text and Unicode line breaking | Same subsystem, while it is open: RTL, CJK wrapping |
| 93 | Rich text: inline colour, size, font, icons | Builds on 91–92's runs |
| 94 | UI 3: dropdown, tooltip, tabs, a theme file | Completes the control set; themes before more UI is written |
| 95 | Texture packer: an atlas from a folder | Asset pipeline; 96–99 want packed sheets |
| 96 | Atlas editor: slice a sheet by grid or by hand | The other half of 95, in the editor |
| 97 | Tilemap editor: palette window, stamps, flood fill, eyedropper | Daily level building |
| 98 | Tiles with their own collision shapes and custom data | Slopes, half tiles, ladders — what 97's palette edits |
| 99 | Particles 2: curves, gradients, emitter shapes, rotation, flipbooks | Effects; CPU and GPU kept equal |
| 100 | Particle editor: live preview, curve and gradient widgets | The other half of 99 |
| 101 | Lines, filled polygons and trails | Ropes, lasers, terrain, motion trails |
| 102 | 2D bones: a skeleton and cut-out animation | Character animation without a frame per pose |
| 103 | Skinned sprite meshes: weights over bones | Smooth bending; after 102 settles the skeleton |
| 104 | Streaming music and crossfades | Long tracks without their whole PCM in memory |
| 105 | Bus effects: filters, delay, reverb, ducking | Audio's own "planned" note, now with a mixer to put them on |
| 106 | Editor: prefab edit mode, search in hierarchy and assets | Comfort once projects grow |
| 107 | Benchmarks and a regression check in CI | Measure before optimising |
| 108 | Performance: what 107 finds, toward the stated budgets | Fixes chosen by numbers, not guesses |
| 109 | Big worlds: regions loaded in parts, loading screens | After 107–108 know the cost of a load |
| 110 | Version 0.2: guide chapters, a sample updated, changelog | A release someone else can use |

### 0.3 — Networking

| Stage | Scope | Why here |
| --- | --- | --- |
| 111 | `voltra-net`: a transport, native UDP and WebSocket | The pipe everything after rides on; the web cannot open UDP |
| 112 | Connections, channels and messages | Reliable and unreliable, ordered, timed out — before any game data |
| 113 | A headless server binary and sessions: host, join, leave | Somewhere authoritative to connect to |
| 114 | Replication: components the server sends, snapshots, deltas | The core of a networked game |
| 115 | Interpolation of remote entities | Smooth motion between snapshots |
| 116 | Prediction and reconciliation for the local player | Responsive input on a real connection |
| 117 | The editor plays with several clients; a network conditions simulator | Test lag and loss without a second machine |
| 118 | Version 0.3: a networked sample game, a guide chapter, CI | Ships the subsystem the way 87 shipped the base |

Deliberately left out: a scripting language (games stay Rust; stage 76),
3D (after 0.3, its own plan), native mobile builds (Android/iOS: a platform
stage of its own once 0.3 is in; the web build already reaches phones),
peer-to-peer and NAT traversal (a server is the first shape; P2P later if a
game needs it), rollback netcode for fighting games (116 predicts one
player; full rollback is its own design).

---

## 99 — Particles 2

**Scope.** Curves over a particle's life for size, speed and alpha; a
colour gradient; emitter shapes (point, circle, ring, box, edge, along a
collider's outline); rotation and angular velocity; flipbook frames from an
atlas region list. The GPU path (stage 45) gets the same features through
small lookup textures, so CPU and GPU emitters stay interchangeable.

**Done when** headless screenshots of each feature match on CPU and GPU.

## 100 — Particle editor

**Scope.** In the inspector: curve and gradient widgets, a live preview
while not playing, restart and burst buttons, and a few presets (smoke,
sparks, rain) to start from.

**Done when** a curve edited through the bridge changes the emitter's
particles in the next screenshot, as one undo entry.

## 101 — Lines, filled polygons and trails

**Scope.** `Line2D` (points, width, joins and caps, a texture along or
tiled, a width curve), `Polygon2D` (filled, textured, triangulated by ear
clipping), `Trail2D` (a ribbon behind a moving entity, fading over a time).
Drawn through the sprite batch, lit, in sorting order. Points editable in
the viewport (reusing stage 55's polygon handles).

**Done when** screenshots pin joins, caps, a concave filled polygon and a
trail behind a moving body.

## 102 — 2D bones

**Scope.** `Skeleton2D` and `Bone2D` (parented transforms with rest pose
and length), sprites attached to bones (cut-out animation), bones keyed by
the timeline (stage 66) like any property, and two-bone IK for limbs. Bones
drawn and posed in the viewport.

**Done when** a sample character walks by an animation clip of bone
rotations, and the IK test reaches a target.

## 103 — Skinned sprite meshes

**Scope.** A sprite split into a mesh whose vertices carry up to four bone
weights, deformed on the GPU; weights painted in the editor; the mesh made
from the sprite's outline (stage 59's silhouette tracing).

**Done when** a skinned arm bends smoothly in a screenshot and the
painted weights round-trip through the scene file.

## 107 — Benchmarks and a regression check

**Scope.** Benchmarks for the hot paths: sprite batching (10k–100k quads),
tilemap meshing, physics steps (1k bodies, piles), UI layout, text layout,
scene load. Run headless, numbers written as JSON; CI compares with the
main branch's and fails on a regression past a threshold (noise measured
first, so the threshold is honest). Budgets stated in ARCHITECTURE.md.

**Done when** the suite runs in CI, and a deliberately slowed function
trips it in a test of the comparer.

## 108 — Performance

**Scope.** What 107 shows: likely instancing sprites instead of expanding
quads on the CPU, fewer bind-group switches, physics broad-phase tuning,
cached text layouts. Each change justified by a number before and after.

**Done when** the stated budgets hold (for example 100k sprites at 60 fps
on the reference GPU and 2k active bodies in one step under 4 ms), recorded
in ARCHITECTURE.md.

## 109 — Big worlds

**Scope.** A scene split into regions loaded and unloaded around the
camera (additive loading, stage 30, made asynchronous: assets decoded on
the job pool, entities added over frames within a budget); a loading
screen with progress for a full scene change.

**Done when** a test walks a camera across a world of many regions with
memory bounded and no frame over budget while loading.

## 110 — Version 0.2

**Scope.** Guide chapters for what 88–109 added (options screen, text and
languages, tile and particle editors, bones, audio effects, big worlds), a
sample game updated to show them, the changelog, `v0.2.0`.

**Done when** CI runs the new chapters' checked blocks and the tag is on
`main`.
