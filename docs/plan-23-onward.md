# Stages 23 onward — the road after sprite sheets

Working file: a sketch per stage, not a plan to execute. Each one gets its own
`plan-NN-*.md` with files, commits and tests when it is next. Delete a section
once its stage has landed and its decisions are in ARCHITECTURE.md.

## Where the engine stands after 22

Runs a game: ECS, scenes on disk, sprites with sheets and animation, a 2D
camera, rigid bodies with filters, sensors, events and queries, two game ticks,
an editor with hierarchy/inspector/gizmos/undo/play/asset browser, and a player
binary, and since 23a it can make a sound. What it still cannot do: reuse a
piece of content, fill a screen with tiles, print a number, read a gamepad, or
be handed to someone who does not have Rust installed.

## The order, and why

| Stage | Scope | Why here |
| --- | --- | --- |
| ~~23a~~ | ~~Audio: the crate, the store, the components, one-shots~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| 23b | Audio: buses, tweens, editor audition, hot reload | What 23a deliberately left; see below. |
| 24 | Prefabs and runtime spawning | Everything after this multiplies content: enemies, bullets, tiles, particles. Doing it before them means they all reuse one mechanism. |
| 25 | Tilemaps | The classic 2D content path, and it needs 22's atlas underneath it. Also the first scene that makes stage 28 urgent. |
| 26 | In-game UI: text and screen space | A game cannot show a score. Held until here because a HUD wants prefabs and fonts want the atlas work from 22. |
| 27 | Input actions and gamepads | Raw `KeyCode` is fine for one example and wrong for a game. Late on purpose: an action map is a thin layer over input that already works. |
| 28 | Sleeping bodies and a real broad phase | `broad.rs` already names its own limit at ~200 bodies. A tilemap's generated colliders are what will cross it. |
| 29 | Export: `xtask`, and a build manifest | Turning the tree into a folder someone else can run. Last, because it packages whatever exists. |

Defensible reorderings, if the goal changes: aiming at **a playable demo**
soonest is 23 → 24 → 26 → 29 and tiles later; aiming at **content tooling** is
25 first and audio later. What should not move is 24 before 25/26 and 28 after
whatever generates the bodies.

---

## 23b — Audio: buses, tweens, editor audition, hot reload

23a landed: `voltra-audio` (mixer, voices, decoding, device), `Clips` beside
`Textures` and `Atlases`, `AudioSource` and `AudioListener` in the scene,
one-shots from `Tick::audio`, and the platformer coin that finally dings. The
decisions are under "The mixer is ours" in ARCHITECTURE.md.

What is left, and what each still has to answer:

- **Buses.** Named tracks (music, sfx, ui) with a volume each, and the mixer
  summing per bus before the master. Where the names live with no project
  settings file is the open question — probably a field on `AudioSource` and a
  table the game hands `Audio`, not a file.
- **Tweens.** A volume that ramps rather than steps, which is what a fade-out
  and a music cross-fade both need, and what stops a gain change from
  clicking. Per voice and per bus.
- **A limiter on the master**, replacing the hard clip 23a squares the mix off
  with. Belongs with buses because it is a bus effect.
- **Editor audition.** A play button on the `AudioSource` inspector — a button,
  not a checkbox — plus the inspector fields themselves, which 23a did not add:
  the component is authored in `.ron` for now.
- **Hot reload.** `Clips::reload` under the stable handle, and the watcher's
  extension filter widened to `wav` and `ogg`. The care it needs that textures
  did not: the audio thread may be reading the samples being replaced, so the
  swap is a command to the mixer, not a write through the store.

---

## 24 — Prefabs and runtime spawning

**Why now.** Every stage after this one wants to stamp out copies of something:
enemies, bullets, tiles, particles, UI elements. Building the mechanism once
means none of them invents its own.

**Prior art.** Unity: a prefab asset, instances that carry overrides, and
variants. Godot: a scene *is* a node tree, and instancing is that tree
inherited; "make local" breaks the link. Unreal: blueprint classes. All three
separate *the asset* from *the instance*, and all three have paid for
override propagation.

**Shape.**

- A prefab is a scene fragment on disk — the same `.ron` format, one or more
  roots — so nothing new has to be able to read it, and `ComponentRegistry`
  already covers every component.
- `prefab::spawn(world, &prefab, at) -> Entity`: fresh `SceneId`s, `Parent`
  links remapped through the existing resolver, and the whole subtree placed
  relative to `at`.
- `Tick` gets the same call, because bullets are spawned from a tick.
- `PrefabInstance { source: AssetPath }` records where an instance came from,
  even in v1 where nothing propagates. Without the link, adding propagation
  later means every existing scene has forgotten what it was made from.
- Editor: drag a prefab from the browser into the scene; "Create prefab from
  selection" writes the fragment and turns the selection into an instance.

**Decisions to make.**

- **Overrides.** The whole difficulty. v1 should instantiate and forget
  (Godot's "instance as plain nodes"), with the link recorded but inert. Per
  entity per component override storage, "apply to prefab", and what happens
  when the prefab gains a component afterwards are their own stage.
- Nested prefabs: allowed by construction, and worth one test that a cycle is
  refused rather than recursing forever.
- Whether a prefab is a distinct asset kind or "any `.ron` scene can be
  instanced". Leaning towards the second, with the browser showing scenes as
  droppable.

**Cut.** 24a = spawn, ids, parent remap, tick API. 24b = editor authoring and
the instance link.

---

## 25 — Tilemaps

**Why now.** It is the 2D content path, and stage 22's atlas is exactly what a
tileset is.

**Prior art.** Godot: `TileSet` resource plus a `TileMap` node with layers and
per-tile collision baked into the set. Unity: `Tilemap` + `TilemapRenderer` +
`TilemapCollider2D`, with rule tiles for auto-tiling. LDtk and Tiled are the
import formats worth reading rather than reinventing.

**Shape.**

- `Tilemap { atlas, tile_size, layer of indices }` as a component on **one
  entity per chunk**, not one entity per tile: a 100×100 map is 10 000
  entities the ECS would carry for no reason, and the mesh is per chunk anyway.
- A mesher that turns a chunk into the same `SpriteBatch` geometry the sprites
  already use, so nothing new reaches the renderer.
- Collision: merged boxes per run of solid tiles, generated once and stored as
  ordinary colliders, which is what makes stage 28 necessary.
- Editor: a paint tool alongside the transform tools — brush, rectangle,
  eraser, and a tile picker showing the atlas.

**Decisions to make.** Chunk size; sparse (`HashMap<IVec2, Chunk>`) versus
dense with bounds; how many layers and whether a layer is an entity; whether
auto-tiling (rule tiles) is in scope at all — probably not, it is a stage.

**Cut.** 25a = data, format, mesh. 25b = collision generation and the paint
tool.

---

## 26 — In-game UI: text and screen space

**Why now.** A game cannot print a score, and every demo eventually needs one.

**Prior art.** Unity: uGUI with `RectTransform` anchors, or UI Toolkit. Godot:
`Control` nodes with anchors and containers. Both settled on *anchors plus
offsets* for resolution independence.

**Shape.**

- Glyph rasterisation (`fontdue` or `ab_glyph`) into a glyph atlas in
  `voltra-render`, drawn by the sprite pipeline — a glyph is a textured quad.
- A screen-space pass with its own orthographic projection in pixels, drawn
  after the world.
- `Text { font, size, colour, align }` and `UiRect { anchors, offsets }` as
  components, so a HUD is authored in the same hierarchy and saved in the same
  scene file as everything else.
- **egui stays the editor's.** Shipping an immediate-mode editor UI inside a
  game means the game's look is egui's, and `voltra-player` does not link it —
  that separation is stage 19's whole point.

**Decisions to make.** SDF text versus bitmap-per-size (bitmap first, SDF is a
later refinement); whether interaction — buttons, focus — is in scope or
whether v1 is display-only; how a font becomes an asset with hot reload.

---

## 27 — Input actions and gamepads

**Why now.** `Input::was_key_pressed(KeyCode::Space)` is correct and unshippable:
no rebinding, no gamepad, no two-player.

**Prior art.** Unity's Input System: actions, action maps, bindings, control
schemes. Godot: named actions in the InputMap, `Input.is_action_pressed`.
Unreal Enhanced Input: input actions and mapping contexts.

**Shape.**

- An `InputMap` asset (`.ron`): named actions, each with a list of bindings —
  keys, mouse buttons, gamepad buttons, axis pairs, with dead zones.
- `Input::action("jump").pressed()` beside the raw reads, which stay: an editor
  shortcut is a key, not an action.
- Gamepads through `gilrs` in `voltra-core`, since gamepad input is platform
  input and core already owns that seam.
- Runtime rebinding — "press a key for jump" — because a map without it is
  half the reason to have one.

**Decisions to make.** Where the map lives with no project settings file:
probably an asset the game names, with `assets/input.ron` as the convention and
a player flag to override. Multiple devices and local multiplayer: one map,
several *actors*, or out of scope for v1.

---

## 28 — Sleeping bodies and a real broad phase

**Why now.** `broad.rs` documents its own limit — O(n²) stops being free at
roughly 200 bodies — and `lib.rs` lists sleeping among what is deliberately not
simulated. A tilemap's generated colliders are what will cross both lines.

**Shape.**

- Sleeping: Box2D's rule — a body under a linear and angular threshold for a
  time goes to sleep, an island sleeps together, and a touch or a force wakes
  it. It removes both the cost and the millimetre of jitter a settled stack
  keeps today.
- Sweep-and-prune on x, which `broad.rs` already names as the intended
  replacement, behind the same signature it kept for this reason.
- A measurement harness first. Neither change is worth making without a number
  before and after, and "it feels faster" is not a number.

**Decisions to make.** Whether the harness is `criterion` as a dev-dependency
or a small deterministic timing test; where islands live (the solver already
gathers bodies); whether static geometry is excluded from the sweep entirely.

---

## 29 — Export: `xtask` and a build manifest

**Why now.** The tree is not a game anyone else can run: it needs Rust, a
checkout and a command line.

**Prior art.** Godot's export templates and export presets; Unity's build
settings with a scene list; Unreal's cook and package.

**Shape.**

- `xtask`, the one crate ARCHITECTURE has been holding a slot for: `cargo xtask
  dist` builds the player in release, copies the assets it needs beside it, and
  produces a folder that runs by double-click.
- A **build manifest** — the startup scene, the window title and size, the
  asset root — read by the player when no command line says otherwise. This is
  where the project settings file that stages 19 and 21 both rejected finally
  earns its existence: it describes *a build*, not an editor.
- Asset handling: loose files first, an archive later. The asset root already
  resolves next to the executable, so a loose folder works today.

**Decisions to make.** Whether the editor writes the manifest (a Build panel)
or it is hand-written; whether unused assets are pruned (needs a dependency
graph — probably not now); Windows first, with Linux behind the same task.

---

## Cross-cutting, unscheduled

Things that will want a stage of their own eventually, listed so they are not
mistaken for oversights: particles (needs 22, wants 24), sorting layers and
parallax, a render graph and post-processing, scene transitions and additive
loading in the player, save games, a job system, and 3D — which per CLAUDE.md
is a new subsystem with its own design, not a parameter added to any of the
above.
