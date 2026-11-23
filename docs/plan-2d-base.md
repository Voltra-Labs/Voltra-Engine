# Stages 75 onward — a solid 2D base

Working file (CLAUDE.md): one section per stage, deleted as each lands and
its decisions reach ARCHITECTURE.md. Each stage gets its own branch and pull
request; a stage that grows past one review is split (75a, 75b) rather than
merged half-done.

## Where the engine stands

Stages 1–74 and W1–W4 (README roadmap): batched, lit and post-processed
rendering; CPU and GPU particles; rigid bodies with joints, characters and
one-way platforms; tilemaps with auto-tiling; audio; prefabs; property
animation with a timeline and state machines; navigation; an editor with
undo, gizmos and a timeline; the player; native and web export; an MCP
server; a profiler.

What a game still cannot do, found by reading the tree (not guessed):

- collide anything but a box or a circle, or stop a fast body tunnelling
  (`voltra-physics` documents both);
- be played inside the editor: the editor links no game code, so a game's
  ticks and its own components (the platformer's `Coin`) never run there;
- show an image in its UI, a slider, a text field, a scrolling list;
- wrap a line of text, or ship it in a second language;
- tween a value from code, or wait on a timer;
- give one sprite its own shader;
- draw through two cameras at once, or into a texture;
- snap to a grid in the editor, read the log there, edit project settings;
- read a touch.

And nothing tells a newcomer how to make a game with it.

## The order, and why

| Stage | Scope | Why here |
| --- | --- | --- |
| 75a | Capsule and convex polygon colliders | Physics is the most structural piece; every later stage that moves things leans on it |
| 75b | Continuous collision for fast bodies | Same subsystem, while it is open |
| 76 | The game as a plugin: the editor plays it | From here on every feature can be tried in the editor with real game code |
| 77 | Hot reload of the game plugin | The iteration loop a scripting language would have bought, in Rust |
| 78 | `cargo xtask new-game`: a project template | Makes 76–77 the default way to start, not an expert path |
| 79 | UI 1: images, 9-slice panels, layout | Menus and HUDs, with 80–82 |
| 80 | UI 2: slider, toggle, text field, scroll | |
| 81 | Text: wrapping and localization | |
| 82 | Tweens and timers | Small; used by everything above |
| 83 | Sprite materials: a game's own shader | Hit flashes, outlines, dissolves |
| 84 | Several cameras; drawing into a texture | Split screen, minimaps, pixel-perfect |
| 85 | Editor: grid and snapping, a console, project settings | Daily comfort building levels |
| 86 | Touch input | Phones and tablets, now that the web build exists |
| 87 | A user guide, sample games, version 0.1 | A base is solid when someone else can use it |

Deliberately left out: a scripting language (games are Rust; 76–78 give Rust
the iteration loop instead — see 76), 3D (CLAUDE.md), networking (a
subsystem of its own).

---

## 75a — Capsule and convex polygon colliders

**Scope.** `Collider::Capsule { center_offset?, half_height, radius }` and
`Collider::Polygon { points }` (convex, at most 8 points, counter-clockwise
after validation) beside `Box` and `Circle`, in contacts, queries (ray,
point, box, shape cast), sensors, one-way platforms, characters, the
editor's collider outline and the inspector. A polygon from a sprite's
silhouette (already traced for shadows, `voltra_assets::Silhouettes`) as a
convex hull.

**Shape.** Box2D v3, which the solver already follows: a polygon carries a
radius, so a box is a four-point polygon of radius zero and a capsule is two
points with a radius; contacts are polygon–polygon (SAT with reference-face
clipping), polygon–circle, capsule–capsule (segment closest points) and
capsule–polygon. `box_box.rs` and `box_circle.rs` stay as the fast paths
they are, or fold into the polygon code if the numbers say so — measured,
not assumed (`examples/stress.rs`). Shape casts move from exact box/circle
formulas to GJK + conservative advancement for the new shapes.

**Done when** a capsule character walks a polygon slope and a polygon stack
settles (physics tests with numbers, like the existing ones), every query
answers for every shape pair, an old scene loads unchanged, and the editor
draws and edits the new shapes.

## 75b — Continuous collision

**Scope.** A dynamic body moving more than a fraction of its own size in
one step is swept against static geometry (time of impact by conservative
advancement) and stopped at first contact; `RigidBody::bullet` sweeps it
against dynamic bodies too. Box2D v3's split: continuous against static by
default, bullets opt in.

**Done when** a body at 200 units/s does not pass through a wall 0.1 thick,
the stress example's frame time stays within its current budget, and the
"not simulated yet" line in `voltra-physics` loses continuous collision.

## 76 — The game as a plugin: the editor plays it

**Scope.** A game crate exposes one entry point — register its components,
install its ticks — through a small `voltra_core::Game` trait. Native builds
of the game link it as now; the editor loads the game as a dynamic library
(`cdylib`, through `libloading`, a leaf crate) named by the project, so Play
runs the game's ticks and its components show in the inspector by their
registered names.

**Shape.** Fyrox's plugin model, the closest engine to ours: Rust all the
way, the editor hosts the game's code, shipping builds link statically. Not
an embedded language (Lua, rhai): two languages and a doubled API, against
the point of the engine. Not wasm-hosted scripts (wasmtime): heavy, no host
inside a browser, a call boundary on every tick.

**The hard parts, decided in the stage:** Rust has no stable ABI, so the
editor refuses a library built by another compiler or with other flags, and
says which (a version stamp exported by the plugin); nothing of the game may
outlive its library (boxed ticks, registered types, `TypeId`s) — the editor
drops all of it before unloading.

**Done when** the platformer, built as a plugin, is played in the editor
with its coins counted, and its `Coin` is edited in the inspector.

## 77 — Hot reload of the game plugin

**Scope.** The editor watches the game crate, rebuilds the library on a
change, serializes the world through the `ComponentRegistry` (what scene
save already does), swaps the library, and deserializes — so a running Play
session keeps going with the new code. A build error is shown in the editor
and the old code keeps running.

**Done when** a test edits a tick's constant in a scratch game crate, the
editor reloads it mid-play, and the world's state survives the swap.

## 78 — `cargo xtask new-game`

**Scope.** A template crate: `Cargo.toml` set up for both the plugin and a
static binary, a `build.ron`, an asset folder with a scene, a `main` through
`when_assets_ready`. Builds with `dist` and `dist --web` untouched.

**Done when** CI generates a game, builds it native and web, and loads it in
the editor headless.

## 79 — UI 1: images, 9-slice panels, layout

**Scope.** `UiImage` (a texture or atlas frame in a `UiRect`, with a tint),
9-slice borders for panels and buttons, and `UiLayout` rows and columns with
spacing, padding and alignment; buttons drawn from images in their states.

**Done when** a pause menu — panel, title, three image buttons laid out in a
column — is built in a test scene and drawn and clicked headless.

## 80 — UI 2: slider, toggle, text field, scroll

**Scope.** `UiSlider`, `UiToggle`, `UiTextField` (caret, selection, IME
text through winit's `Ime` events) and a scroll container with a mask;
focus navigation as buttons have it.

**Done when** an options screen (volume slider, fullscreen toggle, name
field, a scrolling list of saves) works by mouse, keyboard and pad in a test.

## 81 — Text: wrapping and localization

**Scope.** `Text::max_width` with word wrapping and line spacing; string
tables per language (`locale/<lang>.ron`, keys to strings) read through the
asset funnel, a current language, and `Text` naming a key instead of a
literal; fallback to the key when a string is missing, logged once.

**Done when** a wrapped, translated dialogue box switches language at run
time in a test.

## 82 — Tweens and timers

**Scope.** `tick.tween(entity, property, to, seconds, ease)` over the
property paths animation clips already address, with the same eases;
`tick.after(seconds, …)` and repeating timers; both cancelled with their
entity.

**Done when** a test fades and moves a sprite, chains two tweens and fires a
timer, and a despawned entity's tweens stop.

## 83 — Sprite materials

**Scope.** A `Material` asset: a WGSL fragment function with declared
uniforms, compiled into a pipeline variant of the sprite pass; sprites and
tile layers name one; uniforms set from the inspector and from ticks;
materials break batches only where they differ.

**Done when** a hit-flash and an outline material run on WebGPU, WebGL2 and
native, and a scene without materials batches as before.

## 84 — Several cameras; drawing into a texture

**Scope.** More than one active camera, each with a viewport rectangle and
an order (split screen); a camera targeting a `RenderTexture` asset a sprite
or `UiImage` can show (minimap); a pixel-perfect option snapping the view to
the texel grid.

**Done when** a two-player split screen and a minimap draw in a test, and
pixel-perfect output is checked against a reference image.

## 85 — Editor comfort

**Scope.** A grid with snapping for moves, rotations and scales; a console
panel reading the captured log ring with level filters; a project settings
window for the input map, sorting layers and `build.ron`, saved as the files
they already are.

**Done when** each is driven through the editor bridge in a test.

## 86 — Touch input

**Scope.** Touches as pointers (`Input::touches`, began/moved/ended), a tap
as a click for UI, and virtual-stick bindings in the action map.

**Done when** a Playwright test touches the platformer's on-screen stick and
it walks.

## 87 — A guide, sample games, version 0.1

**Scope.** A user guide in `docs/guide/` (install, first scene, first game
crate, physics, UI, audio, export), every public item documented
(`missing_docs` on the public crates), two more small sample games beside
the platformer, a changelog, and a `v0.1.0` tag.

**Done when** someone following the guide from a clean machine ships a game
to the web — checked by CI running the guide's commands.
