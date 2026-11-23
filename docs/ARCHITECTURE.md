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
  ┌──────────────┐  ┌──────────────┐  binaries — each wires its own front end
  │voltra-editor │  │voltra-player │
  └──────────────┘  └──────────────┘
                  │
        ┌─────────▼────────┐  platform: event loop, window, input, time
        │   voltra-core    │
        │   (owns winit)   │
        └──────────────────┘
                  │
        ┌─────────▼────────┐  components and the geometry they become
        │   voltra-scene   │
        └──────────────────┘
                  │
         ┌────────┴────────┐
         │                 │
  ┌──────▼─────┐   ┌───────▼───────┐  identity, texture and clip caches
  │ voltra-ecs │   │ voltra-assets │
  │ (no deps)  │   └───────────────┘
  └────────────┘       │       │
       ┌───────────────┘       └───────────┐
┌──────▼────────┐                  ┌───────▼──────┐  owns cpal + symphonia
│ voltra-render │  owns wgpu       │ voltra-audio │
└───────────────┘                  └──────────────┘
```

`voltra-scene` is the only crate that knows about both entities and vertices.
Keeping that knowledge in one place is what lets `voltra-ecs` stay free of
rendering and `voltra-render` stay free of entities.

`voltra-assets` points into `voltra-render` because the thing it caches *is* a
GPU texture — caching decoded bytes and re-uploading per sprite would cache the
cheap half. It reaches `Device` and `Queue` through `voltra_render::wgpu` and
declares no `wgpu` of its own, so the one-crate-per-backend rule holds.

The diagram draws one path per crate, not the full dependency lattice: it omits
`voltra-scene`'s own direct reach into `voltra-render` for `Vertex` and `Mesh`,
the same way it already omitted `voltra-editor`'s direct reach into
`voltra-scene`. `voltra-core` and `voltra-editor` are two more edges it leaves
out on purpose — both depend on `voltra-assets` directly, not only through
`voltra-scene`, because both own a `Textures` cache: `App` builds one and holds
it for the frame loop, and the inspector panel takes a `&mut Textures` to
resolve a path the moment someone edits it. Drawing every real edge would bury
the one this section exists to explain. `voltra-testkit` does not appear at
all: it is a dev-only crate, `publish = false` and never anything but a
`[dev-dependencies]` entry, so it carries no edge a shipped binary's dependency
graph — which this diagram is — would ever need to show.

`voltra-assets` points into `voltra-audio` for the same reason it points into
`voltra-render`: what it caches is a decoded `Clip`, and the crate that knows
how to make one is the crate that owns the decoder. `Clips` needs no device —
decoding is arithmetic over a file — which is what keeps its tests headless on
a machine with no sound card.

**Rule:** exactly one crate may depend on `winit` (`voltra-core`), exactly one
on `wgpu` (`voltra-render`), and exactly one on `cpal` and `symphonia`
(`voltra-audio`). Everything else consumes them through re-exports, so a
version bump is a one-line change.

### Current crates

| Crate | Owns | Key types |
| --- | --- | --- |
| `voltra-ecs` | Entity handles and component storage. No dependencies at all | `World`, `Entity`, `SparseSet` |
| `voltra-jobs` | The worker pool and scoped jobs that borrow from their caller. Standard library only | `JobPool`, `Scope` |
| `voltra-profile` | Where a frame's time goes: named scopes and counters, recorded per thread, summarised as a tree. Standard library only | `scope`, `count`, `end_frame`, `Summary`, `Recorder` |
| `voltra-assets` | Asset identity, the texture, atlas and clip caches, loading from the asset root | `Handle`, `Assets`, `AssetPath`, `Textures`, `Atlases`, `Clips` |
| `voltra-audio` | The mixer, decoding, and the output device | `Audio`, `Mixer`, `Voice`, `Clip`, `PlayParams`, `VoiceId`, `decode`, `spatial` |
| `voltra-render` | GPU device, swapchain, frame recording, the egui backend | `GpuContext`, `Renderer`, `RenderTarget`, `EguiBackend` |
| `voltra-scene` | Scene components and their geometry | `AudioSource`, `AudioListener`, `Transform`, `Sprite`, `SpriteBatch`, `pick::sprite_at`, `SceneFile`, `ComponentRegistry`, `SceneId`, `RigidBody`, `Collider`, `PhysicsMaterial`, `CollisionLayers`, `Sensor`, `Name`, `Parent`, `WorldTransforms` |
| `voltra-physics` | Simulation over those components: integration, contact detection, the solver that resolves them, and the questions a game asks of them | `PhysicsWorld`, `PhysicsClock`, `candidate_pairs`, `Contact`, `Overlaps`, `step`, `SolverParams`, `ImpulseCache`, `Touching`, `CollisionEvent`, `query::ray`, `QueryFilter`, `RayHit` |
| `voltra-nav` | Navigation grids baked from the scene's colliders, A* paths through them | `NavGrid`, `NavSettings`, `Path` |
| `voltra-bridge` | The editor bridge: commands to a running editor and their answers, JSON lines over a loopback socket. No MCP, no async runtime | `BridgeServer`, `BridgeClient`, `Command`, `Pending` |
| `voltra-mcp` | The MCP server binary: a project's scenes, assets and runs — and a running editor, through the bridge — for an AI client, over stdio | `VoltraMcp`, `Project`, `tools::*` |
| `voltra-core` | Event loop, OS window, input, frame timing, the egui seam | `App`, `UiFrame`, `EguiLayer`, `Input`, `Clock` |
| `voltra-editor` | Editor binary and its panels | `main`, `Editor` |
| `voltra-player` | Player binary: a scene, a window, no editor | `main`, `args::Args`, `scene::load` |
| `voltra-testkit` | Headless GPU scaffolding for tests. `publish = false`, and only ever a `[dev-dependencies]` entry | `headless_device`, `read_texture`, `read_buffer`, `scratch_root`, `write_png` |

### Planned crates

Added only when there is code to put in them:

| Crate | Purpose | Blocked on |
| --- | --- | --- |
| — | Every crate planned here exists | — |

## Frame flow

```
winit event loop  (voltra-core::App)
  │
  ├─ Resumed          → create Window, build Renderer, EguiLayer, RenderTarget
  ├─ Resized(size)    → Renderer::resize → GpuContext reconfigures surface
  └─ RedrawRequested
       │
       ├─ App::update                 game tick → the steps owed, each one
       │                              preceded by the game's fixed tick
       ├─ RenderTarget::resize        to whatever the viewport panel asked for
       ├─ SpriteBatch::from_world     world → vertices → Mesh
       ├─ Renderer::render_scene      draws into the target, not the window
       ├─ EguiLayer::prepare          lays the UI out, uploads its geometry
       └─ Renderer::present_with      acquire → clear → EguiLayer::render → present
                                      then request_redraw → continuous loop
```

`GpuContext::acquire` returns `Option` on purpose: surface loss, resize races
and minimised windows are *normal*, not errors. `Outdated` and `Lost`
reconfigure and skip the frame; `Timeout` and `Occluded` skip silently.

Without a UI callback the middle collapses to `Renderer::render_mesh`, which
draws the world straight to the window. That is the path a shipped game takes.

### The viewport is one frame behind

The scene has to be drawn before egui can sample it, but a panel only learns how
much room it has *while* egui is laying out. So `UiFrame::request_viewport_size`
takes effect on the next frame. Dragging a splitter shows the previous size for
one frame, which is not visible; the alternative is two egui passes per frame.

## Decisions

### No ECS crate (`hecs`, `bevy_ecs`, …)

Deliberate. Writing our own is the point of the project. The trap is that a
Bevy-style archetype ECS in Rust needs `UnsafeCell`, `TypeId` erasure and manual
aliasing proofs in every query — a subproject, not a module.

**Therefore the first ECS is the simple one**, and it is what `voltra-ecs` now
contains: generational `Entity` handles plus one sparse set per component type,
zero `unsafe`. Insert, remove and lookup are O(1) and each component type
iterates contiguously. The weak spot is multi-component queries, which walk one
set and look the rest up per entity.

Archetypes come later, driven by profiler output, not by aesthetics.

Two invariants the tests pin down, both of which fail silently if broken:

- **Generations must be checked on every access.** An index is recycled after a
  despawn, so a stale handle would otherwise read and write whichever entity
  took the slot.
- **`World::despawn` must clear the storages before bumping the generation.**
  Bump first and every storage sees a stale handle, refuses to remove anything,
  and leaks one component per dead entity forever.

### `glam` instead of a `voltra-math` crate

The no-frameworks rule targets engine architecture — ECS, scene graph, render
graph — not leaf libraries. `glam` is a leaf: SIMD vectors and matrices with no
opinion about how a game is structured, so depending on it costs no design
freedom. Hand-written matrix maths would teach nothing and is prime territory
for silent sign and transpose errors.

It is used directly in `voltra-render` and re-exported from there. A
`voltra-math` facade wrapping it would be a crate with no code in it.

### `egui` for the editor UI, with our own wgpu backend

The no-frameworks rule targets engine architecture. A UI toolkit is not that:
egui has no opinion about entities, render graphs or asset loading, and writing
one would be a project on its own for no lesson this repo is trying to teach.

`egui-wgpu` — the official backend — is pinned to `wgpu = "29.0"`, so pulling it
in would give the build two incompatible copies of wgpu, whose `Device` types
are different types. The backend is therefore ours, in
`voltra-render::egui_backend`, and it is only ~400 lines because egui hands a
backend nothing harder than a vertex buffer, an index buffer and texture deltas.

`voltra-render` depends on `epaint`, not `egui`: the render layer deals in
triangles and knows nothing about widgets or layout. `egui` itself is a
`voltra-core` dependency, alongside `egui-winit`.

Two things a hand-written egui backend gets wrong silently, both pinned by
tests in `tests/headless_egui.rs`:

- **egui's textures are gamma-encoded, and its blending happens in that space.**
  They upload as `Rgba8Unorm`, never `Rgba8UnormSrgb`, and the shader converts
  at the end. Letting the sampler convert as well darkens everything.
- **The same applies to the viewport image.** The scene target is sRGB so its
  own blending is right, so egui samples it through `RenderTarget::raw_view`, a
  second view in the non-sRGB format. `view()` there costs a visible darkening
  that no validation layer reports.

### The editor owns the editor camera

`voltra-core` used to read `WASD`, the wheel and `R` in `App::update` and fly
`Renderer::camera` with them. That made the platform layer decide what a pan
gesture is, and handed editor bindings to any game taking the no-UI path.

Checked against the engines that have already answered this:

- Unity's scene camera is `Editor/Mono/SceneView/SceneView.cs`, in the
  `UnityEditor` assembly; `SceneView` extends `EditorWindow`.
- Unreal navigates through `FEditorViewportClient`, an editor-module type.
- Godot handles 2D navigation in `CanvasItemEditor`, an editor plugin, not in
  `Camera2D`.
- Bevy ships no controller in `bevy_render`.

Unanimous: **the render layer exposes a camera, the tool decides how it moves.**
Navigation therefore lives in `voltra-editor::camera::ViewportCamera`, and
`App::update` touches no camera at all.

Two consequences worth stating:

- **Scroll and keyboard scoping to the viewport are both ours, not egui's.**
  `ViewportCamera` reads `InputState::smooth_scroll_delta` only when
  `response.hover_pos()` is `Some`, so a scroll over the hierarchy never
  reaches it regardless of what egui does with the delta elsewhere. A
  `ScrollArea` does additionally zero `smooth_scroll_delta` once it has
  actually scrolled — gated on `scrolling_up || scrolling_down`, so a list
  that fits entirely, or one already at its end, leaves the delta untouched —
  but that is a courtesy to *other* consumers of the delta, not the reason
  `ViewportCamera` gets this right; using the raw delta instead of the
  smoothed one is what let the old code get it wrong, by zooming the scene
  on a scroll over the hierarchy. Keys get no such courtesy at all: `keys_down`
  is populated from the raw `Event::Key` regardless of focus, and
  `count_and_consume_key` only strips matched events out of `self.events`,
  never out of `keys_down`, so `i.key_down(Key::W)` reads true with a text
  field focused. `ViewportCamera::navigate` therefore gates keys on two
  things of its own: `response.hovered()` scopes them to the viewport, and
  `Context::egui_wants_keyboard_input` backs off again while a widget holds
  focus.
- **Zoom is clamped, in the layer that divides by it.** `Camera2D::zoom` is
  private behind `set_zoom`, which clamps to `MIN_ZOOM`..=`MAX_ZOOM` and refuses
  `NaN`. Godot does the same (`CLAMP` in `EditorZoomWidget::set_zoom`); so does
  every editor that has shipped a zoom control. Steps are multiplicative for the
  same reason theirs are — a notch should feel the same at any magnification.

`viewport_to_world` / `world_to_viewport` sit on `Camera2D` rather than in the
editor because the projection lives there. Picking, gizmos and a grid overlay
will all want them.

### wgpu over raw Vulkan or OpenGL

The C++ engine was OpenGL-only. wgpu gives Vulkan/DX12/Metal/GL/WebGPU from one
codebase, enforces resource lifetimes through `Drop`, and validates at API level
— which removes the whole class of "forgot to delete the GPU object" bugs that
the C++ tree kept hitting.

### Draw order is a sort key on the sprite, not a Z

This is a 2D engine with no depth buffer — `depth_stencil: None` in every
pipeline — so what covers what is decided entirely by the order geometry reaches
the GPU. That order used to be `World::query2`'s, which is sparse-set storage
order, and a sparse set fills the hole left by a removal with its last element.
Despawning one entity could therefore reorder two unrelated overlapping sprites.
Alpha blending is order-dependent, so that was a rendering bug before it was a
picking bug.

`Sprite::sort_order` is an `i32`, and `SpriteBatch::from_world` sorts on
`(sort_order, entity.index())` before emitting vertices.

- **An integer, not a float.** Unity's `sortingOrder` and Godot's `z_index` are
  both integers. Ties are then exact rather than dependent on a float's
  representation.
- **`entity.index()` breaks ties, and that half matters most.** Without it,
  everything sharing the default `sort_order` of 0 — which is the common case —
  falls straight back to storage order, and the fix would fix nothing.
- **Named `sort_order`, not `z_index`.** Godot's name describes an axis it has
  no relation to. `Sprite::sort_order`'s own doc comment already reserves the
  point: when a real Z eventually exists, the name has to still be free.

Picking uses the same ordering, in `voltra_scene::pick::sprite_at`. One
definition used twice: if they diverged, a click would select something other
than the sprite whose pixels are visible.

The hit test carries the point into the sprite's local space and compares
against the unit quad, rather than building an oriented bounding box. Rotation
and non-uniform scale then need no separate code path. A transform whose
determinant falls below a named `MIN_DETERMINANT` (`1e-12`) is rejected before
the inversion — deliberately not `f32::EPSILON`, which is a *relative*
precision figure, the gap between `1.0` and the next representable float, and
would as an absolute floor reject a sprite that is small but perfectly
invertible and legitimate. `1e-12` is instead the determinant of a uniform
scale of one millionth of a world unit, far below one pixel even at the
camera's closest zoom, so the guard is about intent rather than about
`Mat3::inverse` breaking down: a singular matrix returns NaN rather than
panicking, and every comparison against NaN is false, but `inverse_or_zero`
would be worse — a zero matrix sends every point to the origin, which is inside
the quad, so a collapsed sprite would be pickable everywhere.

Rejected: **pixel-accurate hit-testing**, which is what `bevy_sprite`'s picking
backend does. It can, because each of its sprites carries its own texture and
therefore its own alpha. Sprites here carry an optional texture too as of
stage 12b, but `pick::sprite_at` does not read it — picking is still the same
quad test against every sprite's local space, textured or not, until
pixel-perfect hit-testing is built on top of that per-sprite alpha.

### When 3D arrives: one world, two render paths

**Nothing in this entry is built.** The engine is 2D today and CLAUDE.md forbids
building 3D scaffolding early. This records the *shape* 3D will take, so that
decisions made now — a scene file format above all — are not made accidentally
against it. It is a direction, not a design; 3D gets its own spec when there is
code for it.

#### What the established engines actually do

| Engine | Scene model | Render path | Transform | Physics |
| --- | --- | --- | --- | --- |
| **Godot** | Two trees: `CanvasItem` and `Node3D` | Two paths, separate by design | `Transform2D` and `Transform3D` | Two engines |
| **Unity** | One `GameObject` graph | 2D is an orthographic camera over the 3D path | One, always 3D | Box2D and PhysX |
| **Bevy** | One `World` | Two render subgraphs, `Core2d` and `Core3d`, one per camera | One, always 3D | Third-party, per-dimension |
| **Unreal** | 3D only | 3D only — Paper2D sprites are quads in the world | One, 3D | One |

Two things are unanimous and one is not.

**Unanimous: the render path splits, and so does physics.** Not one path with a
flag — separate paths. Bevy's `Core3d` runs a depth prepass and `Core2d` does
not; Godot's canvas renderer and its 3D renderer share only low-level GPU
resources. Nobody parameterises one into the other, because the differences are
not parameters: depth testing versus painter's order, frustum versus rectangle,
per-pixel sorting versus a sort key.

**Not unanimous: whether the scene model splits.** Godot duplicates it. Unity and
Bevy share it and pay for 2D objects carrying a 3D transform they never use.

#### What Voltra will do

**One `World`, following Unity and Bevy.** Godot splits its *scene tree*; we do
not have one, we have an ECS. Copying that split would mean two `World`s, and
with it the duplication Godot lives with — two cameras, two physics, two of
everything above the renderer. `voltra-ecs` will never learn that 3D exists.

**Separate render paths, following Godot.** The camera component selects the
path: a 2D camera gets no depth buffer and draws in `sort_order`; a 3D camera
gets a depth buffer and a frustum. `voltra-render` grows a second path, not a
branch inside the first.

**`Transform` and `Transform3D` as separate components in that one world.** This
is the one place we deviate from both favourites, and the deviation is earned by
the storage we chose. Unity and Bevy cannot do this: their transform is a single
type on a single component slot, so a 2D sprite pays for a `Vec3` and a rotation
quaternion it never reads. Our ECS is one sparse set per component type, so an
entity holds whichever transform it needs and a query costs nothing for the other.
Godot reaches the same outcome through separate node types; we reach it through
separate components, which is the ECS-native form of the same idea.

**Consequences for code written today**, all of which CLAUDE.md already requires:

- `Vertex::position` stays `[f32; 2]`. 3D gets its own vertex type rather than a
  widened one, so a sprite never carries a dead Z.
- The 2D path never gains a depth buffer, before or after 3D exists.
- `Sprite::sort_order` stays 2D. 3D sorts by depth; these are different
  mechanisms, not one mechanism with a parameter.
- **A scene file records which of the two an entity is**, and records a 2D
  transform as two floats rather than three. This is the decision this entry
  exists to protect: a file format is the one thing that cannot be refactored
  freely later, because files written in the old shape already exist.

#### Rejected

- **Two worlds, Godot-style.** Buys clean separation at the cost of duplicating
  every subsystem above the renderer. Godot accepts that trade because its scene
  tree is the engine's central abstraction; ours is the ECS, and an ECS already
  separates by component.
- **One 3D transform for everything, Unity- and Bevy-style.** Simpler, and it is
  what both of our reference engines do — but it taxes every 2D sprite forever
  for a dimension the engine may not use for years, and our component storage
  makes the tax avoidable. Revisit if sharing systems between 2D and 3D turns out
  to matter more than the per-entity cost.
- **3D only, Unreal-style, with 2D as quads in a 3D world.** Coherent, and it is
  why Unreal has no 2D/3D split to maintain. Rejected because 2D is what this
  engine is for right now, and paying 3D's costs to get it would be backwards.

### Scenes are RON, and unknown components survive

**Serialization cannot live in `voltra-ecs`.** That crate stores components as
`HashMap<TypeId, Box<dyn ErasedStorage>>`, and `ErasedStorage` exposes only
`remove_entity` and a downcast — there is no way to enumerate which types a
`World` holds, or to serialize a storage without already knowing `T` at compile
time. `voltra-ecs` also has zero dependencies on purpose; adding `serde` there
would end that. The list of persistable types has to live somewhere that both
knows concrete types and is allowed to depend on `serde`, which is
`voltra-scene::format::ComponentRegistry`. `register::<T>("Name")` closes over
`T`, and saving walks the registry rather than the storages.

**The registry is keyed by a chosen name, not a Rust path.** `register::<Sprite>
("Sprite")` writes `"Sprite"` to the file, not `voltra_scene::sprite::Sprite`.
Renaming or moving the type in code then does not silently orphan every scene
that already references it — the name on disk and the name in code are two
things, deliberately, and only one of them is allowed to be free to change.

**`SceneId` is UUID v7.** A v7 carries a timestamp in its high bits, so sorting
by id sorts by creation order. `to_scene_file` writes entities in `SceneId`
order, which is what makes a save both deterministic — the same world always
serializes to the same bytes — and append-friendly in a diff, since a newly
created entity's id sorts after every existing one instead of landing in the
middle of the file. A v4 id would force a choice between those two properties;
v7 buys both from the same field.

**Unknown components are preserved and written back**, not dropped. On load, a
component name the registry does not recognise is kept unparsed in
`UnknownComponents`, a `BTreeMap<String, Box<ron::value::RawValue>>` on that
entity, and `log::warn!` names it once; on save, those raw values are merged
back into the entity's map alongside whatever this build did understand.
Checked, not guessed at:

- **Unity** drops a component's data the moment it saves a scene containing one
  whose script cannot be resolved — the well-known "Missing (Mono Script)"
  failure mode. Rejected: it is the one behaviour here that destroys work,
  silently, the instant an editor with an older or incomplete plugin set
  touches a file someone else built.
- **Godot** keeps a node's serialized properties even when its script is
  missing. Adopted: a build that does not know `Physics` can still open a
  scene, move a sprite, and save without deleting the `Physics` line it cannot
  read.

`RawValue` rather than `ron::Value` is what makes the preserved side exact
rather than approximate: `Value` has no struct variant, so a component would
round-trip through it as a brace-and-quoted-key map with its fields resorted
alphabetically, and its deserializer rejects enums outright. `RawValue` is
`#[repr(transparent)]` over the original RON text, so an unknown component's
field order, syntax and any enum it contains survive untouched, not merely its
data.

**A known component that fails to deserialize is an error, not a preserved
unknown.** The two look similar — both are "a component in the file this call
cannot turn into a live value" — but they mean opposite things. Unknown means
"not mine, do not touch"; a registered name whose data does not fit the type
means "mine, and broken", and preserving that as if it were foreign would
silently keep broken data alive across every future save instead of surfacing
it once. `from_scene_file` therefore rolls back every entity a failing load
spawned rather than committing a partial world.

**`Scene ▸ Open` loads before it despawns** (`scene_file::open`). The obvious implementation clears
the world and then loads, but a typo'd path, a corrupt file or an unsupported
version then leaves the user with an empty scene and nothing to show for it —
and `from_scene_file`'s own rollback cannot help, because the despawning
happened outside the call it protects. The menu instead captures the entities
already in the world, loads — which only ever adds — and despawns the captured
set once loading has reported success. A failed open is a no-op: the scene is
exactly what it was. This is the all-or-nothing guarantee above, extended past
the function boundary it would otherwise stop at.

**What is "in the scene" is decided by identity, not appearance.** `Save`,
`Open`, `Clear` and the hierarchy list all query `SceneId`. They used to
disagree — two of them queried `Sprite` instead — and only agreed in practice
because every spawner happens to insert both components together. The first
entity to break that pairing, such as one loaded from a file with no
`"Sprite"` component, would have been visible to some of the four and
invisible to the rest: listed but not cleared, say, or cleared but not saved.
One question asked four times needs one answer. The useful corollary is the
other direction: an entity with a `Sprite` and no `SceneId` is deliberately
transient — not listed, not cleared, not saved — so opting out of persistence
is the absence of a component, not an exclusion list someone has to keep in
sync.

**Not a guarantee this design makes: byte-identical output against an
arbitrary input file.** Formatting is the serializer's choice, so a
hand-written file with different indentation, spacing or key order never
round-trips to itself — there is nothing to compare it against but its own
opinion. What is guaranteed, and is what `saving_is_idempotent_through_a_load`
pins down, is narrower and exact: **save, load, save produces identical
bytes.** A file this build already wrote is a fixed point.

### A scene save replaces the file or leaves it alone

**`voltra_scene::format::save` used to end in `std::fs::write`, which
truncates the destination and then writes into it.** Between those two steps
the file on disk is shorter than both the old scene and the new one, so a
crash, a process kill, a full disk or an I/O error inside that window leaves
a truncated scene file with the previous version gone. This is the only
place in the workspace that writes user data, so it was the only place that
could lose any of it.

**A save now has exactly two outcomes.** `Ok(())` means the file holds the
complete new scene; `Err(_)` means the file is byte-for-byte what it was
before the call, or still absent if it was absent before. There is no third
state — a reader opening the file concurrently sees one whole version or the
other, never a partial one.

The mechanism is the standard one: write the new bytes to a temporary file,
flush them to the physical disk, then rename the temporary over the
destination. Three details are what make that guarantee actually hold:

**The temporary is a sibling of the destination, not in the system temp
directory.** `rename` is atomic only within a single volume; across volumes
it degrades to a copy, which reopens the exact window this change exists to
close.

**`sync_all` runs before the rename, not after.** Skip it and the rename can
reach the disk before the bytes do, and a power loss then leaves the file
renamed and empty — the original destroyed, the replacement never written.

**The temporary name is unique per write**, `<file_name>.<uuid-v7>.tmp`,
reusing the `uuid` dependency the crate already has. Two editor instances
open on the same project then never share a temporary and so cannot
truncate each other's half-written file.

Godot already solves this the same way: it writes a `.tmp` file beside the
destination and renames it over. We take that pattern and depart on one
point — Godot's temporary name is fixed, ours is not. A fixed name
self-cleans, but it means two concurrent writers share one temporary and can
corrupt each other, which is this same bug one level down; Godot lives with
it and has the matching report
([godotengine/godot#956](https://github.com/godotengine/godot/issues/956)).

**`std::fs::rename` does replace an existing destination on Windows.**
Windows 10 1607 and later use `FileRenameInfoEx` with
`FILE_RENAME_FLAG_POSIX_SEMANTICS`, falling back to `MoveFileEx` with
`MOVEFILE_REPLACE_EXISTING`
([rust-lang/rust#131072](https://github.com/rust-lang/rust/pull/131072)).

#### Rejected

- **An in-place write plus a `.bak` copy of the previous version.** Turns one
  truncation window into two — the destination still has a moment where it
  is truncated, and now so does the backup. Scene files already live in git,
  which is a better backup than a sibling file ever would be.
- **The `tempfile` crate.** `NamedTempFile::persist` does the same thing and
  is well tested, but it is about sixty lines against `std`, pulls in
  `fastrand` plus `rustix`/`windows-sys` transitively, and picks the
  temporary's own name. That last point stops being cosmetic in stage 12,
  when a hot-reload watcher starts watching `assets/`: the name of the file
  that appears and vanishes on every save needs to be ours to choose.
- **A fixed temporary name.** The Godot point above, rejected for the same
  reason it costs Godot: it self-cleans, but it lets two concurrent writers
  corrupt each other.

**Known gap, not papered over:** the directory entry itself is not fsynced,
because Windows has no portable equivalent of an `fsync` on a directory
handle. After a power loss the rename may not have reached the disk even
though the bytes did. The consequence is losing that one save, never the
previous file.

### An asset is named by its path, and a bad name draws magenta

**A path is the identity, in an enum.** Bevy chose `AssetPath` as canonical
deliberately and deferred UUIDs — *"everyone uses filesystems... to manage
their asset source files"*
([bevyengine/bevy#8624](https://github.com/bevyengine/bevy/pull/8624)). The
enum shape is what keeps a `Uuid` variant addable later without changing the
scene format's `VERSION`, which matters because a file format is the one
thing that cannot be refactored freely — files in the old shape already
exist.

**`AssetPath` is a security boundary, not a newtype for tidiness.** A scene
file is external input; a raw string in one could name a file anywhere on
the machine, and merely opening the scene would read it. The check lives in
the constructor and `Deserialize` routes through it by hand, because a
derived impl would skip it on the only path that matters.

**The handle is an index and a generation, not a refcount.** Same shape as
`voltra_ecs::Entity`, so the engine has one idea of a handle. Bevy and Godot
refcount and get eviction for it; here an `Arc` in `Sprite` would cost its
`Copy`, which `batch.rs` and `pick.rs` rely on in per-frame loops, and no
measured memory problem is asking for eviction. `Assets::remove` exists so
the generation is real and testable; when to call it is the part still
deferred.

**A failure draws a magenta checker and the scene still opens.** Same value
the format already states twice — an unknown component is preserved, a
failed Open changes nothing. A path is user data, not a build invariant, so
a moved PNG must not make a scene unopenable. The 1×1 white texture already
in the tree was rejected as the placeholder: it is indistinguishable from a
sprite with no texture, which hides the failure everywhere but the log.

**Loading is synchronous, and that is not a shortcut to undo.** There is no
task system, and building one for this would be the subproject rather than
the module. The placeholder is exactly what an async load must return while
bytes are in flight, so the call site does not change shape when async
arrives.

#### Rejected

- **Refcounted strong/weak handles.** Solves eviction properly in Bevy and
  Godot. Rejected while `Sprite` stays `Copy` and nothing measured asks for
  eviction — see above.
- **A UUID sidecar per asset** (Unity's `.meta`, Godot 4.4's `uid://`).
  Survives a rename, but costs a sidecar and an import step this engine has
  no editor to manage. Failure mode when the sidecar is lost: a silently
  dead reference.
- **Failing the scene load on a missing texture.** Would contradict Open's
  rollback — a moved PNG must not make a scene unopenable.

### Sprites carry a path and a handle, and batch by contiguous runs

**`Sprite` stores both `texture: Option<AssetPath>` and a `#[serde(skip)]`
`texture_handle: Option<Handle<Texture>>`.** The path is the identity a `.ron`
file understands; the handle is a session-local index `Textures::load` fills
in, and nothing on disk ever sees it. `Sprite` loses `Copy` this way —
`AssetPath` holds a `String` — but `batch.rs` and `pick.rs` already take
references, and Bevy and Godot do not treat their sprite component as cheap
`Copy` either.

**Resolving a path into a handle happens outside `from_world`, in the
app/editor wiring that owns `Textures`** — on scene Open, on an inspector path
commit, and whenever code calls `Sprite::set_texture`. `from_world` never
touches the disk, matching Bevy's `AssetServer` and Godot's `ResourceLoader`:
both resolve off the draw loop. Loading inside `from_world` would stall every
frame on a broken path until the miss got cached, and would couple scene
geometry to GPU device lifetime, which nothing else in `voltra-scene` does.

**`from_world` still sorts by `(sort_order, entity.index())` first, unchanged,
and only then splits the sorted mesh into contiguous same-handle runs.** Unity
and Godot both batch this way — adjacent sprites merge only if they already
share a texture after sort order is decided, never before. Sorting by texture
first would break painter's order, which alpha blending depends on.
Interleaved textures (`A, A, B, A`) become three draws; two sprites naming the
same PNG still share one handle and one GPU texture no matter what sits
between them in sort order — 12a's promise, unaffected by how many draws that
costs.

**No texture and a failed load stay visually distinct**, same values 12a
already chose for the failure case: `None` draws the existing 1×1 white times
the sprite's colour; a path that fails to load draws the magenta-and-black
checker. White-times-colour is what a coloured sprite with no PNG has always
drawn, so keeping it is not a new behaviour — a second, indistinguishable
"missing texture" look would only hide the failure from the render, not the
log.

**The scene format grows a field, not a version.** `texture: Option<AssetPath>`
is `#[serde(default)]`, so a scene written before this stage deserializes with
`texture: None` and opens as untextured white quads. A missing optional field
with a default is not a breaking format change, so `VERSION` does not move.

**Bind groups for loaded textures are cached on `Textures` at load time,
built against the render pipeline's bind group layout, not rebuilt per frame
or per draw call.** `Textures` already owns the `Device` a bind group needs;
recreating one every frame would be GPU work with no matching change on
screen. `Renderer` keeps its own white bind group as the sentinel for a
`None` range — the one bind group not cached on `Textures`, because it
belongs to no texture.

**`voltra-render` still does not depend on `voltra-assets`.** It receives
index ranges and bind groups from the caller and draws each range with the
matching group; it has no idea a range came from resolving a path. The
dependency direction is `voltra-scene → voltra-assets → voltra-render`; the
layers diagram in `## Layers` was corrected in the same stage that added this
paragraph, which had drawn that chain as a stray arrow into `voltra-render`
rather than the edge it actually is.

#### Rejected

- **Sorting by texture first, `sort_order` only within a texture group.**
  Maximizes batching, but breaks painter's order the moment two
  differently-textured, overlapping, alpha-blended sprites need a specific
  draw order. Unity and Godot both reject this for the same reason; Godot's
  overlap-aware reordering is an opt-in optimisation on top of sorted order,
  not a replacement for it, and is out of scope here.
- **Loading textures inside `SpriteBatch::from_world`.** Keeps the call site
  simple at the cost of disk I/O on the draw loop and a GPU device the
  geometry layer has no other reason to reach. Matches neither `AssetServer`
  nor `ResourceLoader`, both of which resolve before the frame that draws
  the result.

### The asset root is resolved, never assumed

**The directory `AssetPath`s are joined onto comes from
`voltra_assets::default_root()`, or from `App::with_asset_root`.** The
resolution order is `VOLTRA_ASSET_ROOT`, then the nearest `assets` directory at
or above `CARGO_MANIFEST_DIR`, then the nearest one at or above the
executable's directory, then `<cwd>/assets`.

No engine resolves assets against the process working directory, and this one
should not either: the working directory is set by whatever shell or launcher
started the process, and it silently changes what every path in every scene
file means. Bevy resolves `BEVY_ASSET_ROOT`, then `CARGO_MANIFEST_DIR`, then
the executable's parent. Unreal hangs every path off the executable's base
directory. Unity's `Application.dataPath` is `<project>/Assets` in the editor
and `<exe>_Data` in a player build. Godot's `res://` is the project directory
in the editor and the PCK beside the executable once exported.

The walk *upwards* is the one place this differs from Bevy: `cargo run -p
voltra-editor` sets `CARGO_MANIFEST_DIR` to `crates/voltra-editor`, not the
workspace root, so joining `assets` onto it directly would resolve to a
directory that does not exist. The walk is bounded at six levels, because an
unbounded one started in a temp directory would adopt any `assets` sitting near
the drive root.

#### Rejected

- **`<cwd>/assets`, which is what stage 12b shipped.** Works only when the
  process is started from the workspace root, which no shipped binary is.
- **The executable's directory alone.** Correct for a shipped game, wrong
  during development, where the binary is under `target/debug` and the assets
  are not.

### Sprite indices are `u32`

**`Mesh::indexed` takes `&[u32]` and binds `IndexFormat::Uint32`.** A sprite
batch is not one object's geometry: it holds every sprite in the world in one
buffer, split into per-texture ranges, so it cannot be "split before it gets
large" the way a mesh with its own draw call can. At 16 384 sprites a `u16`
base index wraps and the next sprite silently draws over the first, with no
validation error anywhere. Bevy binds its sprite index buffer as `Uint32` for
the same reason. The cost is two extra bytes per index.

### Headless test scaffolding lives in `voltra-testkit`

**Adapter acquisition, texture readback, scratch directories and PNG writing
are one dev-only crate, not one copy per crate's `tests/` tree.** Each
integration test is its own binary and each crate its own tree, so the
alternative is the same 120 lines copied per crate — it was already at two
copies with a third due. `voltra-testkit` is `publish = false` and appears only
under `[dev-dependencies]`, so it is not part of the shipped dependency graph.

### Hot reload swaps contents under a stable handle

**`Textures::reload` replaces the texture in the slot its handle already names
and replaces that handle's bind group with it.** The handle does not change, so
`Sprite`, `SpriteBatch`, the scene format and the renderer never learn that
anything happened — which is the only reason a reload can be a per-frame
operation rather than a walk of the world.

Replacing the bind group is not optional. It holds the *old* `TextureView`, so
swapping the texture alone changes nothing the GPU can see.

A reload that fails keeps the previous pixels and warns once. An image editor's
save leaves the file truncated for a few milliseconds and the debounce window
does not always cover it; degrading to the magenta checker on every save would
teach the reader to ignore the one colour that means "this path is broken".
Unreal and Unity both keep an imported asset when its source becomes unreadable.

A path that failed at load therefore gets **its own** slot holding the
placeholder's pixels rather than the shared placeholder handle. A `Sprite`
stores the handle it was given and nothing re-resolves it, so a shared handle
would make a broken path permanently broken: repairing one would mean
overwriting the texture every other broken path draws. Its own slot costs 256
bytes.

#### Rejected

- **Issuing a new handle and repointing the world.** Correct, and it makes the
  scene dirty for a change nobody made to it, plus a full world walk per save.
- **Degrading to the placeholder on a failed reload.** Flashes magenta on
  routine saves, which spends the signal that means a real broken path.

### The asset watcher is opt-in and filters by extension

**`App::with_hot_reload()` starts one recursive `notify` watch on the asset
root, debounced at 200 ms.** The editor opts in; a shipped game does not. All
four of Unity, Unreal, Godot and Bevy ship this switchable and none leaves it on
in a build.

Events are filtered by **file extension**, not by event kind. Which `EventKind`
an overwrite produces varies by platform and by the writing program, and
matching on them is what makes a watcher miss changes
([bevy#10576](https://github.com/bevyengine/bevy/issues/10576)). The extension
filter also makes the scene format's atomic save invisible for free — neither
`demo.ron.tmp` nor `demo.ron` is a texture — rather than as a rule about our own
temporary files.

Both the root and each event path are canonicalized before the prefix is
stripped, and an event that still will not relativize is logged at `debug` and
dropped. On Windows a canonical path carries the `\\?\` extended-length prefix
that a root built from `CARGO_MANIFEST_DIR` does not, and an `expect` on that
mismatch is exactly what crashed Bevy's watcher on Windows
([bevy#18342](https://github.com/bevyengine/bevy/issues/18342)). The editor's
own log shows the two forms meeting: `asset root: D:\...\assets`, then
`watching \\?\D:\...\assets`.

`AssetWatcher` produces `AssetPath`s and nothing else — no textures, no handles,
no device. There is no subscriber registry: there is one consumer, `App`, and
the shape of a second is not known.

#### Rejected

- **Godot's rescan on window focus.** No new dependency, and less machinery. It
  works there because Godot keeps a `.import` database with timestamps; here the
  equivalent means walking the tree and `stat`ing every file, which is a
  filesystem index — a bigger thing than the watcher it avoids.
- **A cargo feature, as Bevy does it.** Keeps the dependency out of a release
  binary, at the price of `#[cfg]` through `Textures` and the frame loop. Code
  behind a disabled `cfg` is not compiled, which is the shape that let Bevy's
  Windows regression through.
- **Depending on `notify` directly as well as on the debouncer.** Two entries
  can drift to incompatible versions; the debouncer re-exports the one it was
  built against.

### A line is a quad the shader widens

**WebGPU has no line width, so `voltra_render::lines` uploads each segment as a
quad carrying both endpoints, and `shaders/lines.wgsl` gives it its thickness
after projection.** Verified against the vendored source rather than recalled:
`PrimitiveState` in `wgpu-types-30.0.0/src/render.rs:385` has `topology`,
`strip_index_format`, `front_face`, `cull_mode`, `unclipped_depth`,
`polygon_mode` and `conservative`, and nothing else. `PolygonMode::Line` is
wireframe fill behind an optional feature, not a width.

Widening *after* projection is what makes the width a number of **pixels**. A
gizmo has to be the same size on screen at every zoom, and doing the conversion
at each call site means every caller recomputes pixels-per-world-unit and one of
them gets it wrong. Bevy reached the same shape by adopting `bevy_polyline` into
`bevy_gizmos`.

`ViewportBinding` carries the pixel size the shader converts with, at group 1 —
the slot the sprite pipeline gives its texture — so group 0 is the camera in
every pipeline this engine has.

`pass::draw_lines` is the engine's only pass that **loads** rather than clears.
Every other pass is the single thing drawn into its target that frame; an
overlay is by definition the second. With no mesh it records nothing at all, not
even a pass: unlike the others there is no clear to be the point of recording
one.

`Mesh::new` and `Mesh::indexed` became generic over the vertex type at the same
time. A `Mesh` is buffers and a count; what the bytes mean belongs to whichever
`VertexBufferLayout` the pipeline was built with, and `Vertex` and `LineVertex`
are two such meanings.

#### Rejected

- **`PrimitiveTopology::LineList`.** One pixel, always — invisible on a
  high-DPI display and impossible to hit-test generously.
- **Expanding on the CPU.** Fine for a gizmo's six segments, and it spreads
  screen-space arithmetic across every caller. The grid would repeat it and the
  collision overlay would repeat it again.
- **`egui::Painter`.** No GPU code at all, and the overlay would live in the UI
  layer: unreachable for a game, in a different coordinate system from the scene
  it annotates, and useless to anything that has to be drawn under the same
  camera.

### The gizmo is a persistent tool, and it hit-tests in screen space

**`Tool` is editor state, and the gizmo on screen says what a drag will do.**
Unity's model; Godot 2D's too. Blender binds the transform to a gesture instead
— `G`/`R`/`S` start a modal transform the mouse drives until a click confirms —
which is faster once learned, invisible until someone tells you the letter, and
needs modal input capture `Input` does not have: swallowing every key while
active, surviving a lost window focus, unwinding on `Esc`. It layers onto a
working gizmo later; the reverse does not.

**Handles are tested against the pixels they were drawn at**, with a grab margin
wider than the line, because a two-pixel line is a two-pixel picture and not a
two-pixel target. Testing in world space gives a target that scales with the
zoom while the picture does not, which is a standing Unreal bug report. The
centre square is tested before the arms: both arms begin inside it, so testing
it last would make the smallest target in the gizmo the unreachable one.

**A drag stores the grab point and the original translation, both in world
units,** and each frame sets `start + (cursor − grab)`. Setting the translation
to the cursor teleports the sprite's origin under the pointer the moment the
grab is anywhere but dead centre — which is every grab, since the handles are
drawn away from the origin. World units rather than screen so that resizing the
viewport or zooming mid-drag does not move the sprite. The drag holds its
`Entity`, so a release outside the viewport still ends it and a despawn mid-drag
ends it rather than panicking.

The arm layout is a pure function taking a camera, a viewport and an origin,
separated from the drawing for one reason: the property that matters is that an
arm is 60 px long at zoom 0.1 and at zoom 25, and that is a statement about
numbers. Verified through egui and a GPU it would have been verified at one
zoom, by eye.

### Three tools, one gizmo, and the camera gave up its bare keys

**`W`/`E`/`R` select move, rotate and scale.** Unity, Unreal and Godot 2D all
bind those three letters, and a shared binding is worth more than any local
improvement: it is the one thing a user of another editor already knows before
reading anything. That cost the scene camera its bare `WASD` pan and its `R`
reset — `WASD` now pans only while the right mouse button is held, which is what
Unity and Unreal do with it, and the camera goes home on `F`, which is the frame
key in both. Rebinding the camera was the cheaper side: it is one editor's
navigation, against a manipulator binding shared across the industry.

**One `Gizmo` serves all three tools.** A press picks the handle the active tool
draws, and the `Drag` carries the tool with it, so the difference between the
tools is which arithmetic runs on the anchors — not which object owns the
pointer. Three gizmo types would mean three copies of the grab, the release, the
despawned-mid-drag path and the undo claim. The drag holding its own tool is
also what makes a tool key pressed mid-drag arm the *next* grab instead of
turning a move already under way into a rotation, and what keeps the history
entry named after the verb it actually performed.

**Translate draws on the world axes, scale on the entity's own.** Unity's
Global/Local switch, with the half of it that is not a choice: a scale is
applied in the entity's frame, so an arm drawn along a world axis on a turned
entity would grow a different axis than the one it points at. The hit test
therefore takes arm *directions* rather than assuming axis alignment, and the
scale drag maps both anchors through `Mat2::from_angle(-rotation)` before
dividing.

**Rotation accumulates; scale is a ratio.** A rotate that measured the angle
between grab and cursor would snap back the short way round the moment a drag
passed half a turn, so the drag keeps a running total and folds only the
per-frame difference into `[−π, π)` — what Blender and Unity track. A scale
divides the current distance from the origin by the distance at the grab, so
dragging a handle to twice its distance doubles the entity whatever size it was,
and repeated drags compose. Both guard the degenerate case the geometry allows:
a cursor on the pivot has no angle, and a grab on the origin has no distance to
divide by. A scale component is kept clear of exact zero, because zero is a
one-way door — every later factor multiplies it and stays zero — while negative
stays reachable, since a mirrored sprite is a legitimate thing to author.

### Physics is in-house, and its components live in `voltra-scene`

**Written here rather than adopted from Rapier2D.** The reason is the ECS, not
the maths. `bevy_rapier` *"has to maintain a separate physics world and
synchronize a ton of data with Bevy each frame"* — that sentence is `avian`'s
stated reason for existing. With a hand-written ECS the same wrapper has to be
written first and then maintained, so adopting Rapier reproduces exactly the
problem `avian` was written to remove. A body is a `RigidBody` component in the
one `World`, integration reads and writes it in place, and there is no second
world to synchronise.

**`RigidBody` and `Collider` are `voltra-scene` components; `voltra-physics`
owns no component at all.** This is forced rather than preferred.
`ComponentRegistry::with_defaults` lives in `voltra-scene`, and it is what makes
a scene file round-trip. Putting the components in `voltra-physics` gives two
options and both are worse:

- `voltra-scene` registers them, so `voltra-scene → voltra-physics`. Integration
  needs `Transform`, so `voltra-physics → voltra-scene`. A cycle.
- Or the registry stops being complete on its own and every caller has to
  remember a second `register` call. Forgetting one does not fail — it silently
  drops the component from that save, and a format that loses data when a caller
  forgets a line is not a format.

So the split is the one the repo already has: `voltra-scene` is components and
the geometry they turn into, and the crate that *acts* on them is separate.
`voltra-physics` owns `PhysicsWorld`, `PhysicsClock`, `Contact`, the solver and
its tuning, and nothing a scene file contains. Nothing depends on it but
`voltra-core` and `voltra-editor`, so the cycle cannot reappear.

`step` **returns** its contacts rather than storing them in a resource, and
`PhysicsWorld` holds the last step's list for whoever asks — today the debug
overlay. A `Contacts` resource would be the same data behind a concept
`voltra-ecs` does not have, invented for one reader.

#### Rejected

- **Rapier2D or `avian`.** Above. `avian` additionally is `bevy_ecs`, which is
  the crate this project exists to not depend on.
- **Components in `voltra-physics`.** The cycle, or a registry that silently
  loses data.
- **A `Contacts` resource.** No reader; `voltra-ecs` has no resource concept and
  inventing one for a single unbuilt caller is the shortcut the hard rules
  forbid.

### A fixed step, semi-implicit integration and inverse mass

**Physics runs on a fixed step with an accumulator, never on the render delta.**
Integrating with a variable delta makes behaviour depend on the frame rate — the
same scene settles differently at 60 and 144 Hz, and one long stall makes it
explode. Unity's `FixedUpdate`, Unreal's substepping, Godot's `_physics_process`
and Box2D's own advice are all the same accumulator.

**The accumulator is capped at 8 steps and the excess is dropped, not carried.**
The spiral of death is the whole reason a cap exists: a frame slow enough to owe
many steps gets slower by running them and then owes more. Carrying the debt
only defers the spiral by one frame, so past the cap simulated time runs slow —
which every engine chooses, because the alternative is a hang.

**Semi-implicit (symplectic) Euler: velocity first, then position from the *new*
velocity.** Explicit Euler injects energy, and a stack of boxes climbs on its
own. The difference is the order of two lines and it is pinned by a test, since
after one step from rest the two differ by exactly `g·dt²` and nothing else
distinguishes them.

**`RigidBody` stores `inverse_mass`, not mass**, as Box2D does. Every formula
divides by mass, and "cannot be pushed" is then `0.0` rather than a branch
repeated at each of them. A mass of zero or less therefore reads as *infinitely
massive*, not infinitely light: `1.0 / 0.0` is `inf`, and an infinite inverse
mass sends a body out of the world on its first contact.

**`BodyType::Static` is the default, not `Dynamic`.** A `RigidBody` added by a
click in the inspector must not make the sprite fall off the screen before
anyone has typed a mass. Unity, Godot and Box2D all default to the inert body.

Everything a scene file can say is clamped where it is read, because a scene
file is external input: negative mass, a non-positive fixed step, a damping of
10 000 (which would turn `v *= 1 − damping·dt` from a brake into a catapult), a
negative scale on a collider (which would give an AABB whose min exceeds its max
and silently collide with nothing), and a zero-length vector about to be
normalised — `glam`'s `normalize` of a zero vector is `NaN`, not an error, and a
`NaN` normal spreads into every velocity a solver touches.

#### Rejected

- **Integrating on the render delta.** Frame-rate-dependent behaviour, and a
  stall that explodes the scene.
- **Carrying the capped debt.** Defers the spiral rather than breaking it.
- **Explicit Euler.** Two lines cheaper, and it adds energy for free.
- **Storing mass.** A divide and a zero-check at every use site.

### The broad phase was O(n²), and detection shipped before resolution

*Superseded for the broad phase by "The broad phase is sweep-and-prune" below.*

**`candidate_pairs` compares every pair, rejecting on world-space AABBs.** For
the tens of bodies a scene holds today this beats a spatial hash, which pays for
bucketing before it saves anything. **Replace it past roughly 200 bodies** — n²
is then 20 000 pair tests per step at 60 Hz. Sweep-and-prune on the x axis is the
replacement, because a 2D world is wide. The signature does not change when that
happens, which is what it is for.

Pairs are emitted once and never mirrored, by starting the inner loop at `i + 1`:
`(a, b)` and `(b, a)` are the same contact, and emitting both would double every
impulse the solver applies. The same bound is what stops a body pairing with
itself.

**Stage 11b-1 detected contacts without resolving them, deliberately.**
Detection is *visible*: `voltra-physics`'s debug draw puts collider outlines and
contact normals through stage 11a's line pipeline, so a wrong normal is a line
pointing the wrong way rather than a number nobody reads. The solver was then
built against contacts someone had already looked at — it is the next section,
and 11b-1's "a body sinks through the floor" no longer holds.

#### Rejected

- **A spatial hash or a BVH now.** Bucketing costs more than it saves at tens of
  bodies, and the replacement threshold is written down instead.
- **Oriented boxes and polygons.** Deferred to 11b-3, which landed the oriented
  half: `Collider::Box` turns with its transform and `world_aabb` bounds the
  rotated shape, so the broad phase still rejects on an axis-aligned box while
  the narrow phase sees the real one. General convex polygons are still open.
- **Ellipses.** A circle under a non-uniform scale takes its larger axis. A true
  ellipse is a different shape, not a parameter of this one, and the larger axis
  keeps the collider covering the sprite rather than cutting into it.

### The solver is TGS Soft: sub-step, relax, then restitution

**The contact solver is TGS Soft ("Soft Step"), the algorithm Box2D v3 uses.**
Erin Catto compared eight solvers in
[Solver2D](https://box2d.org/posts/2024/02/solver2d/) and took this one; XPBD,
the obvious alternative, lost on friction and on precision far from the origin.
It is a velocity solver with sub-stepping, soft constraints and a relaxation
pass, and every part of `step`'s order is load-bearing:

1. **Collide once**, from the positions the step starts at. The normal is then
   held constant and each contact's separation is tracked from `delta_position`,
   so the narrow phase runs once per step rather than once per sub-step.
2. **Prepare and warm start.** Masses, mixed surface, spring and base separation
   are computed once; the accumulated impulses are seeded from last step and
   applied to the velocities before anything else runs.
3. **Sub-step** four times: integrate velocities, solve the contacts with the
   soft bias pushing overlap apart, integrate positions. TGS spends its budget
   on short steps rather than on iterations over one long one — four sub-steps
   beat four passes, because gravity and position both advance between them.
4. **Relax**: solve once more with the bias off and positions frozen. The bias
   is added energy, and this pass is what takes it back out; without it a stack
   creeps upward.

**Friction runs in every pass**, after the normals and against the impulse they
have just accumulated, which is what `b2SolveContact` does. 11b-2 confined it to
the relax pass on the argument that the bias inflates the `μ·λ_n` limit, and
11b-3's slope tests showed the price: warm starting applies last step's friction
impulse up the slope at the start of the step, the sub-steps cancel it against
gravity, and with nothing resisting in between a box on a rough ramp crept
*uphill* at a steady 2 cm/s. A friction impulse that is a little too large in a
biased sub-step is corrected by the relax pass that follows; a friction impulse
that is absent for four sub-steps out of five is not.
5. **Restitution**, from the approach speed captured *before* the solve, then
   record the impulses for the next step.

The details that are not free to change: the accumulated impulse is clamped, not
the increment — clamping the increment is a classic jitter source, because a
later pass can then no longer undo an earlier overshoot. `max_push_speed` caps
how fast overlap is pushed out, so a body spawned inside a wall walks out instead
of being launched. Contact frequency is capped at `0.125/h` — a spring faster
than its sample rate is noise, not stiffness — and a contact against something
immovable is solved at twice the frequency, since all the correction has to come
from the one body that can move. A contact that separated *within* the step is
solved speculatively at `separation/h`: exactly the velocity that closes the gap
and no more.

The coefficient formulas are `b2MakeSoft` verbatim, in `solver/softness.rs`, and
the defaults are `b2DefaultWorldDef`'s: 4 sub-steps, 30 Hz, damping ratio 10.
30 Hz is not arbitrary — four sub-steps of a 60 Hz step is `h = 1/240`, and
`0.125/h` is 30 exactly, so the default sits on the Nyquist cap.

#### Rejected

- **XPBD.** Loses on friction and on precision away from the origin; Solver2D
  measured it.
- **Sequential impulses with Baumgarte.** What 11b-1 would have grown into. The
  magic bias fraction has no physical meaning, must be retuned per step rate,
  and pumps energy that nothing removes. Soft constraints are the same code with
  coefficients a human can reason about.
- **Position projection (pushing bodies apart after the solve).** Corrects the
  symptom, does not conserve momentum, and jitters when several contacts fight.
- **More iterations instead of sub-steps.** Solver2D's point exactly: iterations
  converge on a stale gravity and a stale position.
- **Friction in the relax pass only.** 11b-2's reading of Box2D, and wrong: it
  leaves the sub-steps with no tangential resistance at all, and a warm-started
  box walks up a rough slope.

### Warm starting, and the pair is what persists

**Contact impulses survive between steps, and `PhysicsWorld` owns them.** Warm
starting — seeding a contact with the impulse it needed last step — is the
single largest quality difference in the solver. Without it every step
rediscovers the force holding a stack up from zero, the lower boxes give way
before the solve converges, and the stack sinks and jitters. This is pinned by
`warm_starting_switched_off_settles_worse`, which is also why the switch is a
parameter rather than an assumption.

That is what forced `PhysicsWorld` into existence: 11b-1 got away with a free
`step` function because nothing survived a step. Something has to remember, and
this is the crate's owner for everything of that kind — the fixed clock and its
debt, the tuning, the impulses, and later sleeping islands, joints and events.

**The persistent unit is the pair, keyed `(a, b)` as the broad phase emits it.**
Box2D v3 keeps impulses on the manifold points of a persistent contact owned by
the world, matched across steps by feature ID; Godot keeps a `GodotBodyPair2D`
and inherits a previous contact's impulses within `contact_recycle_radius`;
Avian keys by the contact-graph edge's `ContactId`. All three agree on the
shape — the world owns it, the pair is the unit, an entry dies when the pair
stops being reported. What separates them is how a *point* is matched across
steps, and that became a real question the moment 11b-3 gave a manifold two
points: **the key is `(a, b, id)`, the pair plus the point's feature id.**
Box2D's answer rather than Godot's recycle radius, because our narrow phase can
name the two corners a clipped point came from exactly, and a radius is a guess
at the same question that also mismatches under fast motion.

Eviction is structural rather than a sweep: `ImpulseCache` holds two maps,
`current` is read and `next` is written, and `commit` swaps them. A pair that
separated, a body that was despawned and a scene that was reloaded all disappear
by the same rule — none of them recorded anything this step — so there is no
eviction path that can be forgotten. `Entity` carries a generation, so a
recycled index cannot inherit a dead entity's impulses either.

#### Rejected

- **A pure `step` function with the impulses in the ECS.** A `ContactImpulses`
  component belongs to no single entity; a pair is an edge, not a node.
- **One map with a "touched this step" flag.** Same effect, plus a sweep that
  has to run even on the frame that owed no step, and a bug the first time
  someone returns early.
- **Clearing the cache each step.** That is warm starting switched off, and a
  test shows what it costs.
- **Keying by entity index.** A recycled index would inherit a dead body's
  impulses and push the new one out of the scene.
- **Keying the pair alone, once manifolds carried two points.** The two points
  would share one entry, so a box resting on a floor would warm-start both
  corners from whichever was written last.
- **Godot's recycle radius.** A distance threshold that has to be tuned, and
  that matches the wrong point when a body moves far in one step. Feature ids
  are exact and cost a `u16`.

### `PhysicsMaterial` is a component, mixed at the contact

**Friction and restitution live in their own component, not on `Collider` or
`RigidBody`.** `Collider` is an enum, so two fields there would be repeated in
every variant and in every shape a later stage adds. `RigidBody` would leave
static geometry — a collider with no body, which the solver already handles —
with no surface at all, and a floor with no friction is the one surface that
matters most. Unity's `PhysicsMaterial2D` makes the same separation; ours is a
component rather than an asset reference, because that is what this ECS composes
with and an asset indirection buys nothing until materials are shared and
edited.

**A contact mixes `sqrt(fa·fb)` and `max(ra, rb)`**, as Box2D does: the
geometric mean lets one slippery surface make the pair slippery, the maximum
lets one bouncy surface make the pair bounce. Averaging both would make ice on
rubber behave like neither. A missing component reads as the default surface —
friction `0.6`, restitution `0.0` — so an entity nobody has given a material
still rubs. Both values are clamped where they are mixed, since a scene file is
external input: a negative friction reverses the friction impulse, and a
restitution above one adds energy to every bounce until the body leaves the
world.

Restitution is applied in its own pass, from the approach speed captured before
the solve, and only above `restitution_threshold` (1 m/s). Below it a resting
body would bounce forever on its own numerical noise. A contact whose
`max_normal_impulse` stayed at zero never pushed, so it has nothing to bounce.

#### Rejected

- **Fields on `Collider`.** Duplicated per enum variant, and again per shape.
- **Fields on `RigidBody`.** Static geometry would have no surface.
- **A material asset with a handle.** Indirection with no sharing to justify it
  yet; the component can gain one without moving the fields.
- **Averaging friction.** Ice on rubber behaves like neither surface.

### A contact is a manifold, and box–box is SAT with a flip bias

**A `Contact` carries up to two points, and the reason is a box that will not
stop rocking.** A box resting flat on a floor touches it along a whole face.
Given the single deepest point, the solver corrects one corner, the box tips,
the next step reports the other corner, and it oscillates forever without ever
being wrong at any one step. Two points is the smallest number that lets a
single contact apply a torque of zero. Box2D, Godot, Unity and Avian all carry a
manifold for exactly this; none of them stops at one point.

Two is also the maximum in 2D — a convex face pair clips to at most two points —
so the storage is `[ManifoldPoint; 2]` with a count, not a `Vec`. `Contact::new`
is the only constructor and truncates, so the count cannot disagree with the
array. The narrow phase returns an entity-free `Manifold`; `Contact` is that
plus the pair, which keeps the pair functions testable without a `World`.

**Box–box is Box2D v3's `b2CollidePolygons`**: the separating axis over both
boxes' face normals, a reference face, then the incident face clipped against
the reference face's two side planes. It is written against a four-vertex list
from `Collider::corners` rather than half extents, so the convex polygon of a
later stage is a different caller and not a different algorithm.

Two details are load-bearing. **The reference face only flips when B's axis beats
A's by `FLIP_BIAS = 5e-4`** — Box2D's `0.1 · linear_slop`. Two axes that separate
equally would otherwise swap between steps on a resting box, and since the point
ids are built from the face indices, every swap discards every accumulated
impulse. And **`id = (reference_index << 8) | incident_index`, built after the
flip**, so the same physical pair of corners keeps its id whichever box was
reference — which is what makes the id usable as the warm-start key above.

Separation replaced penetration at this boundary: the solver wants a signed
number that is negative while overlapping, and the sign flip now happens once,
in the narrow phase, rather than at each of the solver's uses.

#### Rejected

- **One point per contact, with the rocking accepted.** It is the visible bug of
  the whole stage; every engine surveyed carries a manifold.
- **A `Vec<ManifoldPoint>`.** An allocation per contact per step for a length
  that is at most two.
- **GJK/EPA.** The general answer for arbitrary convex shapes, and it returns
  one point — a manifold still has to be built afterwards. SAT over face normals
  gives the reference face directly, which is what the clipping needs.
- **Keeping penetration and negating in the solver.** The same flip written in
  three places, and one of them will be missed.

### Rotation: inertia is derived at gather, not stored

**`SolverBody::inverse_inertia` is computed in `gather` from the body's mass and
its collider** — `I = m(hx² + hy²)/3` for a box, `I = m·r²/2` for a disc (and
since 75a a capsule's and a polygon's, `solver/inertia.rs`), all in
world units so a scaled collider resists exactly as much as it looks like it
should. It is not a component field. Inertia is a function of two values a user
edits in the inspector, and a stored copy would need an invalidation this ECS
has no mechanism for: dragging a collider's size would leave the body spinning
like the shape it used to be. An engine whose API funnels shape changes through
a setter can cache it and recompute there; this ECS has no setter to hook, so
the derived value is computed where it is used. It costs four multiplies per
body per step and cannot go stale.

`lock_rotation` is how a body opts out, named after the same switch in every
engine that has one — Unity's `freezeRotation`, Godot's `lock_rotation`, Box2D's
`fixedRotation`. A platformer character must not topple, and its shape says
nothing about that, so it cannot be inferred.

Zero means infinitely resistant to torque, matching what zero `inverse_mass`
already meant: static, kinematic, `lock_rotation`, mass zero, no collider, and a
degenerate zero-extent collider all land there. That last one is a guard, not a
formality — `3·inv_mass/0` is infinity, and an infinite inverse inertia spins a
body out of the world on its first contact.

Angular velocity is capped per sub-step at `max_rotation = 0.25·π`. The step
collides once and holds the normal for the whole step, so a body that turns more
than a quarter turn inside one sub-step is being solved against a normal that no
longer describes its surface. Angular damping is applied as Box2D applies linear
damping, `1/(1 + h·damping)`, which cannot go negative however large the damping
is; the naive `(1 − h·damping)` reverses the spin past `1/h` and is a catapult.

#### Rejected

- **`RigidBody::inertia` as a serialised field.** Stale the moment the collider
  is resized, and a scene file could contradict its own shape.
- **A separate `MassProperties` component kept up to date by a system.** It
  needs change detection this ECS does not have.
- **Per-shape inertia on `Collider`.** Inertia depends on mass, which lives on
  the body; the collider would have to reach across.
- **`(1 − h·damping)`.** Negative past `1/h`, and it launches the body.

### Anchors, split masses, and friction on anchors that do not move

**Every contact point holds two anchors** — the point relative to each body's
centre — and they do three jobs: the torque arm, the point velocity, and the
separation tracking. The effective mass gains the rotational terms,
`k = mA + mB + iA·rnA² + iB·rnB²` with `rn = cross(r, n)`, and the normal and the
tangent now differ because the arms differ, so 11b-2's single `effective_mass`
became `normal_mass` and `tangent_mass`. A contact straight through both centres
has `rn = 0` and reduces to `1/(mA + mB)` exactly, which is pinned by a test:
rotation must not change the head-on case. A zero `k` yields a mass of zero
rather than an infinity, as Box2D does.

**The sub-steps rotate the anchors, but friction does not.** Tracking separation
through a sub-step means asking where the anchor is *now*, so the solve rotates
it by the body's accumulated `delta_rotation`. The friction Jacobian instead uses
the anchors as they were when the step began. This is Box2D's split and the
reason is drift: a friction anchor that moves with the body measures a tangential
velocity that includes its own rotation, and static friction turns into a slow
creep. Friction is clamped per point against that point's own normal impulse —
sharing one clamp across the manifold would let a lightly loaded corner borrow
grip from a heavily loaded one.

#### Rejected

- **A single anchor at the contact centroid.** That is one point again, with the
  rocking back.
- **Rotating the friction anchors too.** Symmetric and wrong: it is the drift
  above, and it shows up as a resting box sliding without a force on it.
- **One friction clamp for the whole manifold.** Coulomb's limit is per contact
  point; sharing it lets an unloaded corner grip.
- **Recomputing the anchors from the transform each sub-step.** The ECS is not
  written until scatter, so there is nothing newer to read — `delta_rotation` is
  the newer value.

### Play mode restores a snapshot, and core gets a switch rather than a mode

The editor has an edit state and a play state. Play snapshots the scene, Stop
puts the snapshot back, and everything done in between is discarded. Before it,
`voltra-editor` called `App::with_physics()` unconditionally and a box placed
with the gizmo started falling while it was being placed.

**The snapshot is a `SceneFile`**, taken with `to_scene_file` when Play is
pressed and restored with `from_scene_file` by Stop. That reuses a format which
is already the definition of what a scene is, is already tested both ways, and
already preserves components this build does not know. The captured set is
exactly the entities carrying a `SceneId`, which is the same rule `save` and
`Scene ▸ Clear` use, so an entity with no identity is transient in both
directions: never captured, never despawned.

A snapshot that cannot be taken refuses the transition. `to_scene_file` returns
a `Result`, and the failure is real — a component whose `Serialize` fails.
Entering play with no way back is the one outcome the mode exists to prevent.

Stop's order is load-bearing: simulation off, cancel the gizmo drag, despawn,
restore, reset physics, re-resolve textures, re-resolve the selection. The
selection is matched by `SceneId` and never by `Entity`, because every handle is
stale after a despawn-and-respawn. Textures are re-resolved because
`Sprite::texture_handle` is `#[serde(skip)]` — a handle addresses a slot in this
session's `Textures` and does not survive a round trip.

The transitions reach the running app through a `PlayHost` trait rather than
through `UiFrame` directly. It keeps the state machine testable — a `UiFrame`
cannot exist without a `wgpu::Device` — and it names exactly what play may
touch, which is narrower than what a panel can reach.

**Core gets a boolean, not a mode.** `App` grew `set_simulating`,
`is_simulating` and `request_steps`, and `with_physics()` became the initial
value of that switch rather than a build-time choice. `PlayState` stays in the
editor: play, pause and stop are an editor's vocabulary and a shipped game has
one mode, so putting the enum in the platform layer would make every game binary
carry it forever. What core needs to know is whether this frame steps.

One frame of latency is structural: a UI callback runs after `App::update`, so a
Play pressed this frame first steps on the next. At 60 Hz it is invisible, and
the alternative — the UI reaching back into the frame that already ran — is
worse.

Rejected:

- **Duplicating the `World` (Unreal's PIE).** The most faithful answer: play
  simulates a copy and the editor's world cannot be lost at all. It needs a
  type-erased clone in `voltra-ecs` — every component type registering a clone
  fn — and a second world for the renderer and every panel to be told about.
  Revisit if snapshot cost ever shows up in a frame; the scenes here are tens of
  entities.
- **Snapshotting to a temporary file.** Puts disk errors and a temp path into
  the one button that must not fail, to save an in-memory `SceneFile` that
  already exists.
- **A read-only inspector during play.** Less code, and it removes the reason to
  have play mode in an editor rather than in a separate binary. Unity and Unreal
  PIE both allow the edits and discard them; dragging a body while gravity acts
  on it is how a scene gets tuned.
- **Godot's separate game process.** The right answer once a runtime binary
  exists to launch, and it survives a crash in game code. There is no runtime
  binary, and a crash here is a crash of one process either way.
- **Unreal's "keep simulation changes".** Needs a per-entity diff and a UI to
  resolve it. A stage of its own; `play.rs` splits into `play.rs` + `play/` when
  it arrives.
- **Stop as an undo entry.** There is no undo stack, and pretending Stop is one
  sets an expectation the next stage would have to break.
- **Re-snapshotting on Pause.** Makes Pause destructive and Stop unpredictable:
  the paused mid-air scene would silently become the thing Stop restores.

### A fixed step is expressible on its own, and a reset clears the debt

`PhysicsWorld::step_once` runs exactly one fixed step with no accumulator and no
cap, and `advance` is written in terms of it, so there is one place in the crate
where a step happens. The editor's Step button needs the single step; `advance`
would run zero, because a paused frame owes nothing.

`PhysicsWorld::reset` also calls `PhysicsClock::reset`. The clock's accumulator
can hold up to one step of banked time, and leaving it behind meant the next
Play would open by running a step of the *previous* session's owed time against
the restored scene. The step length and the cap are configuration, not state,
and stay.

Requests reach the simulation through `App`'s switch rather than by the editor
calling `PhysicsWorld` itself: `App::update` runs before the UI callback and
`PhysicsWorld` is `App`'s, so a panel can only ask. Steps are additive and
consumed exactly once; the reset is a flag consumed at the top of the next
`step_physics`, before any step, which is the ordering Stop requires.

### Undo holds serialized records of the entities an action touched

An entry is a `Vec<EntityChange>`, each an `Option<EntityRecord>` on both sides,
addressed by `SceneId`. That is Unity's `Undo.RecordObject` and Unreal's
`FTransaction::FObjectRecord` rather than Godot's typed `add_do_property` /
`add_undo_property`: `ComponentRegistry` already converts any registered
component without the caller naming its type, so typed inverses would be a
variant per component and a case per call site for nothing. `Option` on both
sides makes spawn, delete, modify and clear one type with no per-action code —
`None` on a side means the entity did not exist there.

Rejected: a `SceneFile` per step, the shape play mode uses. Correct and already
written, but memory is O(scene × depth) and every undo despawns and respawns the
whole scene, invalidating every `Entity` handle on each keystroke.

`apply_record` makes the entity **equal** to its record rather than "at least":
a registered component the record does not carry is removed, and
`UnknownComponents` goes with it. Anything less makes undoing the addition of a
component a partial undo that reports success.

### The `before` side is captured at the top of the frame

Unreal opens an `FScopedTransaction` before the mutation because retained-mode UI
can. An immediate-mode `DragValue` has already written its new value by the time
it reports `dragged()`, and egui only reports a drag past its threshold, so a
scope opened there is several pixels late.

`History` instead watches the selection from the top of the frame, and a panel
`claim`s the open edit by name each frame the interaction is live; the entry
closes on the first frame no claim arrives. That is what makes a gizmo drag one
`Ctrl+Z` instead of one per frame. Actions the watcher cannot see — spawn,
delete, `Clear` — open and close their own entry with `begin` / `commit`, and a
spawn hands its id to `commit_including` because it has none to name at `begin`
time.

The claim rule has one hole worth knowing: a widget that edits through a popup
is neither dragged nor focused while the value moves, so the colour button
claims on the popup being open instead. Any future popup-driven field needs the
same treatment.

`watch` is also what "a new frame" means, and it clears the recorded flag an
undo sets. Relying on `end_frame` to consume that flag left an undo taken
outside a watched frame swallowing the next frame's real edit.

### A capture that fails clears the history

`record_scene_id` can fail on a component whose `Serialize` fails. Dropping just
that entry would leave a stack that lies: the next undo would restore a state
from before the unrecorded action and silently discard it. Clearing is visible
and recoverable; a history that misrepresents itself is not. `Scene ▸ Open`
clears for the same reason — every id in the stack belongs to the scene that was
just closed, which is Unity's answer.

Undo is gated on `PlayState::Editing` rather than allowed and undone on Stop:
while playing, the world is the simulation's, and the snapshot Stop restores is
the authored scene the entries are addressed against. Every apply also cancels
the gizmo drag, resets physics and re-resolves sprite textures — a `Drag` holds
an `Entity`, the contact cache is keyed by one, and `Sprite::texture_handle` is
`#[serde(skip)]`.

### The hierarchy is a link on the child, stored by identity

`Parent` lives on the child and names its parent by `SceneId`; there is no
`Children` list. Bevy carries both and spends real API surface keeping them
agreeing through every spawn, despawn and reparent — one direction cannot
disagree with itself, and the list a panel wants is one pass (`children_of`).
The cost is that finding a parent's children is O(entities) rather than O(1),
which is a panel-per-frame cost over a scene that fits in a hierarchy list.

Stored as an id, resolved to an `Entity`: exactly the shape `Sprite` already
uses for a texture (`texture: AssetPath` plus a skipped `texture_handle`), and
for the same reason — an `Entity` is an index and a generation, both recycled by
the allocator, so writing one to a file would make the file mean something
different every run. `resolve_parents` runs after every load, undo and play
restore, in one pass over an id map rather than a linear `entity_with_id` per
link. A link naming an id that is not there stays unresolved and is **kept**:
the child draws as a root, and the next save writes the id back rather than
quietly deleting a relationship a build with more of the scene could resolve.

The scene file stays a flat list in id order — Unity's shape (a transform with
a parent pointer), not Godot's (a node tree that *is* the scene). Moving an
entity in the tree is then a one-line diff, and the loader never has to order
its spawns; the price is that a child may be written before its parent, which
is why resolution is a pass after the load rather than per record.

Rules live in `set_parent` because a rule enforced at the only write is a rule:
no self-parent, no cycle, and no parent without a `SceneId` (a link that cannot
be saved is worse than a refused drag). Every walk up or down the tree — the
panel's recursion included — terminates on a cycle anyway, because a hand-edited
file can name one and the editor has to open that file rather than hang on it.

### A transform is local, and the composed one is computed per frame

`Transform` means "relative to my parent". Two shapes of consumer need the
composed matrix, and they get different tools: `world_matrix` walks one chain
for a single lookup (a gizmo, an inspector), and `WorldTransforms::from_world`
composes the whole world in one memoised pass for the callers that walk
everything (the sprite batch, picking). The pass stops each walk at the first
ancestor already composed, so a chain of depth *d* over *n* entities is O(n)
rather than O(n·d).

Rejected: a stored `GlobalTransform` component kept up to date on every write,
which is Bevy's and Unity's answer. It is the right answer when a frame reads
world transforms many times more often than it writes them; ours reads them
twice — draw and pick — and an editor writes one every frame a gizmo is
dragged. A cached component would also need invalidation on reparent, on
despawn and on any direct write to a `Transform`, which is three places to
forget. Revisit when something reads world transforms per contact.

`SpriteBatch::push` and picking's `contains` take a `Mat3` rather than a
`&Transform` for a reason worth stating: a chain of parents can express a shear
(a rotated child under a non-uniformly scaled parent) that three fields cannot,
and passing the matrix keeps the drawn geometry exact. `Transform::from_matrix`
does drop the shear — Unity has the same hole and calls it skew — and is used
only where a value has to land back in the three fields, which is a gizmo drag
on a parented entity.

### Physics and parenting are exclusive, for now

The solver reads and writes `Transform` as a world-space value, in the broad
phase, the narrow phase, the gather and the scatter. A parented body would fall
along its parent's rotated axes and resolve contacts in a frame nothing else
knows about, silently. `set_parent` therefore refuses any entity carrying a
`RigidBody` or a `Collider`, and refuses to parent anything *to* one.

Rejected for now: gathering world transforms in `SolverBodies::gather` and
decomposing back to local at scatter. That is the real answer and it is Unity's
(a child rigidbody simulates in world space), but it touches the whole physics
crate, and the decomposition is lossy exactly where a hierarchy is interesting
— a non-uniformly scaled parent. Doing it badly to avoid saying no would make
the simulation quietly wrong, which is the one failure mode physics must not
have. A file that already pairs the two is warned about on load and left alone:
dropping either component would be an edit to someone's scene made without
asking.

### Deleting a parent deletes its children

Unity, Unreal and Godot all do this, and the alternative is worse than it
sounds: an orphan keeps its local transform, which now means a world transform,
so a deleted parent scatters its children across the scene instead of taking
them with it. `despawn_recursive` returns everything it despawned so the undo
entry can name every id — an undo that revived the parent alone would leave the
children deleted for good.

A *despawn* that is not recursive (a load rollback, `Clear`) still leaves links
dangling, and that is handled the other way: an unresolved link draws as a root
and is kept. The two rules cover the two cases — a deliberate delete takes the
subtree, an accident of bookkeeping never loses data.

### The asset browser is a dock, and a drop is the placement

Unity's Project window, Unreal's Content Browser and Godot's FileSystem dock are
the same panel three times: a listing of the asset root, a thumbnail per entry,
and a drag that ends either on the scene or on a property field. All three dock
it rather than opening it as a picker, and all three instantiate where the
pointer released. Adopted whole — a modal picker cannot be dragged from, and a
sprite that appears at the origin has to be moved for no reason.

Two pieces of theirs are not adopted. There is no folder-tree pane beside the
tiles: it is a second tree next to the hierarchy, and Unreal's path bar reaches
the same directories in one row. And a directory is not scanned recursively into
one flat "all assets" view; the tree on disk is the tree on screen.

The listing lives in `voltra-assets` (`browse::list`) and the panel in the
editor, on the seam this crate already has — `AssetWatcher` produces paths and
`Textures` consumes them, with `voltra_core::App` joining the two. `browse`
knows about names, directories and extensions, and nothing about the GPU.

Everything under the root is listed, including files no loader claims. Those are
dimmed and cannot be dragged: hiding them reads as a failed copy, and letting
them be dragged would promise a loader that does not exist. Dotfiles are the one
exception, because `.gitkeep` and `.DS_Store` are bookkeeping.

The listing is re-read on a 1-second throttle rather than per frame or on a
button. Per frame is a syscall storm for a picture that changes a few times an
hour; a button is stale exactly when it matters, which is the moment an artist
alt-tabs back from an image editor. The asset watcher — which is already
running — is not the answer: it filters to the extensions that can become
textures, and this panel also lists directories and everything else.

A texture dropped on the viewport spawns a sprite named after the file, sized to
the texture's aspect with its longest side at the default scale, in one undo
entry, and refused outside `Editing` like every other edit. A texture dropped on
the inspector's texture field assigns it, and beats whatever was half-typed in
the box, because a drop is the more deliberate of the two gestures.

### A thumbnail is registered before the layout, and sampled raw

A panel cannot hand egui a texture while it is being laid out: the layout
callback runs *inside* `EguiLayer::prepare`, which already holds the layer
mutably. So `App` walks `Textures::loaded` before every layout and registers
whatever is new, and `UiFrame::thumbnail` hands the panel a finished
`TextureId`. The cost is that a texture named during a layout is drawable on the
*next* frame — the same lag, for the same reason, as "The viewport is one frame
behind" above. A blank tile for one frame is the whole of it.

Hot reload swaps a texture's contents under a stable handle, which leaves the id
correct and the view stale, addressing a texture that no longer exists. So
`reload_changed_assets` calls `update_view` for the handle whose file changed.

Sprite textures are `Rgba8UnormSrgb`, because the sprite pipeline does not
convert gamma and the sampler must. egui does convert, at the end of its own
shader, so a thumbnail sampled through that same view is converted twice and
every mid-tone darkens — the failure the viewport already had. `Texture`
therefore carries a second, non-sRGB view exactly as `RenderTarget` does, and
`Texture::raw_view` is what egui gets. Pinned by
`a_loaded_texture_keeps_its_colour_as_a_thumbnail` in `tests/headless_egui.rs`.

### A game camera is a component, and the viewport picks which camera it shows

The editor camera is not a scene camera. `ViewportCamera` moves the renderer's
`Camera2D` and belongs to the tool ("The editor owns the editor camera" above);
what a *shipped game* sees has to be authored, saved with the scene and present
in a build with no editor in it. Every engine draws this line in the same place:
Unity's `Camera` is a component on a `GameObject`, Godot's `Camera2D` is a node,
Unreal's is an actor, Bevy's is an entity — and none of them is the editor's own
viewport camera.

**`Camera` stores Unity's `orthographicSize`, not `Camera2D`'s zoom.** `size` is
half the world-space height the camera shows, so "this camera sees three units
of world" is a fact about the scene; a zoom factor is a fact about a viewport,
and the two differ by a reciprocal that would sit in a file forever. `size` is
converted at the seam, by `camera::view`, and `Camera2D::set_zoom` clamps
whatever comes out of it — a size dragged to zero produces an infinity that the
existing clamp already answers, so there is one place that decides what a broken
camera does rather than two.

**Which camera wins is `priority`, then the lower entity index.** Unity calls it
`depth` and Bevy calls it `order`; both, plus Godot's `enabled`, exist because a
scene holds several framings and renders through one, and deleting the others to
choose is not a choice anyone should have to make. `active` is the same flag
under Godot's and Bevy's name. The index tiebreak matters more than it looks:
without it the answer depends on query order, and a scene would silently change
which camera it renders through after a reload.

**Position comes from the composed `Transform`, and rotation is read for
nothing.** A camera parented to the thing it follows works before any follow
system exists, which is what `hierarchy::world_matrix` already gives for free. A
*turned* camera is not free: `Camera2D` is a position and a zoom, so rotation
would need a rotated view matrix and, behind it, a rotated `viewport_to_world`
that picking and every gizmo also read. That is a stage, not a field.

**The viewport is one panel with two modes, not two panels.** Unity, Unreal and
Godot all dock a Scene view and a Game view side by side. There is one viewport
here, so `View` switches which camera the frame was drawn with and parks the
editor camera while the game camera has it — pan and zoom survive the round trip,
which is what makes the toggle cheap enough to press. In `Game` the panel does
nothing else: no navigation, no `W`/`E`/`R`, no gizmo, no picking, no asset drop,
because none of those exist in a running game.

**Play does not touch the switch.** The transport already allows a gizmo drag
during play on purpose — it is how a body gets tuned while gravity acts on it —
and a Play that seized the viewport for the game camera would take that away
from whoever had the scene view up. Two docked views make that a non-question
for Unity; with one panel, leaving the choice with the user is the honest form
of the same answer.

**No active camera leaves the last framing on screen and says so in words.**
Unity writes "No cameras rendering" into a black Game view. The words are the
part that matters — they name a cause the author can fix — while blacking the
image out throws away the more useful of the two pictures for no gain.

Rejected:

- **A camera resource on the world rather than a component.** One camera, no
  priority, no serialisation question. It cannot express a scene with a menu
  camera and a gameplay camera, which is the first thing anyone builds, and it
  puts the framing somewhere a scene file does not reach.
- **Making the editor camera a scene entity too** (Godot briefly did something
  like this). It would put an editor-only object into every saved file and into
  every game build, and the previous entry rejected the same coupling from the
  other direction.
- **Per-camera viewport rectangles and render targets** (Unity's `rect`, split
  screen). Real, and it needs a second render target and a pass per camera. The
  frame today draws one scene into one target; this is the stage that follows,
  not a field on the component.
- **Drawing every camera's rectangle in the scene view.** A scene with a camera
  per section becomes a screenful of overlapping rectangles. The selected one is
  drawn, which is Unity's answer, and the active one is drawn solid so the
  distinction is legible without opening the inspector.

### The player is a second binary, and the command line is what a build is told

A shipped game is `voltra-player`, not `voltra-editor --game`. Unity builds a
Player, Godot exports against a template that has no editor in it, and Unreal
ships without the editor module — all three separate the two *binaries* rather
than branching inside one, because the guarantee wanted here is a compile-time
one: egui, the panels, undo, the gizmos and the asset watcher cannot end up in
a build that does not link them. A flag can be got wrong at runtime; a
dependency edge that does not exist cannot be.

The player is wiring and nothing else — parse, load, `App::run` — because
everything it needs already exists one layer down. That is the test the split
was worth making: if the player had needed engine code the editor already had,
that code was in the wrong crate.

**The scene's camera is applied by `voltra-core`, not by the player.** The
no-UI draw path frames through `camera::active_view` before it builds the
batch, so the `Camera` component means the same thing in a build as it does in
the editor's game view. Leaving it to the binary would make every game written
against this engine re-implement it, and would leave the editor as the only
thing that ever read a component authored for the opposite case. The editor
keeps its own answer — `View::show_game_camera` — because it has a second
camera to fall back to and a panel to say so in; both now go through one
lookup in `voltra-scene`, and only the "no active camera" answer differs: the
editor keeps the last framing and writes it into the viewport, the player keeps
the default framing and logs it once, latched, so a sixty-a-second complaint
does not bury the log.

**The scene loads before the window opens.** A build that cannot read its scene
has nothing to be, and a window onto an empty world looks like a working build
of a broken game. Godot says "Cannot load main scene" and stops; the player
logs the error and exits non-zero.

**Physics runs from the first frame and hot reload is off.** There is no
authoring state to protect and no reason to watch files that will not change —
the two switches `App` already had, set the way a game sets them.

**What a build is told arrives as arguments** — `<scene>`, `--asset-root`,
`--title`, `--size` — parsed in `args.rs` rather than read inline in `main`.
Godot's export templates take `--main-pack`, Unreal's shipped binary takes a map
name and `-ResX`/`-ResY`. An unknown flag is refused rather than ignored, and a
second positional argument is refused rather than merged, because additive
scene loading is a real feature and not one this stage decided on. The window
title falls back to the scene file's stem, which is the closest thing to a
project name the player has been handed.

`assets/scenes/sandbox.ron` is what the player is run against: a camera, a
floor and two crates that fall onto it. It is pinned by a test in the player
crate — the sample a `cargo run` in the README points at must keep parsing, and
a scene file is exactly the kind of artefact that rots silently.

Rejected:

- **A `--editor` flag on one binary.** One target to build, and every editor
  dependency in the shipped game forever.
- **A project settings file now** (Godot's `project.godot`, Unity's
  ProjectSettings). It is the right shape for the same information — which
  scene boots, window size, title — and it is a stage of its own, with an
  editor UI to author it and an export step to write it. The command line is
  what that stage will read *into*, so nothing here has to be undone.
- **A `voltra-runtime` library between `voltra-core` and the player**, holding
  the wiring "any game" needs. The wiring is sixty lines with one caller; a
  crate for it would be an abstraction invented before its second user.
- **Loading several scenes into one world.** Additive loading is real — Unity
  has it — but it needs an answer for duplicate `SceneId`s and for which scene
  an entity belongs to when one is unloaded. Refusing a second scene now costs
  nothing later.

### The game gets two ticks, and they run before the steps

Stage 19 shipped a binary that could show a scene and not react to a key. The
loop now hands the game its turn through two hooks — `App::with_update`, once
per frame, and `App::with_fixed_update`, once before each physics step — both
taking a `Tick`: the world, the frame's `Input`, the `delta` that applies, and
the last step's contacts.

**Two hooks, because a frame and a step are not the same clock.** Unity splits
`Update` from `FixedUpdate`, Godot `_process` from `_physics_process`, and both
for the reason the split is unavoidable here: a frame owes zero, one or several
fixed steps, so a velocity written per frame is scaled by the frame rate, while
an *edge* — the frame a key went down — is only seen once per frame and would
be read twice, or missed entirely, by a per-step hook. `Tick::input` is in both
and the doc comment says which reads belong where, because the compiler cannot.

**One context type for both.** Unity's callbacks take no arguments and read
`Time.deltaTime` or `Time.fixedDeltaTime` off a global — a tick that can read
the wrong clock. Godot passes the right `delta` in, which is the better shape,
and once `delta` is a field the two hooks want the same four things and a second
type would differ only in its name.

**The tick runs before the steps, not after.** Unity's nominal order is
`FixedUpdate` → `Update` → render, which costs a velocity set from input one
step of latency before anything integrates it. Input here is read at the top of
the frame, so the frame that saw the key is the frame that moves. The fixed tick
runs immediately before its own step, so a force it applies acts during that
step rather than the next one.

**The per-frame tick is gated on the simulation switch; the fixed tick is gated
on a step happening.** Game logic must not run while an editor is authoring —
Unity runs scripts in play mode only, and `[ExecuteInEditMode]` is the opt-in
exception — so `run_update` checks `is_running`. A *requested* step, which is
what the editor's Step button asks for while paused, still runs the fixed tick:
"advance the world by one step" has to mean the whole world, logic included.
The two paths were merged into one loop for exactly this reason; two loops meant
two places to forget the hook.

`PhysicsWorld::owed_steps` exists because of the interleaving: `advance` runs
the whole loop internally, and a caller that must do something *between* steps
cannot use it. Splitting the count out keeps `step_once` the one place in
`voltra-physics` where a step happens, and stops `voltra-core` from owning a
second clock to work the count out for itself.

**`with_physics` became `with_simulation`.** The switch was never only about
physics — it is whether the world is live — and a game with no rigid bodies had
to ask for physics to get its logic ticking. The runtime names (`set_simulating`,
`is_simulating`) already said this; the builder now agrees with them.

The platformer was the API's first user: walk with `A`/`D`, jump with
`Space` when there is something flat underfoot, and a camera that eases
after the walker. It began as an example of `voltra-core`, and became the
`games/platformer` crate in stage 76, when a game became a crate the editor
loads.

Rejected:

- **One hook, per frame only.** Simpler, and it makes every game that touches a
  rigid body frame-rate dependent — the bug both other engines split their
  callbacks to prevent.
- **A `Vec` of systems, Bevy-style scheduling.** Ordering, change detection and
  parameter injection are a scheduler, and a scheduler is a subsystem with its
  own stage. Two closures cover a game today and do not have to be undone to
  add one later.
- **Handing the callback `&mut App`.** Every private field of the platform
  layer becomes public surface, and the borrow checker stops the callback from
  touching the renderer mid-frame anyway.
- **Collision events (`on_collision_enter`).** Real, and it needs the solver to
  diff its pairs from step to step and an ordering answer for despawns during a
  callback. `Tick::contacts` answered "am I standing on something" without
  deciding any of that. The stage below decides it.

### A collider carries two masks, a sensor is a mark, events are a stream

Stage 20 gave a game its turn and nothing to react to. Every collider collided
with every other one, the example worked out whether it was standing on
anything by scanning `tick.contacts` by hand, and there was no way to ask the
world a question without a body already touching the answer. Coins, doors,
checkpoints, one-way platforms and a reliable ground check were all blocked on
the same four missing pieces: filtering, sensors, events and queries.

**Two masks per collider, not a global matrix.** Unity has thirty-two *named*
layers and one collision matrix in Project Settings; Godot, Box2D and Rapier
each put a pair of bitmasks on the collider itself. The matrix is a project
settings file, and stage 19 deliberately decided there is no project settings
file — a `CollisionLayers { layers, mask }` serialises next to the collider it
belongs to and needs no global anything. The rule is **symmetric**: a pair
interacts only when each side is on a layer the other looks at. Godot's
one-sided test, where A detects B while B walks straight through A, is the
source of its most reported confusion. The test happens in `candidate_pairs`,
before the bounds test, because it is cheaper and because a pair that can never
interact is not a candidate.

**A sensor is a marker component.** `Sensor` is Unity's `isTrigger` and Box2D's
sensor fixture; Godot's `Area2D` is a separate *node*, which has no equivalent
here because this engine has components, not nodes. It is a marker rather than
a flag on `Collider` for the reason `PhysicsMaterial` is its own component:
`Collider` is an enum, and a flag on it is repeated in every variant and in
every shape added later. `collide` splits its findings into
`Overlaps { contacts, sensors }`, so a sensor never reaches the solver, never
enters the impulse cache and never appears in `PhysicsWorld::contacts`. That
keeps `contacts()` meaning "what the solver resolved", which is what a ground
check reads: nothing can stand on a trigger.

**Events are a diff of what was touching.** `Touching` keeps the last step's
pairs and diffs them: new pairs `Began`, missing pairs `Ended`. There is no
`Stayed` — that is what `contacts()` already is, and one event per resting pair
per step would mean a settled stack generating events forever for standing
still. A despawned entity is not a special case: it stops being a candidate, so
its pairs go missing and end like any other, which is what a pickup that
deletes itself needs. `reset()` clears the set silently, or the editor's Stop
would open the next session by ending every pair of a scene that is no longer
loaded. The set is a `BTreeMap`, so the order is fixed — an event order that
changes between two runs of one scene is a bug nobody can reproduce.

**Each hook is handed every event exactly once.** Contacts are *state* and
events are a *stream*, so they cannot follow the same rule. The fixed tick
reads the last step's, since the step it precedes has not run. The per-frame
tick reads everything since its own last turn, which is why `App` buffers them:
a frame can owe two steps and `PhysicsWorld` only keeps the latest, so a pickup
taken on the first would vanish. The buffer is emptied every frame whether or
not a game installed a hook, so it cannot grow for a session. Unity delivers
its trigger callbacks in the physics loop only and leaves the per-frame case
undefined; defining it costs one `Vec`.

**Queries take `&World` and nothing else.** `query::ray`, `query::point` and
`query::overlap_aabb` are not methods on `PhysicsWorld` because they need none
of its state — which means the editor can ask, a test can ask with no
simulation at all, and a game can ask from either tick. `QueryFilter` carries
*one* mask, not the pair a collider has: a query has no layers of its own to be
looked at, which is the shape of Unity's `LayerMask` argument and Rapier's
`QueryFilter`. Sensors are skipped unless asked for, so a shot does not stop at
a trigger volume. A ray that starts inside a shape hits it at distance zero
with the ray reversed as the normal — silence there is the classic surprise,
where a ground check begun a hair inside the floor reports thin air. Box casts
are the slab test in the box's own frame, so an oriented box is one inverse
rotation away rather than a second code path, and `overlap_aabb` runs the real
narrow phase rather than comparing bounds: the corner of a turned crate is
empty space, and a query that says otherwise is worse than no query.

The editor gains the rows to author both — a `Sensor` checkbox and a toggle per
layer, hung off the *collider* rather than off the filter, because an entity
whose filter cannot be reached until it has one would never get its first — and
the debug overlay draws sensors in their own colour, since a trigger is an
invisible rectangle by design. Drawing the widget writes nothing: only a real
change inserts a component, so selecting a collider does not add a filter that
says nothing to the scene file.

The platformer example is the first user of all of it. Its ground check is now
one short ray per foot, which answers before the first contact exists and keeps
a walker whose middle overhangs a ledge standing on it, and its coins are
static sensors carrying a `Coin` component the *example* defines — nothing
registers that type and nothing in the engine knows it exists, which is what an
in-house ECS is for.

Rejected:

- **Named layers and a collision matrix.** Both need a project settings file
  and a UI to edit it. The masks work today and a name table can be laid over
  them later without changing what is stored.
- **Box2D's `groupIndex`.** A third rule that overrules the other two: nobody
  can predict the result of a filter with three interacting mechanisms.
- **`Area2D` as an entity kind.** Nodes are Godot's model, not this one.
- **A `Stayed` event.** `contacts()` is that state, every step, already.
- **Callbacks on the component (`on_enter: fn`).** A `dyn Fn` inside a
  component that has to serialise into a scene file is not serialisable, and
  the ordering questions — despawning during a callback, reentrancy — are
  exactly what a stream read in the tick avoids.
- **Filtering after the narrow phase.** Correct and wasteful: it pays for the
  bounds test, and for the manifold, of every pair that was never going to
  interact.

### A sheet is a slicing, a sprite is an index into it

A sprite used to be a whole texture, so there were no character sheets, no
tilesets and no animation of any kind. Three things fix that: a rectangle of a
texture, a named set of those rectangles, and a clock that walks through them.

**A frame is texels, not UVs.** Unity, Godot, Bevy and Aseprite's exporter all
store a region in pixels, because pixels are what a sheet is authored in and
what a slicer produces. `0.0625` is nobody's idea of a sixteen-pixel cell. The
conversion happens once, at batch time, against the sheet size the atlas
carries.

**The slicing is an asset; what a sprite shows is an index into it.** Bevy's
`TextureAtlasLayout` made the same split and it is the right one: a slicing is
geometry, so one file serves every sheet cut the same way — four palette swaps
of a character share it instead of copying it four times. Godot's `AtlasTexture`
binds region to texture and needs one resource per frame; Unity's slice table
lives in an importer `.meta` this engine has no equivalent of. `Atlas::texture`
is only an authoring hint, so the editor can assign both in one drag; nothing at
draw time reads it.

`hero.atlas.ron` is versioned like `SceneFile`, and a wrong version is refused
rather than guessed at. It describes a `grid` — cell, columns, rows, margin,
padding — or explicit `frames`, or both, because a sheet that is a grid plus two
odd rectangles is one file. `Atlases` is `Textures`' shape exactly: `load`,
`by_path_handle`, `reload`, `get`, keyed by `Handle<Atlas>`, so the two stores
have one hot-reload story between them. It needs no GPU, which is what keeps
every test of it headless. The watcher filters on the `.atlas.ron` suffix rather
than the `ron` extension, so re-cutting a sheet reloads and a scene save stays
silent.

**One function decides where a sprite's corners are.** `sprite::quad::quad`
returns the extents, the UVs and the texture to bind, and both `SpriteBatch` and
`pick::sprite_at` call it. `Sprite::HALF_EXTENT` as a shared constant was enough
while every sprite was a unit square; once a frame's size and a pixels-per-unit
decide the extents, two call sites computing them separately is a click that
lands somewhere other than the pixels it appears to, with nothing to report it.

Both callers take a `Sheets`: the atlas store, and a `TextureSizes`. A trait
rather than `&Textures` because `Textures` cannot exist without a device, and
this geometry is exactly what has to stay assertable headless — the editor and
the draw path pass `Textures`, a test passes a map. Both stores are optional,
and absent gives the unit quad every scene on disk was authored against.

**A frame that is not there draws the placeholder checker**, which is what a
missing texture already draws. Drawing nothing is a silent failure; clamping to
the last frame hides an authoring mistake in the one place it would be seen.

**The flips are explicit**, as they are in all three of Unity, Godot and Bevy,
even though a negative scale already mirrors: scale is the transform's, shared
with the collider and the gizmo, and turning a character round must not resize
its physics. `pixels_per_unit` is `Option`: `None` keeps the original rule — the
quad is one unit and the transform's scale is its size — so no scene changes
meaning.

**An animation is a list of indices, not a range.** A run cycle that returns to
frame 1 between 0 and 2 is the common case and a range cannot say it.
`SpriteAnimation` advances once per *frame*, not per physics step: what is drawn
is not what is simulated, and a clip on the fixed clock would show two frames at
once whenever a frame owed two steps and none when it owed zero. It is gated on
the same simulation switch the game's tick is, so an editor that is authoring
does not animate the sprite being placed. Looping wraps on the elapsed time
rather than by stepping the index, so a stalled frame lands on the phase it
would have reached anyway and `elapsed` stays bounded; not looping stops on the
last frame and stays, which is what a death animation is. Where a clip had
reached is never serialised — a scene that reopened mid-blink would change its
own diff on every save.

Rejected:

- **A state machine.** Unity's `Animator` graph and Godot's `AnimationTree` are
  a subsystem of their own — transitions, conditions, blending. A game today
  changes clips by writing the component, which is what Bevy's users did for
  years with a timer and an index.
- **Per-frame durations.** Aseprite exports them; a second rate per frame is a
  timeline, and a timeline wants tracks, keys and an editor for them.
- **`AtlasTexture`-per-region.** One resource per frame of every sheet, and the
  slicing stops being reusable across palette swaps.
- **A negative scale as the flip.** It resizes the collider and moves the
  gizmo, because it is the same number.
- **Clamping an out-of-range frame.** The mistake becomes invisible exactly
  where it would have been seen.

### The mixer is ours; `cpal` is a device and `symphonia` a decoder

Stage 23a. Nothing made a sound; stage 21 had already given a game the moment a
coin is taken.

**What the engines actually do.** Unity ships a native mixer of FMOD descent
and drives the platform under it, with FMOD Studio and Wwise as replacements
rather than as the thing being wrapped. Unreal's Audio Mixer is Epic's, with
XAudio2, CoreAudio and SDL as backends beneath it. Godot writes its mixer and
its buses and talks to WASAPI, ALSA, PulseAudio and CoreAudio. The one engine
that delegates is Bevy, whose `bevy_audio` is a thin layer over `rodio` — and
the ecosystem's answer to it is `bevy_kira_audio`, which people reach for the
moment they want a bus or a tween. Four data points, and the odd one out is the
one that hands the mixer away.

So the layer we own is the mixer, and the layer we take is the same one they
take: a device to hand finished buffers to, and a decoder to turn files into
samples. `cpal` is the first, `symphonia` the second. `kira` and `rodio` were
rejected not for weight but for position — both sit exactly where the mixer
belongs, and every question below would have been answered by their API instead
of by this engine.

The second reason is testability. `Mixer` and `Voice` have no thread, no device
and no clock: they are `&mut [f32]` in, samples out. Every rule in this section
is a unit test that runs on a build machine with no sound card, which is the
property a wrapped mixer cannot give and the one that made the missing-device
question easy to answer.

**The layering.** `voltra-audio` owns `cpal` and `symphonia`, the way
`voltra-render` owns `wgpu` and `voltra-core` owns `winit`. Inside it, `mixer`
and `voice` are pure arithmetic, `decode` is the only file reader, `device` is
the thin `cpal` shell with no tests of its own — what could be tested there is
`cpal` — and `audio` is the seam the engine holds. `voltra-assets::Clips` is
the cache, beside `Textures` and `Atlases` and shaped like them.

**Nothing shared, nothing locked.** The device callback runs on a thread the OS
will not wait for: a mutex it blocks on is a gap in the speakers. The main
thread posts `Command`s down an `mpsc` channel and the callback drains them
with `try_iter` before rendering, so a sound triggered this frame starts in
this buffer. Clips travel *back* out through a second channel when a voice
ends, so the last `Arc` is dropped by the thread that can afford to free a
megabyte — freeing it inside the callback is the one unbounded piece of work
that would otherwise be there.

**A silent `Audio` is a working `Audio`.** No device is not an error: it logs
once and hands back an `Audio` that accepts every call and issues real
`VoiceId`s. That is the rule the rest of the engine already follows — a missing
texture draws the checker, a missing camera keeps the default framing — and it
is also why `App::default()` needs no sound card and every test above stays
headless. A device that dies mid-run collapses to the same state on the first
failed send.

**The decisions inside the mix.**

- *Panning* is constant power: the gains are `cos` and `sin` of the angle the
  pan describes, so a sound does not swell as it crosses the centre. A centred
  sound is −3 dB in each ear, as it is on every mixing desk.
- *Falloff* is squared linear, reaching exactly zero at the source's `range`.
  Unity and Godot default to an inverse law, which never reaches zero and
  therefore needs a `maxDistance` cut-off bolted on — a step in the curve
  exactly where a sound is quietest and most likely to be crossing it. A curve
  that arrives at zero on its own has no cut-off to hear, and near the listener
  it is within a few decibels of the inverse law. A tilemap of ambient sources
  (stage 25) is what makes this matter.
- *Only x* panning. Both ears are on the same horizontal line and this is a 2D
  engine; height reaches the mix through the distance, which is where it
  belongs. A vertical pan would be a head model for an axis with no interaural
  difference.
- *Voice stealing* takes the oldest at a ceiling of 64. The sound just asked
  for is tied to something that happened on screen; the one half-played is the
  one a listener misses least. Unity's rule, and its default too.
- *Hard clipping* at the end of the buffer, because a host handed a sample out
  of range may wrap it into a crack. Replaced in 23b by a limiter, with the
  clamp kept underneath it — see below.
- *Whole-file decode*, no streaming. Streaming is a second asset kind with its
  own buffering, which is the line Unity draws between `Decompress On Load` and
  `Streaming`; nothing this engine ships is long enough to need the other side
  of it yet.
- *WAV and OGG Vorbis only*, `default-features = false`, exactly as `image` is
  PNG-only. MP3 buys nothing over Vorbis for a game's own assets.

**In the scene.** `AudioSource` carries a path and a `#[serde(skip)]` handle,
like `Sprite`, and `App` resolves every one of them when the loop resumes and
when Open replaces the world. `AudioListener` deliberately has no `priority`
where `Camera` has one: a scene holds several framings and renders through one,
which is real authoring; a scene does not hold several sets of ears. Two active
listeners is a mistake to make deterministic — lowest entity index wins — not a
knob to add. No listener at all plays every source flat with a line in the log,
which is the same call `SceneFraming` makes for a missing camera: a scene that
went silent because nobody added a component would be a worse answer than one
that plays.

`play_on_spawn` is started once, recorded by the same map that tracks which
voice each source is playing on, and the whole thing is gated on the simulation
switch — so an editor's Stop silences everything and its Play starts the
scene's sounds afresh. Sources are moved every frame from the listener; a
one-shot from `Tick::audio` is not, because it is not tied to an entity, which
is exactly what lets a coin be heard after it is despawned.

### Buses are names in code, every gain is a ramp, and a reload needs no message

Stage 23b: what 23a left — groups with a volume, fades, a limiter instead of
the clip, hot reload, and an inspector that can play the sound.

**Where the bus names live.** Unity keeps them in an `AudioMixer` asset, Godot
in `default_bus_layout.tres`, Unreal in `SoundClass` assets. This engine has no
project settings file, and stages 19 and 21 both refused to invent one, so the
table is code: `Buses::default()` is master, music, sfx, ui, and a game hands
`App::with_buses` its own. The scene stores the *name*; the mixer sees a
`BusId` resolved once on the main thread, so a file still means the same bus
after the table changes order and the audio thread never compares a string.

- *An unknown name registers a bus* at unity gain rather than being refused or
  folded into the master. Refusing silences a sound for no visible reason;
  folding plays it but leaves a slider that will never reach it. Registering
  gives a working group under the name it was typed as. Past `MAX_BUSES` (32)
  it falls back to the master — audible, with a warning — so a scene of typos
  cannot grow the table forever.
- *Routing is one level*, Godot's shape: every bus feeds the master. A bus
  graph wants topology validation and a cycle check, and earns them when there
  is an effect to put on an intermediate node.
- *An empty name is the master*, and is not written to the file, so scenes from
  before buses save byte-identical.

**Every gain is a `Ramp`.** A gain written straight into the multiply steps
inside a buffer, and a step is a click — the reason JUCE has `SmoothedValue`
and Web Audio has `linearRampToValueAtTime`. A voice carries three: `left` and
`right` (the pan law's gains, smoothed over `SMOOTHING` = 15 ms because a moving
source is re-panned every frame) and a `fader` for fades, kept separate so the
next frame's position cannot undo a fade. Linear, not exponential: the ramp
that runs most is the 15 ms one, where the curves are indistinguishable. Stops
are de-clicked over the same 15 ms; `fade_out` is the same command with a
longer time, and a fade on a stopping voice cancels the stop.

**Per-bus gain without per-bus buffers.** Each voice is handed a *copy* of its
bus's ramp at the top of the buffer and steps it frame by frame; the mixer then
advances the real one by the buffer's length. Every voice on a bus hears the
same fade at the same frame, a bus with nothing on it still arrives on time,
and nothing is allocated on the audio thread. The master's ramp multiplies the
finished sum, then the limiter runs.

**The limiter** is feed-forward, linked across channels, 1 ms attack and
100 ms release, holding 0.99. No look-ahead: it would cost a buffer of latency
on everything, which this engine spends nowhere else. A transient faster than
the attack therefore still gets through, which is why the clamp stays under it
— and why a `NaN` reaching it becomes silence there.

**`Audio` mirrors the bus volumes** on the main thread, for a settings screen
to read and so `Audio::connect` can hand a device opened late the mix a game
set on the silent `Audio` before the window existed. `App` no longer replaces
its `Audio` at resume; it connects the one it has.

**Hot reload needs no command to the mixer.** 23a expected one, because it
pictured the samples being replaced under a voice. They are not: a `Clip` is an
immutable `Arc`, `Clips::reload` swaps which one the handle names, and a voice
already playing finishes the take it started with. What *does* need doing is
restarting the scene sources playing that clip, so a looping ambience picks up
the edit; `App` stops and forgets them and `play_on_spawn` starts them on the
next frame. A file that will not decode mid-save keeps the old samples, as
textures and atlases do.

**Audition** is a button pair in the inspector, not a checkbox: nothing about
it is saved. It plays through the game's own `Audio` — one stream, through the
buses and the limiter — flat, at the source's volume, on its bus. One at a
time; a second Play stops the first. A clip dropped on the viewport becomes an
entity with an `AudioSource` and no sprite, Unity's answer.

### A prefab is a scene file, and an instance forgets it

Stage 24a. Everything after this stamps out copies — enemies, bullets, tiles,
particles — and should share one mechanism.

**Prior art.** Unity: a prefab asset, instances carrying overrides, variants.
Godot: a scene is a node tree, instancing inherits it, "make local" breaks the
link. Unreal: blueprint classes. All three separate the asset from the
instance, and all three have paid for override propagation.

**The asset is the format that already exists.** A prefab is a `SceneFile` —
one or more roots, the same `.ron`, read through the same `ComponentRegistry`
— so nothing new has to read one and any scene can be instanced. `Prefabs`
lives in `voltra-scene`, not beside the other stores in `voltra-assets`,
because parsing one is this crate's format and depends on its components.
Version is checked once at read, not per spawn.

**`prefab::spawn(world, registry, prefab, at)`**, all-or-nothing like a load:

- *Fresh `SceneId` per entity*, so two instances never save under one id.
- *Parent links inside the prefab are remapped and resolved on the spot.* No
  whole-world `resolve_parents` pass: every entity a link can name was made by
  this call, which matters for a bullet spawned every frame into a big level.
- *Links to a parent outside the prefab are cut* and the entity becomes a
  root. A fragment cut from a bigger scene still names its old parent, and
  following it would hang every instance off one entity.
- *Roots are placed by `at`*, composed in front of their own transform; a
  translation-only `at` is an addition, so an authored rotation and scale are
  not put through a matrix decomposition for nothing. Children stay local.
- *Instantiate and forget* — until stage 64, which keeps instances in step
  with their file (below). Since 24b each root carries a
  `PrefabInstance { source }`, and since 64 every entity a `PrefabPart`.

**The tick spawns, the loop resolves.** `Tick::spawn` cannot resolve a sprite's
texture — that needs the GPU, which a tick is deliberately not handed — so it
queues the entities, and `App` resolves textures, slicings and clips *per
entity* right after each tick (and after the steps, for the fixed one). An
entity is complete before anything but the tick that made it sees it. The
prefab store is lazy and needs no window, so headless runs spawn too;
`App::spawn` is the setup-time twin, and `App::with_registry` is how a game's
own components (the platformer's `Coin`) ride in a prefab.

**Hot reload** reaches prefabs through the watcher, which now treats any `.ron`
that is not `*.atlas.ron` as an asset event. Since stage 64 existing instances
take the edit too, but for what they override; the next spawn gets it whole.

### The instance link, and authoring prefabs in the editor

Stage 24b.

**`PrefabInstance { source: AssetPath }` on every instance root.** Unity's
`m_SourcePrefab`, Godot's `instance=ExtResource(...)`. Recorded and inert until
stage 64 read it back (below) — and that stage found every scene saved before
it already saying what each instance came from.
`Prefabs::load` stamps the read path on the `Prefab` (`with_source`), and
`spawn` writes the link on each root, *replacing* any link the root's record
carried: a root is an instance of what it was just spawned from. A child's own
link is kept — that child is a nested instance. A `Prefab` built in memory has
no source and links nothing.

**Nested prefabs are records, not references.** A file holding an instance of
another holds that instance's entities in full, plus its link. A spawn copies
records and never follows a link, so a cycle cannot hang a spawn today. It is
refused anyway, at the one place one can be made — `prefab::write` — because the
propagation stage *will* walk these links. `prefab::reaches` walks the nested
sources (links on records parented inside the file; a root's link says nothing
about content) through the `Prefabs` cache with a visited set, so a diamond is
walked once and a hand-written loop that misses the target ends.

**`prefab::fragment` cuts a root and its descendants out of a world.** On the
root only: the parent link is dropped (it names an entity the prefab does not
hold), its own `PrefabInstance` is dropped (it *is* the prefab now), and its
translation is zeroed while rotation and scale stay — a spawn places roots
around `at`, so a prefab saved where it stood in the level would land that far
from every cursor. Godot's saved branch is likewise local to its own root;
Unity keeps the position and authors reset it by hand. `format::write` is split
out of `save` so the fragment comes out formatted exactly like a scene.

**Editor.** The browser lists any `.ron` that is not a slicing as
`EntryKind::Scene`, draggable like a texture. Dropping one on the viewport
spawns it through the *same* `Prefabs` the game's ticks use (now on `UiFrame`),
so a hot-reloaded prefab drops as it now is; every entity goes in one undo
entry (`spawn::record_all`), and the spawned assets are resolved per entity
(`UiFrame::resolve_assets_of`, sharing `spawning::resolve_entity` with the
loop's queue). **Scene ▸ Create prefab from selection** writes the selection to
the browser's open folder as `<name>.ron` — `<name> N.ron` when taken, never an
overwrite, Unity's rule — then links the selection to it as one undo entry. The
undo removes the link and leaves the file: an undo that deleted assets would be
the only one in the editor that touched the asset folder.

### A tilemap layer is one entity, its chunks live inside it

Stage 25a. The 2D content path, on top of stage 22's atlas.

**Prior art.** Godot 4 split `TileMap` into one `TileMapLayer` node per layer,
with rendering quadrants (16 by default) inside each. Unity: a `Tilemap`
component per layer plus `TilemapRenderer`. bevy_ecs_tilemap: an entity per
tile. Tiled and LDtk: infinite maps stored as chunks.

**One entity per layer, not per chunk or per tile.** The plan sketch said one
entity per chunk; dropped, because painting across a chunk border would then be
an entity spawn, and the hierarchy would list chunks nobody authored. Per tile
makes a 100×100 level ten thousand entities that every query, save and
hierarchy walk carries. A layer ("ground", "foliage in front") is what an author
thinks about, so it is the entity, with its own `Transform`, `sort_order` and
place in the hierarchy.

**Sparse chunks, dense cells.** `Tilemap` holds a `BTreeMap<(i32, i32), Chunk>`,
each a 16×16 boxed array. Cell → chunk is `div_euclid`, so cell `-1` is chunk
`-1`. Painting at negative coordinates or on a far island costs one chunk;
erasing a chunk's last tile drops it, so an erased map saves as nothing. Cell
`(x, y)` covers `x..x+1` × `y..y+1` tiles in the entity's space: its lower-left
corner is on the origin, as in Godot, Unity and Tiled.

**Cells use Tiled's encoding.** `0` is empty, otherwise frame + 1 with flip X/Y
in bits 31/30, so a frame tops out at 2³⁰ − 2 and `set` refuses (logs) anything
above it rather than wrapping. On disk a layer is `chunks: [(at: (x, y),
cells: [...256])]` in key order, so two saves are byte-identical; a chunk of the
wrong length or listed twice is a load error, not a guess.

**The tileset is an atlas; the layer keeps both paths.** Like `Sprite`, a layer
names `texture` and `atlas` and resolves both handles through the same stores,
in the same places (`resolve_world_*`, the per-entity `spawning::resolve_entity`).
A cell is a frame index into the atlas.

**Meshed into the sprite batch.** `SpriteBatch::from_world` collects sprites and
layers into one list sorted by `draw_key(entity, sort_order)` (now taking the
order, not a `Sprite`), so a layer sits behind or in front of sprites by number.
`tilemap::mesh::push` emits one quad per tile through `SpriteBatch::push_quad`,
which `push` now uses as well; a layer binds one sheet, so it is one range. A
frame the atlas lacks draws the placeholder, as a sprite's does. Rebuilt every
frame with the sprites; caching a mesh per chunk, and culling chunks against the
camera, are the next steps when a level makes them show.

**Editor.** An inspector section edits colour, sort order, tile size, the two
paths and the collision flag, and shows the tile count and bounds; **Scene ▸
Spawn tilemap** adds an empty layer at sort order −1. A click selects a layer
where it has a tile — not its bounding box, which would cover the sky over an
L-shaped level — weighed against the sprite under the same point by the same
`draw_key` (`pick::entity_at`).

### A solid layer's tiles become generated static boxes

Stage 25b.

**Solidity is per layer** (`Tilemap::collision`), Unity's `TilemapCollider2D`
shape. Godot marks it per tile in the TileSet's physics layer instead, which
lets one layer mix ground and decoration; a level keeps those on separate
layers anyway, and per-frame solidity in the atlas is a later addition that
this flag does not block.

**Generated entities, not a compound shape.** The physics keys every contact,
cached impulse and event by `Entity` and knows one collider per entity. A
"tilemap collider" addressing parts of an entity would have to teach all of
that; a static box per merged rectangle needs nothing. Each box is an entity
with `Transform`, `RigidBody::new_static()`, `Collider::Box` and a
`TileCollider { layer }` mark, plus copies of the layer's `CollisionLayers`,
`PhysicsMaterial` and `Sensor`. No `SceneId`, so never saved, listed or picked
— derived data, like a mesh. Events name the box; `TileCollider::layer` maps
back. Physics reads a collider's `Transform` as world space, so a box is
placed through the layer's composed matrix: a parented, turned or scaled
layer is solid where it is drawn.

**Merged greedily** (`tilemap::solid_boxes`): runs per row, a run grows upward
while the row above holds the identical run. A solid block is one box
whatever its size — the case that matters for both broad-phase count and the
seam a body catches on between two boxes. Not the minimum cover; linear and
deterministic instead.

**Rebuilt when stale, checked every frame.** `sync_colliders` keeps on each
box what it was built from — the layer's `Revision`, tile size, world matrix
and copied filter — and rebuilds a layer whose inputs differ; boxes whose
layer is gone or no longer solid are despawned. `App::advance` calls it before
the physics steps (editing too, so the overlay shows them) and after each
fixed tick inside the step loop. A rebuild means new entities, so bodies on a
layer painted mid-play lose one step of warm start; reusing entities is not
worth the bookkeeping yet.

**`Revision` compares equal to every other revision.** A fresh number from a
global counter on construction, on deserialize and on every `set`/`clear` that
changes something; clones share it. Its `PartialEq` is always `true` so the
derived `Tilemap` equality still means "same cells". An undo replaces the
component with a deserialized copy, which is a fresh revision, so the boxes
rebuild rather than trust a number that happened to match.

### Painting is a viewport mode while a layer is selected

Stage 25b. Godot's TileMap editor shape: with a paint tool armed and a
`Tilemap` selected, a drag paints instead of grabbing the gizmo and a click
paints instead of selecting. Unity's separate Tile Palette window owns the
scene view the same way while one of its tools is active.

**Keys are Unity's Tile Palette ones** — `B` brush, `U` rectangle, `D` eraser —
for the reason `W`/`E`/`R` are the transform keys. Godot's put the rectangle
on `R`, which is Scale here. Tool keys are ignored while the right button is
held, since that is the WASD fly chord and `D` is in it. `W`/`E`/`R`, or
selecting anything that is not a layer, disarms painting, so the toolbar never
shows a paint tool pressed over a gizmo.

**A stroke paints the segment, not the point.** A fast drag crosses several
cells per frame; the brush and eraser walk every cell from the previous
frame's (Bresenham) so the line has no gaps. A rectangle fills on release, and
refuses — whole, logged — anything over `MAX_FILL` (2²⁰) cells, so a mis-drag
across a zoomed-out view cannot freeze the editor.

**One stroke, one undo entry.** The history watches the selection and a stroke
edits only the selected layer, so it claims the open entry each frame it is
live, the gizmo's mechanism. The stroke carries its own tool, so a key pressed
mid-stroke arms the next one.

**The palette is the tileset's frames, drawn from the sheet thumbnail**, in
the inspector beside the tileset path. Picking a tile arms the brush if
nothing is armed. The brush is editor state, not a scene edit.

Not done, on purpose: a per-chunk mesh cache and camera culling (no level
makes the per-frame rebuild show yet), auto-tiling, animated tiles,
isometric/hex grids, per-tile solidity. (Culling landed in 33, animated
tiles in 38, auto-tiling in 39–40, the mesh cache in 44.)

### Text is quads in the sprite batch, from one glyph atlas per font

Stage 26a. A `Text` (content, optional font path, `size` in entity units,
`pixels_per_em` raster resolution, colour, `Align`, `sort_order`) is drawn in
world space by the sprite pipeline: one quad per inked glyph, pushed into the
same `(sort_order, entity)` list as sprites and tilemap layers, so a label sorts
among them like any other drawable. No second pipeline and no screen-space pass
yet — that is 26b.

- **`fontdue` rasterizes; the layout is ours.** `fontdue::layout::Layout`
  applies no kerning and aligns only inside a `max_width` box. Our
  `voltra_assets::fonts::layout` is pens along `
`-split lines with
  `horizontal_kern_indexed`, plus each line's width; the scene aligns about the
  entity's origin. Origin is the first baseline at the aligned end.
- **`Fonts` is CPU-only except `upload`.** Same shape as `Textures`/`Atlases`:
  load once per path, stable handle, reload in place. `fonts/Ubuntu-Light.ttf`
  (egui's face, UFL beside it) is `DEFAULT_FONT`, loaded at construction and
  registered under its path. A file that will not parse gets its own slot with
  a clone of the default face; with no default either, a faceless slot that
  draws nothing. A reload resets the font's atlas (glyph indices may mean other
  shapes) and keeps its texture handle.
- **One atlas per font, keyed by `(glyph, px)`.** Shelf packing (best fit by
  height), 1 px padding right/below, white RGB with coverage in alpha — alpha
  is linear in `Rgba8UnormSrgb`, so it reaches the blend as rasterized and the
  vertex colour tints white. Starts 256², doubles and repacks (keeping each
  glyph's coverage, no re-raster) up to 4096²; a glyph that cannot fit is
  logged once and remembered with no rect. Pinned by
  `voltra-scene/tests/headless_text.rs`.
- **Prepare, upload, then batch.** A growth moves every glyph, so UVs are only
  valid after the frame's last insertion. `text::prepare` rasterizes every
  text's misses, `Fonts::upload` replaces dirty atlases through
  `Textures::insert_generated` / `Textures::replace` (stable handle, new texture
  + bind group), and only then does `SpriteBatch::from_world` read them.
  `Sheets` carries `fonts`, so draw and pick ask the same store.
- **Picking is the laid-out box**, ascent to last descent, through the world
  matrix — a click between letters is a click on the label (Unity, Godot).
- Colour alpha is drawn since stage 34, when the vertex became RGBA.

### Screen-space UI is a `UiRect`, drawn in a second pass in pixels

Stage 26b. Display-only — a HUD; input, buttons and focus are later.

- **`UiRect` is Godot's anchors + offsets** (Unity's `RectTransform` without
  the pivot): `anchor_min`/`anchor_max` are fractions of the parent rect,
  `offset_min`/`offset_max` points added to the anchored corners. Equal
  anchors pin a fixed-size box; spread anchors stretch it. `pinned(anchor,
  offset, size)` and `stretched(margin)` are the two shapes everything is
  authored with. Crossing offsets collapse to zero size rather than mirror.
- **Points, y down, origin top-left.** Every UI toolkit authors that way, and
  the world's y-up would make "16 points below the top" negative. Points are
  physical pixels / window scale factor, so a HUD keeps its size on a dense
  display; in the editor that is egui's points, which is what makes the
  viewport pick line up with the draw.
- **Parent = the nearest ancestor with a `UiRect`, else the screen.** Through
  the existing `Parent` link — no second hierarchy. `UiLayout::from_world`
  resolves every rect once per frame (memoised, cycle-safe), so a resize is
  just the next frame.
- **What draws:** an entity with a `UiRect` is screen-only — excluded from the
  world batch and the world pick, its `Transform` ignored. Its `Sprite` fills
  the rect (frame, region, flips kept); its `Text` is laid out in it, first
  baseline one ascent below the top, aligned to the left edge / middle / right
  edge. Both on one entity: sprite under text. Order is `(sort_order, UI
  depth, entity index)`, so a child draws over its panel by default.
- **UI text is rasterized at its on-screen size** (`ui_px` = size × scale,
  `pixels_per_em` ignored) and its origin snapped to a whole pixel, so glyph
  texels map 1:1. `text::prepare` takes the scale for exactly this.
- **Render:** `voltra_render::screen_projection(w, h)` — not a `Camera2D`,
  whose zoom floor cannot reach pixel units — in a second `CameraBinding`
  (`sharing_layout`, its own buffer: two `write_buffer`s to one buffer before
  a submit keep only the last). `draw_mesh_batches_over` loads instead of
  clearing. `Renderer::render_mesh` / `render_scene` take a `Layer` drawn after
  the world in the same encoder. Pinned by `voltra-scene/tests/headless_ui.rs`.
- The editor draws the HUD over its viewport (what a build shows) and picks it
  before the world.

### Input actions are a map file, evaluated into `Input` once per frame

Stage 27a. Godot's `InputMap` (named actions, each a list of bindings) with
Unity's rule that edges belong to the action, not the key.

- **`ActionMap`** in `voltra-core` (it names `winit` types, which only core
  may): `BTreeMap<String, Action { bindings: Vec<Binding> }>`, `Binding =
  Key(KeyCode) | Mouse(MouseButton)` — physical keys, so WASD survives AZERTY.
  RON with a `version`; a `BTreeMap` so a saved map diffs by the line that
  changed. `winit`'s `serde` feature is on for this.
- **Where it comes from:** `assets/default.input.ron` by convention, or
  `App::with_input_map(path)`, or `App::with_actions(map)` from code. No file
  is normal (debug line); a named-but-missing or broken one warns and the game
  runs with no actions. `*.input.ron` is its own kind: not a scene, listed in
  the browser, not draggable. Hot reloaded; a reload replaces runtime
  rebinds, because the file is what the developer is editing.
- **Evaluation:** `Input::update_actions(&map)` at the top of
  `App::advance`, once per frame, so the per-frame and fixed ticks see the
  same actions. Held = any binding down; pressed = not held last frame and
  (held now or a binding went down); released = held last frame and not now,
  or a sub-frame tap. Two keys on one action are one press. An unknown action
  name reads as never-held and warns once (a typo is a missing texture).
  `action_axis(neg, pos)` and `ActionState::strength` (0/1 now) are the
  digital face of what 27b's sticks will feed.
- **Rebinding** is `tick.actions: &mut ActionMap` plus
  `Input::pressed_binding()` (keys before buttons, lowest first —
  deterministic), into `ActionMap::rebind(action, slot, binding)`, which also
  takes the control off every other action. Seen from the next frame.
  `ActionMap::save(path)` writes atomically; where a player's settings live is
  the game's call until a build manifest (stage 29) names a game.
- **One player.** Every device drives the one map. Players/actors are a later
  stage and can be added beside this API rather than through it.
- Raw reads (`is_key_down`, …) stay: an editor shortcut is a key.

### Gamepads are bindings, merged across pads, analog through a dead zone

Stage 27b. `gilrs` 0.11 in `voltra-core` only, in one file
(`app/gamepads.rs`) that turns its events into `Input`'s mutators —
`on_pad_button`, `on_pad_button_value`, `on_pad_axis`,
`on_pad_disconnected` — so everything else is tested without a pad.

- **Bindings** `Pad(PadButton)` and `PadAxis(PadAxis, Direction)`, in our own
  enums by position (`South` = A / Cross). Not `gilrs`'s: its `LeftTrigger`
  is the bumper and `LeftTrigger2` the trigger, and the file format should not
  follow a backend. Stick Y is positive **up**, as `gilrs` reports it and as
  the world runs. `C`, `Z`, the `Z` axes and the D-pad axes are dropped —
  standard pads report triggers and the D-pad as buttons.
- **One player, any pad.** State is kept per pad (so one pad letting go of a
  button another holds is not a release, and a disconnect drops only that
  pad) and read merged: a button is down if any pad holds it, an axis is the
  pad pushing furthest.
- **Analog.** Every binding has a value `0..1` (keys 0/1, triggers their
  travel, a stick direction how far it is pushed). An action's raw value is
  its bindings' max; it is held above the action's `deadzone` (per action in
  the file, default 0.25, clamped to `0..=0.95`), and `strength` is the travel
  past the dead zone rescaled to `0..1`, so `action_axis` is analog and a walk
  does not jump from nothing to a quarter.
- **Rebinding** sees pads: `pressed_binding` reports pad buttons after keys
  and mouse, then a stick direction crossing 0.5 this frame (resting drift
  never rebinds). As a raw binding a stick direction is "down" past 0.5.
- **Focus.** `gilrs` is not tied to the window, so pad events are ignored
  while it is unfocused and held pad controls are released on focus loss, as
  keys are.
- **Polled** at the top of `App::update`, before the frame's actions are
  derived. `Gilrs::new` failing is logged; the game runs without pads.

### The broad phase is sweep-and-prune, and settled islands sleep

Stage 28, measured before and after with `cargo run --release -p
voltra-physics --example stress` (10 simulated seconds, this machine):

| Scene | Before (O(n²)) | Sweep-and-prune | + sleeping |
| --- | --- | --- | --- |
| level: 2 000 static tiles, 150 crates | 12.3 ms/step | 1.2–1.5 | 1.0–1.2 |
| pyramid: 210 boxes, settled | 1.5–1.7 | 1.0–1.1 | **0.40** (all asleep) |

- **Sweep-and-prune on x** replaces the n² loop behind the same
  `candidate_pairs` signature (`sweep` also returns the bounds-test count,
  which the tests pin — a timing is not a test). Pairs with no dynamic side
  are dropped unless one is a sensor: tile against tile, wall against
  kinematic platform. That also removes static–static contacts from
  `PhysicsWorld::contacts`, which Box2D never makes either. Pairs come out
  sorted by entity index, so the solve order no longer depends on storage
  order.
- **Sleeping is Box2D's island rule.** A dynamic body below
  `SleepParams::{linear,angular}_tolerance` accumulates rest; an island
  (dynamic bodies joined by solid contacts — static and kinematic bodies join
  nothing) sleeps when its least-rested member has rested `time_to_sleep`. A
  moving kinematic body *drives* what it touches, which never sleeps.
- **Defaults: 0.1 u/s, 5°/s, 0.5 s.** Twice Box2D v3's linear tolerance: this
  solver leaves a settled 20-row pyramid at ~6 cm/s of jitter, which never
  sleeps at 5. 0.1 u/s is Unity's default (its 0.005 kinetic-energy-per-mass
  threshold). Crates on the *seam* between two per-tile floor boxes jitter
  above it and stay awake — the ghost-collision limit; real tilemaps merge
  their boxes (25b), so the stress floor is the pessimistic case.
- **Waking**, before the collide, from the body's own state: a velocity
  written by the game, a transform moved from outside, a type change,
  `can_sleep` turned off, or a *partner* (anything it rested against) gone,
  stripped of its collider, or moved. Partner state rather than "is the
  contact still reported": edge-to-edge boxes flicker in and out of contact,
  and waking on that kept a whole level awake. After the collide, a sleeping
  body touching an awake dynamic body or a moving kinematic one wakes, and
  its sleeping neighbours with it (a flood fill, built only when there is a
  seed — the usual settled step builds nothing).
- **What sleeping skips:** integration and the solve (sleeping bodies are
  `freeze`d in `SolverBodies`), and the narrow phase — a pair with nothing
  awake reuses the contact it rested with, as Box2D keeps sleeping contacts.
  Its impulses are carried over (`ImpulseCache::keep`) so a woken stack does
  not jolt. `contacts()` and the events still include sleeping pairs: a
  resting character is still standing on the floor, and falling asleep is not
  an `Ended`.
- **State lives in `PhysicsWorld`**, not on the component — a scene file
  does not say a crate is asleep. `RigidBody::can_sleep` (serde default
  `true`) is the one authored switch; `PhysicsWorld::{is_asleep, wake,
  asleep_count}` and `SolverParams::sleep` (`enabled: false` restores
  always-awake) are the code-side ones. `reset` wakes everything.
- **Measured, then fixed:** the first version spent ~650 µs per settled step
  in bookkeeping against 3 µs of solve. SipHash on entity keys and island
  building every step were most of it; `physics::hash::FastMap` (FxHash's
  mix, in-house) and seed-only flood fills took it out.

### A build is a manifest, and `cargo xtask dist` packages it

Stage 29. The project settings file stages 19 and 21 turned down, admitted
now because it describes *a build*, not an editor (Godot's export preset,
Unity's build settings).

- **`build.ron`** — `version`, `name`, `scene` (an `AssetPath`, relative to
  the asset root), `window`, `assets` (relative to the manifest, never
  absolute, so a build folder can move). Parsed in `voltra_player::manifest`,
  the player's one library module, because the player and `xtask` both read
  it: one parser, not two. Checked before use: version, a name that survives
  as a file name, a scene that cannot climb out of the root, a window with a
  size.
- **The player** runs the manifest beside its executable when started with
  no scene — a double-click runs the game. `--manifest FILE` names another;
  a scene on the command line without it is a plain run as before. The
  command line wins field by field.
- **`cargo xtask dist`** (`crates/xtask`, the slot ARCHITECTURE held; alias in
  `.cargo/config.toml`) builds the player in release with `windowed`, copies
  it as `<name>.exe`, copies the asset tree loose (no hidden files, no
  `*.tmp`; no pruning — that needs a dependency graph), and writes the
  shipped manifest with `assets: "assets"` into `dist/<name>/`. A bad manifest
  or missing start scene fails *before* the three-minute release build. The
  output folder is only emptied if it is empty or holds a `build.ron` — a
  mistyped `--out` cannot delete someone's documents.
- **`windowed`** (a player feature only `dist` turns on): no console window
  on Windows, and the log goes to `log.txt` beside the executable, so a build
  that fails on a double-click says why somewhere findable. `cargo run -p
  voltra-player` is unchanged.
- **Scene-only games**, until stage 36 added `game:` for a game's own
  binary. Windows first; Linux and macOS take the same task with no suffix and
  are untested.

### A tick changes scene by asking, and the loop does it at the end of the frame

Stage 30. Unity's `SceneManager.LoadScene` (single and additive) and Godot's
`change_scene_to_file`, as three `Tick` calls: `load_scene(path)` replaces,
`add_scene(path) -> SceneLoad` loads beside, `unload_scene(load)` takes an
additive load away again.

- **Deferred** to the end of `App::advance`, after the ticks, the steps and
  their spawns — Godot defers the same way. Nothing a tick holds is despawned
  under it, and the new scene's first frame is a whole frame. Several
  requests in one frame apply in order.
- **A scene file is read as a prefab is**, through the prefab store, so
  hot reload, caching and fresh ids come for free. `prefab::spawn_scene` is
  `spawn` without placement and without stamping a `PrefabInstance` on the
  roots: a level is not an instance of itself, and the coin prefabs placed in
  it keep their own links.
- **Replace spawns first and despawns after**, so a file that will not load —
  or whose components will not read — leaves the old scene standing, logged.
  What goes is every root, by `World::entities()` (new: a component query
  would miss what a game made in code), except those marked `Persistent` —
  Unity's `DontDestroyOnLoad`, and like it a property of roots: a persistent
  child of a doomed root goes with it. Physics contacts, sleep and pending
  events are reset, since they name a world that is gone.
- **Additive loads are tracked by handle**: the entities each brought, so an
  unload despawns exactly those still alive. A scene change sweeps every
  additive load and its handle; unloading a swept handle does nothing.
- The platformer's flag loads `scenes/platformer-2.ron` with the walker,
  camera and HUD carried over.

### A save is the game's own type in a named slot, in a per-user folder

Stage 31. Unreal's `SaveGameToSlot` shape rather than a world snapshot: a
world holds what the *level* is, and is mostly rebuilt from its scene file;
what a save needs is what the *player* did, which only the game knows. So the
game hands `Saves::save(slot, &T)` any `Serialize` type and reads it back
with `load::<T>(slot) -> Result<Option<T>>`; the engine owns where, how and
whether to trust it.

- **Where**: `%APPDATA%/<game>/saves` (Windows), `~/Library/Application
  Support/<game>/saves` (macOS), `$XDG_DATA_HOME` or `~/.local/share` (other
  Unixes) — Godot's `user://`, resolved from the environment rather than a
  dependency. `VOLTRA_SAVE_DIR` overrides everything; relative environment
  paths are ignored, as XDG requires. `<game>` is `App::with_game_name`, the
  window title by default, and the build manifest's name in the player —
  sanitised by the one `saves::dir::safe_file_name` the manifest also uses, so
  a build and its saves agree on what the game is called.
  `App::with_save_dir` is the portable/test escape.
- **Slots** are `<slot>.save.ron`, named `[A-Za-z0-9_-]{1,64}` — which is
  what keeps a slot inside the folder: no separators, no dots, no drive
  letters. `slots()` lists only files that are valid slots.
- **Writes are atomic** (temporary file, then rename). **Reads are bounded**
  (16 MiB, checked before reading): a save is a file the player can edit or a
  full disk can corrupt. A file that will not parse as `T` is an error, not
  `None` — an old-version save is something to tell the player about, not to
  start over from silently. Versioning the type is the game's (a field in
  `T`), since only it knows what changed.
- **Synchronous**, from `tick.saves`: a save is small and a checkpoint wants
  it on disk before anything else can go wrong. `Saves::path_of` also gives
  `ActionMap::save` a home beside the slots.
- The platformer saves its coin count and the walker's position on F5 and
  restores them on F9.

### Parallax is a draw-time displacement, seen from the drawing camera

Stage 32. `Parallax { factor: Vec2 }` on an entity draws it and everything
under it as if the camera had moved only `factor` of how far it did — Godot's
`Parallax2D::scroll_scale`, per axis. `1` is the world, `0` is pinned to the
camera, `0.3` is far hills.

- **In `WorldTransforms::seen_from(world, camera)`**, not in each drawable:
  the composed matrices of a layer are displaced by `camera × (1 − factor)`,
  so sprites, tilemap layers and texts all get it at once, and a world with
  no `Parallax` pays one empty query. The nearest `Parallax` at or above an
  entity applies; they do not multiply.
- **The camera travels in `Sheets`** (`seen_from`), which both the batch and
  every pick already take — the reason `Sheets` is one parameter is that the
  draw and the click must ask the same question. The draw path passes the
  renderer's camera; the editor's pick passes the viewport's. The origin, the
  default, draws every layer where its transform says, so every caller that
  does not care is unchanged.
- **Physics does not see it.** A parallax layer is decoration; a collider
  under one stays at its transform. The gizmo edits the transform, so in the
  editor a dragged layer moves by its own offset from where it is drawn —
  acceptable for decoration, worth revisiting if layers get interactive.
- **Repeating** (stage 37): `Parallax::repeat: Option<Vec2>`, Godot's
  `repeat_size`. Each drawable under the layer is drawn again every period,
  at the offsets that cover the view plus one period of margin either side —
  computed per drawable from its own position, so a layer of several hills
  tiles as one. Per axis (`0` does not repeat on it), capped at
  `MAX_REPEATS` (64) per axis so a tiny period cannot flood the batch, and a
  single copy with no view (picks, tests). The batch asks once whether any
  layer repeats, so a world without one draws exactly as before. Picking sees
  the original only: the copies are the same decoration again. The
  platformer's hills repeat every 30 units, so the range never ends.

### The batch draws what the camera sees, chunk by chunk for tilemaps

Stage 33, measured with `cargo run --release -p voltra-scene --example
batch_stress` (a 512 × 64 tilemap and 5 000 sprites, a ~21 × 12 view):

| | Before | Culled | + fast hashing |
| --- | --- | --- | --- |
| batch build | 8.06 ms, 37 768 quads | 2.92 ms, 209 quads | **1.82 ms**, 209 quads |
| of which composing transforms | — | 1.86 ms | 0.82 ms |

- **`Sheets::visible_in(min, max)`** carries the world rectangle the camera
  sees; the draw path sets it from the camera it draws with
  (`position ± half_extents`). `None`, the default, culls nothing — picks,
  tests and the screen-space UI batch (whose quads are in pixels, not world
  units) are unaffected.
- **Per quad**, in `SpriteBatch::push_quad`, on the corners as drawn: one
  test covers sprites, tiles and glyphs, rotated, scaled, parented or
  parallax-displaced. Edges touching count as seen; a non-finite box is kept
  rather than guessed away.
- **Per chunk** for tilemaps: a 16 × 16 chunk whose transformed bounds are
  out of view is skipped without visiting its cells, which is what makes a
  level's size stop mattering.
- **`voltra_ecs::hash::FastMap`** — the FxHash mix from stage 28, moved down
  to the ECS so the scene can use it — now keys `WorldTransforms`. SipHash
  over every entity's matrix was most of what was left.
- **Next, if a profile asks:** the 5 000 sprites are still sorted and their
  quads computed before the cull; a spatial index for sprites. (The
  per-chunk tilemap mesh cache landed in 44.)

### Colour alpha reaches the blend, and particles are quads in the batch

Stage 34, in two parts.

**The vertex is RGBA.** It had been RGB since stage 7, and the shader took
alpha from the texture alone, so `Sprite.color[3]` and `Text.color[3]` were
documented as "not drawn". `Vertex::color` is now `[f32; 4]`
(`Float32x4`, stride 32 bytes) and the fragment is `texture * colour`.
Existing scenes all author alpha `1.0`, so nothing on disk changes look;
a translucent sprite now draws translucent. Pinned by
`voltra-render/tests/headless_alpha.rs` (half-alpha red over black blends
half-way).

**`ParticleEmitter`** — CPU, 2D, drawn by the sprite pipeline (Godot's
`CPUParticles2D` subset a 2D game reaches for first):

- Rate per second (the fractional share carried frame to frame), `burst(n)`,
  `max_particles` (capped at 10 000), lifetime / speed ranges, direction and
  spread in the emitter's space, acceleration, size and colour from start to
  end — fading to alpha 0 is how a particle disappears.
- `world_space` (default): particles keep the world position they were born
  at; otherwise they ride the emitter's matrix.
- `one_shot`: once not emitting, owing no burst and empty, the loop despawns
  the entity — Unity's "Stop Action: Destroy", so per-event emitters (the
  platformer's coin sparks) clean up after themselves.
- Live particles are `#[serde(skip)]` state: a scene says what an emitter is,
  not which sparks were in the air. The state field is public with an opaque
  type only so `ParticleEmitter { rate: 5.0, ..Default::default() }` works
  outside the crate.
- An in-house xorshift64* seeded per entity: deterministic, testable, no
  `rand`. A frame longer than 0.1 s is simulated as 0.1 s, so a stall does not
  fire a second's worth of particles in one place; non-finite numbers emit
  nothing.
- Advanced in `App::advance` after animation, gated on the simulation switch
  like it; sorted with sprites by the emitter's `sort_order` and culled per
  quad like everything else. Inspector section and *Scene ▸ Spawn particles*.

### Buttons take the pointer and the focus, and the tick reads the clicks

Stage 35 — the interaction 26b left out. Godot's `Button` and its default
geometric focus neighbours, on this engine's pieces.

- **`UiButton`** on a `UiRect` element: colours for normal, hovered (or
  focused), pressed and disabled, written into the entity's `Sprite` each
  frame. The label is a `Text` child; a hit on it reaches the nearest
  `UiButton` above it, as Godot's default `mouse_filter` passes a click on.
- **The logic is a pure function** in `voltra-scene` —
  `UiButtons::update(world, layout, Pointer, Navigation)` (since stage 80
  `UiControls`) — so every rule is
  tested without a window. A click is a press and a release over the same
  button; releasing elsewhere cancels it; a press focuses. Clicks last one
  frame. Disabled and despawned buttons drop out of hover, press and focus.
- **Navigation:** `ui_up/down/left/right` move the focus to the nearest
  usable button ahead (distance plus twice the sideways drift); with no focus
  yet, the top-left-most. `ui_accept` presses the focused one. In
  `default.input.ron`: arrows and D-pad, accept on Enter, keypad Enter and
  pad South — not Space, which a game is likely to want for itself. A map
  without the actions navigates nothing, and does not warn about them.
- **The loop** updates the state after the frame's actions and before the
  tick, from the cursor in points (physical ÷ scale factor) and the left
  button's edges, laid out on the window in points — so `tick.ui.clicked(e)`
  is this frame's. `tick.ui.focus(e)` moves the focus. Only in a window with
  no editor around it: in the editor the HUD is being authored, and clicking
  it selects rather than presses.
- Inspector section, *Scene ▸ Spawn HUD button* (button and label in one undo
  entry), and a "Save" button on the platformer's HUD.

### A build can ship a game's own binary instead of the player

Stage 36 — the limit stage 29 named. `build.ron` gains
`game: Some((package, bin | example))`, and `scene` becomes optional: a game
with code builds its own start.

- **`cargo xtask dist`** builds that target in release (`--package p
  --example e`, or `--bin b`, or the package's own binary) and ships it as
  `<name>.exe`, from `target/release[/examples]`. A scene build still ships
  the player with `windowed`. Cargo names in the manifest are checked as
  `[A-Za-z0-9_-]` so a typo cannot become part of a command line; naming both
  `bin` and `example`, or neither scene nor game, is refused before the build.
- **The format stays compatible**: the manifest is read and written with RON's
  `IMPLICIT_SOME`, so `scene: "a.ron"` is still how a scene is written and an
  absent field is `None`.
- **The player refuses a game build** (no scene, a game binary) with a message
  saying to run the game instead.
- **`voltra_core::logging::init(to_file)`** is the player's terminal-or-
  `log.txt` logger, moved where every game can call it; the player and the
  platformer both use it. A game opts out of the console itself, with
  `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem =
  "windows")]` — a crate attribute, so it cannot come from the engine. The
  platformer does, and `platformer.build.ron` packages it: verified as a GUI
  (subsystem 2) executable, with the debug build still a console one.

### Animated tiles are keyed by the frame they replace

Stage 38. Tiled's and Godot's model: `Tilemap::animations` maps a frame to a
`TileAnimation { frames, frame_time }`, and every cell naming that frame
shows the animation instead. The cells are untouched, so a level already
painted animates by adding one entry; a layer saved before has no field and
reads as none (`skip_serializing_if` keeps it out of files that do not use it).

- **The clock rides in `Sheets::at_time`**, beside the camera and the view,
  so the mesh samples `Tilemap::shown_frame(frame, time)`. The draw path
  passes `App::elapsed` — seconds since the loop started, accumulated every
  frame whether or not the world simulates, so water moves in the editor
  too, as Godot's does. Wrapped at a day before the `f32` cast, which keeps
  millisecond precision however long the game runs.
- An animation with no frames, or a frame time that is not a positive number,
  draws the cell's own frame; a frame index past the atlas draws the
  placeholder, as any missing frame does.
- Per layer rather than in the atlas file: two layers sharing a sheet may
  animate it differently, and the atlas stays a pure slicing. The inspector
  edits base frame, frames (`4, 5, 6`) and seconds per frame.

### Auto-tiling is a 16-frame terrain per layer, re-picked around each edit

Stage 39. Godot's "Match Sides" terrain mode, the 16-tile set every 2D
tool knows: `Tilemap::terrains` is a list of `Terrain { name, frames: [u32;
16] }`, where `frames[mask]` is the frame for a cell whose same-terrain
neighbours are the sides in `mask` (N = 1, E = 2, S = 4, W = 8; cell `y`
grows up, as the layer does).

- **Membership is read from the frame**, not stored: a cell belongs to the
  first terrain whose frames include its tile's. The cell format does not
  change, a level painted by hand keeps working, and a terrain can be added to
  a layer already painted with its frames. The cost: a frame used both inside
  a terrain and on its own is read as the terrain's.
- **`Tilemap::paint_terrain(cell, Some(t) | None)`** paints or empties the
  cell and re-picks it and its four neighbours — every cell whose
  neighbourhood could have changed. Flips are kept; a terrain says which
  frame, not which way round. Two terrains never join. An index the layer
  does not have writes nothing.
- **Editor**: the inspector lists a layer's terrains — name, the sixteen
  frames in a 4 × 4 grid with each one's sides as a tooltip, *paint* to make
  it the brush — and the brush, rectangle and eraser then paint by terrain
  (`stroke::line_terrain`, `fill_terrain`, under the same fill limit). The
  choice is editor state, ignored on a layer that lacks that index.
- **The 47-tile blob** (stage 40): `Terrain::blob`, exactly 47 frames, makes
  a terrain read the diagonals too (NE = 16, SE = 32, SW = 64, NW = 128). A
  corner counts only when both sides that form it join — the standard
  reduction, which leaves 47 of the 256 masks; the frames are in ascending
  order of the reduced mask (`blob_masks`, computed once), and a test pins
  the count. Painting re-picks the 3 × 3 around a cell, since a diagonal
  neighbour's shape can change. An empty or wrong-length `blob` keeps the
  sixteen-frame sides mode, so stage 39 terrains read and behave as before.
  The inspector switches a terrain to 47 tiles and labels each frame with its
  sides and corners.

### Sorting layers are named bands above `sort_order`

Stage 41. Unity's sorting layers: an ordered list of names, back to front
(`Background`, `Default`, `Foreground` unless `App::with_sorting_layers`
says otherwise), and a `sorting_layer: String` on `Sprite`, `Tilemap`,
`Text` and `ParticleEmitter`. The draw key is `(layer rank, sort_order,
entity index)` — `draw_key` takes the rank now — so `sort_order` orders a
drawable only among its own layer, and moving a whole band forward is one
edit to the list rather than to every sprite in it.

- **Compatible on disk:** the field defaults to `Default` and is not written
  when it is, so every scene already saved reads and draws as before.
- **One answer for draw and pick:** the list rides in `Sheets::with_layers`
  (the draw path and `UiFrame::sheets` both pass the App's), and
  `Sheets::rank` falls back to the default list when none is given, as tests
  and headless callers do. An unknown name sorts as `Default` and is warned
  about once. `Default` is always in the list: everything naming no layer is
  on it.
- **Project-wide, in a file** (stage 42): `assets/default.layers.ron`
  (`(version: 1, layers: [...])`), by convention as the input map is, read at
  startup by the editor and the game alike so the inspector offers the layers
  the game sorts by; hot reloaded. `App::with_sorting_layers` in code wins
  over it, as `with_actions` does. A broken file keeps the layers in use.
  `*.layers.ron` is its own asset kind — out of the scene list, shown as
  "LAYERS" in the browser.
- **The watcher now passes project files** (`*.input.ron`, `*.layers.ron`).
  It filtered by asset kind, and since `is_scene` excludes input maps, a
  running game never saw `default.input.ron` change — 27a's hot reload worked
  at the `App` level but no event ever reached it. Pinned by
  `tests/watch.rs::project_files_arrive_too`.
- The screen-space UI keeps its own order (`sort_order`, depth, index): it is
  drawn in its own pass, above the world, whatever the world's layers.

### Post-processing is one graded pass between the world and the HUD

Stage 43. The smallest render graph that pays its way: the world is drawn
into an offscreen target, `PostPass` grades it into the frame, and what
must not be graded — the HUD, the editor overlay — is drawn after it.

- **`PostSettings`** (render) / **`PostProcess`** (scene, on `Camera`):
  exposure, contrast around linear middle grey, saturation, tint, vignette —
  Godot's `Environment` adjustments and Unity's grading volume, cut to what a
  2D game reaches for. Neutral is the identity, and not written to a scene.
  The active camera's grade is used, in the game and in the editor preview.
- **Colour space:** the pass samples the source's sRGB view, so it reads
  linear; it grades in linear and writes an sRGB target, which encodes. One
  conversion each way, never two — pinned by `headless_post.rs`, where a
  neutral grade of 0.5 linear reads back 188, not the darker double-encoded
  value.
- **Paths:** the window path draws into a renderer-owned target the size of
  the surface and grades into the swapchain; the editor draws into
  `scene_target`, grades into `post_target`, and the viewport panel shows
  `post_target` with the line overlay on it. A full-screen triangle from the
  vertex index, no vertex buffer; nonsense settings are clamped to safe ones.
- **Not yet:** effects that need more than one pass (bloom, blur) — the
  point at which a real graph of passes and transient targets earns itself.

### Tilemap chunks are cached between frames, and so is the batch

Stage 44, measured first with `batch_stress`: a 32 768-tile layer wholly in
view cost 3.6 ms a frame to rebuild, every tile's frame resolved through its
atlas again. Now 0.26 ms.

- **`TileMeshCache`** (scene, owned by the app's draw path), keyed by layer
  entity, parallax copy and chunk — Godot's per-quadrant canvas items at our
  storage and culling grain. Two levels: **local quads** (corners, UVs,
  texture) and **world vertices** for the last matrix and colour, so a layer
  that moves re-transforms but never re-resolves a frame.
- **Stale by stamp, not by flag**: the layer's cell `Revision`, tile size,
  sheet and atlas handles, `Atlases::generation()` (bumped by a reload under
  the same handle), the placeholder, the animation definitions, and — only
  for a chunk holding an animated tile — the frame each animation is on.
  Two consumers could not share a dirty flag; a stamp each remembers can.
- **Same batch, always**: a chunk wholly in view is copied; one straddling
  the view's edge is pushed quad by quad from its local quads, so each is
  culled as before. The tests build every frame both ways and compare — an
  edit, a move, a recolour, a resize, the clock, a new animation, an atlas
  reload, parallax copies. `SpriteBatch::from_world` stays uncached.
- **Entries a build did not draw are dropped** at its end: despawned
  layers, erased chunks, chunks scrolled away. Memory follows the screen.
- **The batch's buffers are kept too** (`SpriteBatch::rebuild`): with the
  cache in place, allocating a fresh megabyte-scale batch each frame was
  most of what was left — 2.1 ms of the 2.4.

### GPU particles: the CPU counts births, the GPU does the particles

Stage 45. `ParticleEmitter::simulation` is `Cpu` (default, the reference)
or `Gpu` — Godot's `CPUParticles2D` / `GPUParticles2D` pair as one switch on
one component, with the same parameters and look.

- **Split**: the CPU keeps rate, bursts and "births wait for room" — one
  piece of code for both — and a `GpuPlan` per emitter: a ring cursor and a
  ledger of births not yet known dead (by the *longest* lifetime plus a
  0.1 s margin, since the GPU ages newborns a frame late), so a live
  particle's slot is never handed out again. The GPU (`particles_sim.wgsl`,
  one invocation per slot) picks each newborn's angle, speed and lifetime
  from a PCG hash of slot and step, and integrates the rest — velocity then
  position, the CPU's order. Steps between two drawn frames merge into one
  dispatch, because a uniform written through the queue lands once per
  submission.
- **Drawing**: instanced, six vertices per slot, no vertex buffer — the
  vertex shader reads the ring as a read-only storage buffer; a dead slot
  collapses to a point. The batch records a `GpuEmitterDraw` where the
  emitter's sort key falls and **seals** the range before it, so
  `SceneDraw` (`Mesh` ranges and `Particles`) keeps painter's order across
  two pipelines. The HUD path is untouched (`MeshDraw`).
- **Capacity** is `max_particles`, up to `MAX_GPU_PARTICLES` (2²⁰) against
  the CPU's 10 000. `alive()` is an upper bound on the GPU.
- **Fallback**: `ParticlePipelines::supported` needs `COMPUTE_SHADERS` and
  `VERTEX_STORAGE`; without them (WebGL2, some GL) the renderer has no
  particle pipelines and `advance(.., gpu: false)` runs GPU emitters on the
  CPU. A switch either way lets the other side's particles die out.
- **Culled** (stage 50): each ledger entry also keeps where its births
  were and their *reach* — fastest speed × longest life + ½|a|·life² + half
  the widest size, scaled by the emitter for world space. The union of
  those boxes (carried by today's matrix for local space) is
  `gpu_bounds`; an emitter whose box misses the view, or whose ledger is
  empty — nothing alive — gets no draw. World-space sparks left behind a
  moving emitter keep it drawn while they could be on screen.
- **Not done**: sorting by age, collisions, sub-emitters, and repeating
  parallax copies of a GPU emitter (drawn once).
- **Verified** in `headless_particles.rs` on hardware: births and wrap,
  integration, the dead staying dead, and a particle drawn between two
  sprites in painter's order.

### A job pool of our own, and the batch's first use of it

Stage 46. `voltra-jobs`: a fixed set of workers made with the window, and
`JobPool::scope` — `std::thread::scope`'s borrowing rule (a job may borrow
anything that outlives the call) without a thread per job. The shape of
Bevy's `ComputeTaskPool::scope` and Godot's `WorkerThreadPool` groups.

- **Safety argument**, in `scope.rs`: a job's lifetime is erased to sit in
  the queue (`Job::erase`, the crate's one `unsafe` transmute), and `scope`
  does not return — normally or unwinding, via a drop guard — until every
  job it spawned has run. A job cannot borrow a scope-local: a
  `compile_fail` doctest pins it.
- **The caller works while it waits**, so `n` workers run `n + 1` jobs and
  a job that opens its own scope cannot deadlock the pool. A panicking job
  is caught; the scope finishes the rest and resumes the first panic.
- **No work stealing, one queue.** Frame jobs are a few dozen chunks; a
  mutex-guarded deque is not where the time goes. Revisit with a profile.
- **The batch tried it and lost** (stage 48, measured, then removed): a
  pre-pass placing sprites on the pool was 5–20 % *slower* than one thread
  at 5 000, 50 000 and 200 000 sprites, with the pre-pass buffer reused.
  Placing a sprite is a few dozen nanoseconds; writing and re-reading its
  four vertices through a side buffer costs more than the work saved, and
  the ordered append is the bulk. The batch is single-threaded again. The
  pool waits for a consumer whose work per item is compute, not copying —
  asset decoding — and is wired in only once a measurement says it wins.
  `TextureSizes: Sync` stays; it costs nothing. (The physics step was
  measured too: 0.2–0.8 ms for 270–500 contacts, too little to split.)
- **Its consumer: decoding textures** (stage 49). `Textures::preload` reads
  and inflates PNGs across the pool (`decode_png` needs no device) and
  uploads them on the caller in the paths' order, so handles are the ones
  loading in a loop would give — pinned, with broken and duplicate paths.
  `resolve_world_textures` (Open, undo, start) preloads every texture the
  scene names first, on a pool made for the occasion and joined after, so
  no thread idles between scenes. `decode_stress`: 48 noisy 512² sheets,
  58 ms → 32 ms on 12 threads — the uploads, serial, are the rest.
- **What did win, both by profiling first:** the sort recomputed each
  key's sorting-layer rank (a name lookup) on every comparison — keyed
  once per entity, the 5 000-sprite level went 4.1 ms → 1.8 ms (stage 46);
  and `WorldTransforms` copied locals and parents into hash maps before
  composing — read straight from the sparse sets into a dense array
  indexed by `Entity::index` (generation-checked), composing went
  0.94 ms → 0.20 ms and the level 1.8 ms → **0.85 ms** (stage 48).

### The frame's post chain is a render graph; bloom is its first chain

Stage 47. `graph::RenderGraph` — Bevy's render graph and Unreal's RDG cut
to a 2D frame: passes run in the order added, each naming the slots it
reads and writes; a slot is **imported** (scene target, swapchain view) or
**created** for the frame.

- **Validated** before anything is recorded: reading a slot nothing
  imported or wrote is a `GraphError`, logged, and the frame is not
  post-processed — never a half-recorded frame.
- **Culled** backwards from the imported outputs, so an effect switched off
  costs nothing however it was wired. Bloom at intensity 0 makes no level.
- **Allocated** from a `TexturePool` kept on the renderer: a created slot's
  texture returns to the pool after its last reader, so a later slot of the
  same size and format reuses it in the same frame; textures unused for
  three frames are dropped (a resize leaves old sizes behind).
- **Not yet**: automatic ordering from dependencies (insertion order is the
  order), async compute, multiple queues. A 2D frame has neither the passes
  nor the hardware queues that make those pay.

**Bloom** (`bloom::BloomPass`, `bloom.wgsl`) is Jimenez's Call of Duty
scheme, as Bevy and Unity URP use: a prefilter with Karis's quadratic soft
threshold halves the scene, 13-tap downsamples halve it level by level
(down to 8 texels, at most seven levels, `Rgba16Float`), and 3×3 tent
upsamples walk back up, each added onto the level above by an additive
blend. The post pass adds the half-size glow × intensity to the scene in
linear light before exposure and the grade. With bloom off it binds a 1×1
black texture, so the shader has one path.

`PostSettings::bloom` / `PostProcess::bloom` (scene, on the camera):
intensity (0 = off, and not written), threshold, softness, radius.
Pinned by `headless_bloom.rs` (a white square glows past its edge, the
corners stay dark, nothing under the threshold glows, off makes no
texture) and the graph's own GPU tests.

### Particles hit the scene's colliders, swept, the same on CPU and GPU

Stage 51. `ParticleEmitter::collision: Option<ParticleCollision>` (bounce,
friction, lifetime loss, layer mask) — Unity's particle Collision module in
"World" mode. **What is hit** is the scene's own `Collider`s (boxes,
circles, and the boxes generated from solid tiles), placed by their
`Transform` as the physics places them, sensors left out, filtered by the
emitter's mask against each collider's `CollisionLayers::layers`.
Particles are on no layer; nothing collides with them. Godot's
`GPUParticles2D` collides with an SDF baked from occluders instead — a
second set of shapes to author, which would drift from the colliders.

- **Swept, not overlapped.** Each step's path, in the world, is tested
  against every shape (slab test in the box's own frame; ray–circle). The
  earliest hit puts the particle `SKIN` (1e-3) outside, reflects the
  velocity's normal part × bounce and cuts the tangential part by friction;
  the rest of the step's motion is dropped. A fast spark does not tunnel a
  thin wall. A particle that *starts* a step inside a shape dies — born in a
  wall, or caught by a moving one — rather than being pushed an unbounded
  distance out.
- **One algorithm, twice.** `particles::collide` (CPU) and `collide` in
  `particles_sim.wgsl` are line-for-line twins; the GPU tests
  (`headless_particle_collisions.rs`) use the CPU tests' numbers.
- **Local-space emitters** collide where they are in the world: the step
  carries `to_world` (and the uniform its inverse); a matrix scaled to
  nothing tests no shape.
- **Gathered once per advance**, only when some emitter collides, then
  picked per emitter by box: for the CPU, the box of this step's paths; for
  the GPU, `GpuPlan::bounds` — the GPU tests every particle against every
  shape it is handed, so it is handed only what it can reach. The ring's
  collider buffer grows by doubling and is kept when the list shrinks.
- **Culling stays conservative**: bounce and friction are clamped to
  `0..=1`, so a hit never speeds a particle up and its path is no longer
  than the free flight the reach bound measures.

### 2D lights are a light map in the render graph, multiplied into the scene

Stage 52. `PointLight2D` (colour, intensity, radius, falloff; scaled with
its entity) and `Camera::ambient: Option<[f32; 3]>`. Godot's
`CanvasModulate` + `PointLight2D` and Unity URP's global + point
`Light2D`. **Lit is opt-in per camera**: with no ambient the frame is
unlit and no lighting pass is added to the graph, so a scene that never
asked for lighting does not go dark when someone drops a light into it.

- `light::gather` (scene) reads the active camera's ambient and the lights
  whose radius reaches the view; core sets `Renderer::lighting` before each
  frame, editor and game alike (the editor previews it, as it previews the
  grade).
- `lights::LightPass` (render) adds two graph passes: a frame-sized
  `Rgba16Float` light map cleared to the ambient, each light an instanced
  quad through the world camera, added (`pow(1 − d/r, falloff)`); then
  `scene × light`, pixel for pixel by `textureLoad`, into a lit
  `Rgba16Float` frame. Bloom and the grade read the lit frame, so a light
  past one pushes what it lights into the bloom. The HUD is drawn after and
  is never lit.
- Instance data is a vertex buffer, not storage: lights work on adapters
  without vertex-stage storage (WebGL2), unlike GPU particles.
- Shadows landed in stage 53, spot lights in 56, normal maps in 57,
  light layers in 58 and culling by the cone in 60a, below.

Pinned by `headless_lights.rs` (ambient alone far away, lit past white at
the lamp, fading between, nothing outside the radius; unlit makes no
texture) and `light::tests` (unlit without ambient, view culling, parent
and scale).

### 2D shadows are a polar distance map per light, nearest edge by `Min`

Stage 53. `ShadowCaster2D` (scene) blocks every `PointLight2D` whose
`shadows: Option<LightShadows>` (strength, softness) is set — Unity's
`ShadowCaster2D` + per-light shadow strength, Godot's `LightOccluder2D` +
`shadow_enabled`. Off by default on both sides, and not written.

- **The shape is the collider by default** (`CasterShape::Collider`): the
  entity's `Collider`, placed by its own `Transform` as the physics places
  it (a circle cut into 32 sides), or on a solid tilemap layer its
  generated boxes. A wall shadows where it blocks and the two cannot drift
  — the reason stage 51 rejected a second shape set for particles. A
  `Polygon { points, closed }` in the entity's space, carried by its
  composed matrix like a sprite, is there for the shape that should shadow
  differently; it is edited in the viewport (stage 55, below), and the
  viewport draws the selected caster's outline.
- **Which edges cast** (`lights::occluder`): an open line, every edge,
  both sides; a closed outline, only the edges facing *away* from the light
  unless `self_shadow`, so the shadow starts at its far side and its lit
  face stays lit — Unity's "Self Shadows" and Godot's cull mode as one
  switch. Winding is read from the signed area, so a mirrored transform
  does not flip it; a flat or two-point outline casts as a line.
- **The map** (`lights::shadows`) is Godot's 2D shadow atlas: one
  `R16Float` texture, four rows per shadowed light (right, up, left, down),
  a texel a direction through a 90° perspective projection centred on the
  light, holding the nearest occluder's distance over the radius. Each
  casting edge is an instance drawn as a quad filling the light's row in
  each quarter; the clipper keeps it to the quarter (`|x| ≤ w`) and in
  front of the light (`z ≤ w`, a near plane at 1e-4 radii), perspective
  interpolation finds where each direction hits the edge, and a `Min` blend
  keeps the nearest — **no depth buffer**, which the engine does not have.
  Half floats, because `Min` on `R32Float` needs `FLOAT32_BLENDABLE`.
- **The lookup** is in the accumulate pass: the pixel's quarter and
  direction pick a texel in its light's rows, taps across `softness`
  radians either side — each tap turned by its angle and looked up in its
  own quarter, so a penumbra crosses the seam between two rows — average a
  soft edge (thirteen taps, softness at most 0.1 rad so they stay a degree
  apart), and the light is kept by `1 − strength × shadowed`. The bias is
  1/512 of the stored distance (two half-float steps at least) plus 1e-5
  radii: relative, as the format's rounding is. Near the radius that is
  1/2048 of it, so a very large light resolves thin walls coarsely.
- **Culled per light on the CPU**: an edge casts for a light only if it
  comes within its radius; a shadowed light nothing reaches gets no row
  and is drawn unshadowed. Rows round up to a power of two so lights coming
  and going do not reallocate the texture every frame; past the device's
  texture height (a 2048-tall map holds 512 lights) the rest are drawn
  unshadowed and it is logged when it starts. Width is
  `Camera::shadow_resolution` (1024 a quarter by default, not written when
  default), clamped to the device.
- **Casters are gathered only when** a light in view casts shadows, and
  not culled by the view: an occluder outside it can throw a shadow in.
  They reuse the light gather's `WorldTransforms`.

Rejected: **shadow volumes into a stencil** (extrude each edge away from
the light, then mask the light's quad) — needs a stencil attachment per
light and one light per pass; **an SDF** of the occluders, ray-marched per
pixel (Godot 4's `GPUParticles2D` collisions, some 2D GI) — a second
representation to keep current for every edit, and soft edges are its
point, not ours yet; **a fragment loop over every edge** — needs storage
buffers in the fragment stage, which the lights deliberately avoid (they
run on WebGL2).

- Shadows from sprite alpha landed in stage 59, below.
- **Not yet**: contact-hardening penumbrae (a
  softness that grows with distance from the occluder), merging a solid
  layer's boxes into one outline (each box casts on its own; its inner
  edges shadow the tiles behind the first row, as Godot's per-tile
  occluders do). Editing a polygon's points landed in stage 55, and
  per-layer caster masks in 61a, below.

Pinned by `headless_shadows.rs` (dark behind a box and lit beside it, the
shadow widening along the near corners' lines; the box lit unless it
shadows itself; two lamps keeping their own rows; a light without shadows
ignoring the box; a weak shadow taking part of the light; a soft edge
fading over pixels a hard one does not; every quarter and the seam between
two, hard and soft), `lights::occluder` and `lights::shadows` unit tests (which edges
cast, culling, rows, overflow) and `shadow_caster` / `light` scene tests.

`assets/scenes/lights.ron` (stage 54) shows all of it: a shadowed lamp and
a shadowed spot light (since stage 56), both with a height for the studded
plaque's normal map (stage 57), an ember that casts none, pillars shadowing by their colliders, a turned
crate that shadows itself and a wire that is an open polygon with no
collider. Generated through `save` like `demo.ron`, and drawn by
`tests/lights_scene.rs` — with and without its casters — so a scene that
loads but no longer casts is caught. `cargo test -p voltra-scene --test
lights_scene -- --ignored write_the_lights_preview` renders it to
`target/tmp/lights-preview.png` (Cargo's `CARGO_TARGET_TMPDIR`), for a
look without a window.

### A shadow shape is edited as a viewport mode, in the entity's space

Stage 55. Unity's "Edit Shape" on `ShadowCaster2D` and Godot's polygon
editor for `OccluderPolygon2D`, in the shape the tilemap paint mode already
has (`crate::shape_edit`): the inspector's **Edit shape** turns it on for
the selected caster's polygon, and while it is on the viewport's pointer
belongs to the handles — no gizmo, no click-to-select. A drag on a point
moves it; a press on an edge's midpoint inserts a point there, and a drag
goes on to move it; a right click on a point removes it, down to two for a
line and three for an outline. `Esc` (wherever the pointer is, unless a
text field has the keyboard), the button again, a transform or paint tool
from the keys or the toolbar, or a selection that is not that caster with
a polygon turns it off. An undo or redo ends a drag in progress, so the
pointer does not write over what it restored. Dropped assets and the
collider overlay keep working meanwhile.

- **Points stay in the entity's space**: the pointer goes in through the
  inverse of the composed matrix, so a scaled, turned or parented caster
  is edited where it is drawn and stored as authored. Only a matrix with
  no inverse (scaled to nothing) is refused; a tiny caster is fine.
- **Hit-tested in screen pixels** (`shape_edit::handle`), as the gizmo is,
  points before midpoints, so the targets keep their size at any zoom.
- **One drag, one undo entry**, claimed every frame it is live, as a gizmo
  drag and a paint stroke are.
- **Where a polygon starts**: `shadow_caster::starting_polygon` — the
  entity's collider outline as it casts now (a circle as 16 points), taken
  back through the inverse of the composed matrix so the shadow does not
  move (a circle stays round on a non-uniform scale, as the physics keeps
  it), else its sprite's quad. "Casts shadows" on an entity with a sprite and no collider
  adds a polygon caster from the quad; "Use own polygon" switches a
  collider-shaped caster to that start and opens the editor; "Use collider"
  goes back.
- **Not yet**: moving several points at once, snapping, numeric entry,
  editing a solid layer's caster (its boxes are generated).

Pinned by `shape_edit::handle` (hit tests, insert, remove floors) and
`shape_edit` (when the mode turns itself off) unit tests and
`shadow_caster::starting` scene tests. The pointer interaction itself is
egui glue with no harness here; not verified in a window.

### A spot light is a point light with a cone, aimed by its entity

Stage 56. `PointLight2D::cone: Option<LightCone>` (whole angle, edge
softness), Unity URP's spot `Light2D` — a point light with inner and outer
angles — rather than a second component: everything else (radius, falloff,
shadows, culling) is the point light's, and a cone of a full turn is a point
light. `None` is the default and not written.

- **Aimed by the entity**: the cone points along the entity's +x as its
  composed matrix carries it (`PointLight2D::placed`, shared by the gather
  and the editor overlay), so the rotate gizmo aims it, a parent carries
  it, and a mirroring scale flips it. No direction field to keep in step
  with the transform.
- **In the accumulate shader**: `within_cone` multiplies the point light's
  strength by `smoothstep(cos outer, cos inner, along)`; the inner
  half-angle is the outer × (1 − softness), kept strictly inside so the
  edges never meet. A point light is sent edges below any cosine, so one
  shader path serves both. Shadows need nothing: the shadow map is per
  direction already.
- A cone with no angle or no direction gives no light at all, as a light
  with no radius does; the editor outline uses the same
  `PointLight::gives_light`, so it never outlines a light that is not
  drawn.
- The editor draws the selected light's reach: a circle, or a spot light's
  arc and edges (`view::light`). The inspector's spot toggle keeps the
  tuned cone while off, as the shadows toggle does.
- A cone of a full turn or more is sent as a point light's edges; a narrow
  one keeps its inner edge at most 1, so its middle is fully lit.
- Culled by its cone since stage 60a, and given a cookie in 62, below.

Pinned by `headless_lights.rs` (lit ahead and inside the edge, ambient
behind and beside; a soft edge fading where a hard one does not), the
instance and cone unit tests, and the gather test (a quarter turn aims it
up). The sample scene's cool lamp is a spot light.

### Normal maps are a second pass over the batch, read by lights with a height

Stage 57. `Sprite::normal_map: Option<AssetPath>` (Unity's `_NormalMap`
secondary texture, Godot's `CanvasTexture` normal) and
`PointLight2D::height: Option<f32>` (Godot's `Light2D` height). Both off
by default and not written.

- **One store, a linear view.** A normal map is an ordinary texture in
  `Textures`, bound through its non-sRGB view: `Texture::raw_view` already
  existed for egui, and `Textures::raw_bind_group` keeps a second bind
  group beside every colour one (made, swapped and reloaded together).
  Sampling a normal map through the sRGB view would decode data as colour
  and bend every normal — the double-conversion trap in its other form.
- **A second pass, not a second attachment** (`normals::NormalPass`): the
  batch's mesh ranges are drawn again, in painter's order, into an
  `Rgba8Unorm` normal buffer (`n · 0.5 + 0.5`, alpha 1 where drawn).
  Ranges without a map bind a stand-in facing the screen, so they still
  cover what is under them. Coverage is an alpha test at 0.5, not a blend:
  a sprite more than half transparent leaves the normals under it, and a
  fade crosses over at once. The normal map's own alpha is ignored (height
  packed there does not flatten it). GPU particles are not drawn, so the
  pixels they cover keep the normals of what is under them. The scene pass is
  untouched, and the pass is recorded only when the frame's lighting has a
  light with a height **and** some range has a map.
- **The tangent frame comes from screen-space derivatives** of the world
  position and UV (`dpdx`/`dpdy`), so a turned, scaled or flipped sprite
  needs nothing in its vertices — `Vertex` stays as it is for every other
  pipeline. Red follows the image's +u, green its up (−v), OpenGL's
  convention. A mirrored sprite mirrors its normals.
- **The light reads it** in the accumulate pass (group 2): a light with a
  height multiplies by `max(N · L, 0)`, `L` towards a point `height` above
  it. **A pixel no normal map covers faces the screen**, `(0, 0, 1)` —
  Godot's and Unity's rule — so a mapped sprite's flat parts match a plain
  sprite beside it; a light with a height dims what is far from it, which
  is the price of the model and why the height is per light and opt-in. No
  height, no change from before.
- `SpriteRange::normal` is part of what splits a batch run. Tilemaps carry
  their layer's since stage 60b (below); text and particles carry none.
- **A normal map that will not load is drawn flat**, not as the checker's
  colours read as normals: `Textures::is_broken` marks a handle holding the
  checker (until a reload repairs it), the draw list leaves its normal map
  out, and `Sprite::set_normal_map` says so by the normal map's name. A
  height of zero or less is no height; the inspector says "ignored".
- The height is in the light's entity units, scaled with it like the
  radius.

Rejected: **a second colour attachment on the scene pass** (MRT) — every
frame would pay for it, and the GPU particle pipeline would need the
attachment too; **a tangent per vertex** — a wider `Vertex` for every
pipeline to serve one; **a separate linear texture store** — two caches
for one file, and a hot reload that must know which to refresh.

- **Tilemaps (stage 60b)**: `Tilemap::normal_map`, one per layer, laid out
  as its sheet is — every tile samples it at the UVs it samples the sheet,
  so frames, flips and animated tiles need nothing of their own. A tile
  drawing the placeholder (a frame the atlas lacks) is flat rather than
  shaded by the wrong texels. The quads go through `batch::Surface`
  (texture, normal map, light layers), which is what a quad joins a run by;
  the chunk cache keeps only geometry and UVs, and the surface is applied
  as the chunk is copied in, so giving a layer a normal map or changing
  its light layers rebuilds no chunk. Resolved, preloaded and hot-reloaded
  with the sheet; the inspector has the path field under the sheet's.
- **Not yet**: normal maps on text (a glyph atlas has no sensible one), a
  per-light normal strength, specular, blending normals by coverage,
  particles writing their own (flat) normals.

Pinned by `headless_normals.rs` (normals towards the lamp lit and away
dark; a mirrored quad mirrors them; flat faces the screen; no height
ignores them), `lights_scene.rs` (the sample scene's studded plaque: the
lit side of a stud brighter than its far side, both alike with the map
stripped — resolved through the real `Textures`, so the linear bind group
is on the path), and batch, sprite and light unit tests; a layer's normal
map by `tilemap::cache_tests` (on every range, cached, view-edge and
uncached, also given after its chunks were cached; the placeholder flat)
and the format test (saved by path, left out when absent).

### Light layers are bits, drawn into an inverted buffer the lights test

Stage 58. `Sprite::light_layers` / `Tilemap::light_layers` (default the
first bit) and `PointLight2D::layers` (default every bit): a light lights a
surface only if they share a bit — Godot's `light_mask` and item cull mask.
Bits rather than Unity's "target sorting layers": the sorting layers order
drawing, and coupling who is lit to draw order would make moving a sprite
in front of another change its light. Defaults are not written, and a scene
that never touches them lights exactly as before.

- **A third pass over the batch** (`light_layers::LayerPass`), shaped like
  the normals pass: the mesh ranges into an `R32Uint` buffer, a small
  uniform per distinct mask this frame, coverage alpha-tested at 0.5.
  GPU emitters too since stage 60c, below. Recorded only when a lit frame
  has a light that leaves a layer out.
- **Stored inverted** — the layers a texel is *not* on — so the zero a
  buffer is cleared to, and the zeroed 1×1 stand-in bound when the pass did
  not run, put the background and everything on every layer. No upload is
  needed for the stand-in.
- **The light tests it first**: `(light.layers & ~excluded) == 0` returns
  nothing before any falloff, shadow or normal work. The mask is an
  instance attribute (`Uint32`); the buffer shares group 2 with the
  normals (`lights::scene_input`, binding 1) since stage 62, which freed
  group 3 — the fourth and last WebGPU guarantees — for a light's cookie.
- `SpriteRange::light_layers` splits batch runs; a tilemap's tiles carry
  the layer's on every path (cached chunks, chunks on the view's edge, the
  uncached mesh).
- **Text and particle emitters (stage 60c)**: `Text::light_layers` and
  `ParticleEmitter::light_layers`, default the first bit and not written,
  as a sprite's. Glyphs and CPU particles are batch quads and carry it
  through `batch::Surface`. A GPU emitter carries it on its
  `GpuEmitterDraw` / `SceneDraw::Particles`, and the layer pass draws it
  through a second pipeline: `particles_draw.wgsl`'s own vertex stage with
  a layers fragment appended to the module (`particle_layers.wgsl`), so a
  particle lands on the same texels in both passes without a copy of the
  vertex code. Built only with the GPU particle pipelines, whose draw
  layout it binds as group 2 (the mask is group 3); without them every
  emitter is on the CPU and in the batch anyway. Coverage is the same
  alpha test, so the faint edge of a soft particle keeps the layers under
  it — the rule every surface follows. A screen-space HUD's text is never
  lit and has none.
- A surface on **no** layer (mask 0) is lit by nothing, even a light of
  every layer: the pass also runs when such a range is drawn.
- The buffers' 1×1 stand-ins are read with the pixel clamped to their size,
  so a frame that did not draw one reads defined texels on every backend.
- The inspector folds the 32 toggles away (`light_layers` widget) on
  sprites, tilemaps and lights.
- **Shadow casters (stage 61a)**: `ShadowCaster2D::layers` — Godot's
  occluder light mask — default every bit and not written, so an untouched
  scene shadows as before. `Occluder::layers` carries it; `Edges` keeps it
  per shape and `casting` skips a shape sharing no bit with the light
  before any geometry, so a light no caster of its layers reaches gets no
  shadow-map rows at all. A caster on no layer blocks nothing. Inspector:
  a folded "blocks light layers" toggle under the caster's shape.
- **The ambient per layer** is a global light (stage 61b), below.

Pinned by `headless_light_layers.rs` (a layer-2 lamp lights the layer-2
quad and the background, not the layer-1 quad; every layer lights both; no
layer lights nothing), `headless_shadows.rs` (a box on layer 2 shadows a
lamp sharing it and not one that does not), the occluder and shadow-plan
tests (no shared bit, no edge and no row), `headless_particles.rs` (a GPU emitter writes its
own layers over the sprite under it, the sprite's elsewhere), instance,
batch, text and particle unit tests.

### A sprite's silhouette is traced on the CPU into ordinary occluders

Stage 59. `CasterShape::Sprite` — Unity's "Use Renderer Silhouette": the
outline of the sprite's opaque texels casts. **Traced, not rendered**: the
outline becomes the same `Occluder` polygons a collider or a polygon gives,
so the shadow map, culling, self-shadowing and softness need nothing new.

- **`voltra_assets::Silhouettes`** decodes a texture once (from the asset
  root, beside `Textures`, which keeps no pixels on the CPU) and traces each
  rectangle of it once: the sides between texels at or past half alpha and
  the rest become directed edges with the opaque side on the right, linked
  into loops, always taking the rightmost turn where a corner has two ways
  out (islands touching diagonally stay two loops); only outer loops are
  kept (a hole's edges would shadow its own inside), then Douglas–Peucker
  at 0.75 texel turns a staircase into its slope. A shape smaller than that
  keeps its texel edges. Deterministic. The alpha a texel must reach is the
  caster's (`CasterShape::Sprite { alpha }`, half by default), and part of
  the cache key. A sheet's pixels stay decoded for the store's life, for
  the frames still to come.
- **Keyed by texture and the rectangle the sprite draws now**
  (`shadow_caster::silhouette_key`, one function for the tracing and the
  reading): an animated sprite's frames are traced as they come up and its
  shadow follows the frame. A frame that is not there (the sprite draws the
  placeholder) traces nothing. `prepare` traces what this frame's casters need
  before the lighting is gathered.
- **Carried onto the quad** by the affine map from texture UV to the quad's
  corners, read off the quad itself, so flips, regions, atlas frames and
  pixels-per-unit all land where the sprite is drawn.
- Hot reload forgets a path's pixels and outlines; they are traced again
  when next asked for. A texture that will not load is logged once and
  casts nothing.
- `light::gather` and `shadow_caster::occluders` take an optional
  `SpriteShapes` (sheets + silhouettes); without it a sprite caster casts
  nothing — which is what headless callers that never load textures get.
- Editor: "Use sprite silhouette" on a textured sprite's caster (its alpha
  threshold editable, a warning when there is no textured sprite), and
  back to collider or own polygon; the selected caster's outline in the
  viewport is its traced silhouette (`UiFrame::sprite_shapes`).
- **Holes are left out on purpose.** In 2D a hole is enclosed by the
  shape, so no ray from a light outside it reaches it without crossing
  the shape — tracing holes would change only a light placed *inside* a
  hole, and would put their edges into every other light's row for
  nothing. Revisit when a scene puts a lamp in a hole.
- **Not yet**: a simplification tolerance per caster, decoding sheets off
  the main thread.

Pinned by the tracer's unit tests (a square is four corners, a staircase
its slope, islands apart and holes out — also when they touch only at a
corner — a rectangle in its own units, the threshold deciding, nothing
traced from nothing), the store's (traced on demand, forgotten on
reload, a missing file traces nothing) and the scene's (the outline lands
on the drawn quad, mirrored when flipped; nothing without silhouettes, a
texture or the frame it asks for). The shadow from an occluder is pinned by `headless_shadows.rs`.

### A light is culled by what it reaches, a spot light by its sector

Stage 60a. Until it, a spot light was its disc everywhere outside the
fragment shader: gathered when the disc touched the view, drawn as the
disc's square, and given four shadow-map rows when any edge came within its
radius — even aimed away from all of it. `voltra_render::Reach` (from
`PointLight::reach`, `None` for a light that gives nothing) is the disc, or
the sector between the cone's outer edges, and every one of those three
asks it:

- **The view**: `light::gather` keeps a light whose `Reach::bounds` meets
  the view. A sector's box is its centre, its two edges' ends and the
  disc's extreme along each axis the cone faces — so a cone wider than a
  half turn still reaches round behind its centre.
- **The quad**: the light instance carries that box as fractions of its
  radius and `vs_light` draws over it instead of the square; the fragment
  shader is unchanged (`local` is still in radii from the centre).
- **The shadow map**: an edge is drawn into a light's rows only if it comes
  into the reach (`Reach::touches_segment`: an end inside, or crossing an
  edge or the arc); a light with no such edge gets no rows. The cone is
  opened by the shadow's softness first, because the soft edge's taps look
  that far past where the light shines — without it a shadow near the
  cone's edge would lose its penumbra on the outer side. A blocking edge
  must cross the segment from the light to the pixel, which lies inside the
  cone whatever its width, so nothing that can shadow a lit pixel is left
  out.
- The cone used is the one the shader lights to (the sanitised outer
  edge), opened by 0.001 rad so a direction on the edge rounds in.

Rejected: fewer rows for a narrow cone (it would need a per-light row count
and a shader that knows which quarters a light owns — worth it when a
profile shows the map's height mattering, not before); culling by a
bounding circle around the sector (a narrow cone's circle is barely smaller
than its disc).

Pinned by `reach::tests` (boxes of narrow, quarter, wide, turned and
full-turn cones; segments ahead, behind, beside, across an edge, across the
arc only, through the light; widening), the shadow plan's (aimed away from
the only wall, no row; the softness bringing in an edge just past the
cone), the gather's (a spot light whose disc reaches the view but whose cone
does not is left out) and `headless_lights.rs` (a three-quarter cone lights
both sides behind its centre and not below it).

### A global light adds to the ambient, for the pixels of its layers

Stage 61b. `GlobalLight2D { color, intensity, layers }` — Unity URP's Global
`Light2D` — on any entity, its place ignored. **An addition, not a
replacement**: the camera's `ambient` stays what every pixel of a lit frame
gets, and each global light adds its colour where what was drawn shares one
of its light layers. Unity's global light instead *is* the ambient of the
sorting layers it targets (one per layer, a layer without one is black);
ours keeps the camera as the one switch that makes a frame lit (stage 52)
and the one ambient a scene without layers needs, and layers lift from it.
A darker layer is had by lowering the camera's ambient and lifting the
rest.

- `voltra_render::GlobalLight` in `Lighting::globals`; `light::gather`
  collects them (all of them — they are not in any view) only into a lit
  frame. One that gives nothing (no intensity, no colour, no layer) is
  left out.
- **Drawn** by `lights_global.wgsl`: one full-screen triangle per light,
  instanced from a vertex buffer (WebGL2 again), added onto the light map
  right after its clear, through the accumulate pass's blend. It reads the
  same light-layer buffer the point lights do (group 0 here) and gives
  nothing where the pixel shares no bit — the background, drawn by nothing,
  is on every layer and gets every global light.
- A global light of every layer is more ambient and needs no layer buffer;
  one that leaves a layer out makes `Lighting::uses_layers` true, so the
  layer pass runs for it as for a picky point light.
- Editor: "Spawn global light" in the Scene menu; an inspector section
  (colour, intensity, the folded light-layer toggles) that says it needs a
  lit camera.

Rejected: **an ambient per layer on the camera** (a list of mask/colour
pairs) — a second place to look for lighting, and it would not merge into
prefabs or be spawned from a tick like a component does; **replacing the
ambient per layer** — a pixel on two layers would need a rule for which
wins, an addition has none.

Pinned by `headless_light_layers.rs` (a layer-2 global light lifts the
layer-2 quad and the background to ambient + glow and leaves the layer-1
quad at the ambient; one of every layer lifts both; `uses_layers` follows
point and global lights), `global_light::tests` (gathered when it gives
light; default layers not written) and `light::tests` (only into a lit
frame).

### A light's cookie is an image over its reach, bound as the fourth group

Stage 62. `PointLight2D::cookie: Option<AssetPath>` — Godot's
`PointLight2D` texture, Unity's light cookie: an image laid over the
square the light's radius reaches, along the entity's axes (turned and
mirrored with it), each texel multiplying the light by its colour × alpha.
White leaves it, black takes it away, a colour tints it. Panes on a floor,
a flashlight's ring, a lamp behind leaves. It multiplies a spot light's
cone, shadows and normals alike.

- **The frame numbers its cookies.** The render crate knows no asset
  handles, so `voltra_render::Cookie { index, x, y }` points into
  `Lighting::cookies: Vec<wgpu::BindGroup>` (reference-counted clones of
  the store's colour bind groups). `light::gather` takes the `Textures` and
  wears cookies after culling (`light/cookies.rs`), each image once; with
  no store — headless callers — every light shines plain, as sprite
  silhouettes do without one.
- **Read as colour**, through the texture's sRGB view and sampler — the
  same bind group a sprite draws with, so a cookie and the sprite showing
  it agree, and hot reload swaps it under the same handle.
- **Group 3.** The accumulate pass was at WebGPU's four guaranteed bind
  groups, so the scene's normals and light layers became one group
  (`lights::scene_input`, a move-only refactor first) and the cookie took
  the fourth. Lights are drawn in runs that share a cookie (sorted by it —
  the light map is additive, so order does not matter), and the lights
  that wear none bind a zeroed stand-in the shader never samples.
- The instance carries the inverse of the cookie's axes (two rows) and a
  flag in the shadow attribute's spare lane; `worn` in `lights.wgsl` maps
  the offset into the image's `-1..1` and samples its top level
  (`textureSampleLevel`): the branches before it make derivatives
  undefined there, and a cookie's texels are large on screen.
- A cookie that would not load is not worn (the checker would be stamped
  on the floor); `set_cookie` says so by name. Axes on one line, or a
  cookie index the frame does not hold, shine plain.
- Editor: a "cookie" path field in the light inspector; resolved and
  preloaded with the scene's textures.

Rejected: **a texture array of every cookie** — one draw, but a fixed size
for every image and a re-upload when one changes; **the cookie in the
shadow map's group** — a bind group per light and shadow map pair, and a
layout the store's bind groups would not fit.

Pinned by `headless_lights.rs` (a half-black cookie lights only its white
half; turned with its light, the halves swap; a red one lets only red
through; an index the frame does not hold is plain), `light_cookies.rs`
(through the real store: two lights share one image's index and binding,
the axes turn with the entity, a missing image and no store are plain;
saved by path), and the cookie and instance unit tests.

### The selection is an ordered set, the last picked its primary

Stage 63a. `editor::Selection` replaces `Option<Entity>`: each selected
entity once, in the order picked, the last the **primary** — Unity's
`Selection.objects` and `activeObject`, Godot's last-selected node. The
primary is what the inspector edits and what every one-entity action acts
on (painting a layer, editing a caster's shape, making a prefab, the
gizmo until 63b); the set is what Delete removes, what the history watches
and what an undo or Stop puts back.

- **Picking**: a click selects one; Ctrl (Cmd) toggles one; Shift in the
  hierarchy takes the rows between the primary and the clicked one, in
  the order they are shown (applied after the tree's walk, which is the
  only time that order is known), and in the viewport adds one. A click on
  empty ground clears unless a key is held.
- **The box** (`editor::marquee`): a left drag that starts on nothing the
  gizmo takes draws a box, followed in world units, and on release takes
  in `voltra_scene::pick::entities_within` — an entity whose drawn shape
  (quad, text box, a layer's tiles) lies **wholly** inside, or, with
  nothing drawn (a light, a camera, an empty parent), whose origin does.
  Figma's and Inkscape's rule rather than Unity's "touches": a box across
  a level must not take the ground layer every time. Bottom of the draw
  order first, so the topmost is the primary. Shift or Ctrl at release
  adds.
- **Outlines** (`view::selection`): with more than one selected, each
  selected entity's drawn shape is outlined, the primary brighter; a lone
  selection is left to the gizmo, as before.
- **Undo** keeps the selection by identity on both sides of an edit, as a
  list (`Edit::selected_before/after: Vec<SceneId>`), and puts the same
  set and the same primary back. The play snapshot does the same.
- **Delete** (`editor::delete`, the inspector's button and the Delete
  key, not while a text field has the keyboard): every selected entity and
  everything under each, one entry; a child selected with its parent is
  recorded once.
- The inspector says "the last picked of N" when more than one is
  selected; editing several at once is not built.

Rejected: **a hashed set** — the primary would be arbitrary and an undo
could put back a different one; **"touches" for the box** — see above.

- **The gizmo carries the set (stage 63b).** It sits on the primary and
  the drag works out where the primary goes, as before; every other
  selected entity takes the same change about the primary's pivot
  (`gizmo::follow`) — Godot's rule, and Unity's "Pivot" handle with its
  active object. A move moves each by the same amount; a turn swings each
  round the primary and turns it by the same angle; a scale stretches each
  one's offset along the primary's axes and multiplies its scale by the
  same factors (a collapsed axis on the primary scales nothing rather
  than dividing by zero). World transforms, captured when the grab
  begins, turned back into local ones as the primary's are; an entity
  under another selected one is left to ride on its parent, which would
  otherwise move it twice. The history already watched the whole set, so
  the drag is one entry. Rejected: Unity's "Center" handle, a pivot at the
  selection's middle — a second mode to explain, and the primary is where
  the handles are drawn.
- **Duplicate, copy and paste (stage 63c, `editor::clipboard`).** A copy
  is the selection's tops (`Selection::tops`: no selected entity above
  them — a child copied with its parent is copied once, under it) and
  everything under each, recorded as a save records them, with each top's
  parent by id and its place in the world. A paste spawns them as a
  prefab's copies are spawned (`prefab::spawn_scene`: fresh ids, links
  inside remapped, links out cut), then puts each top back under its old
  parent if the scene still holds it — a duplicate lands on its original,
  as in Unity and Godot — or where it stood in the world if not, as a
  paste into another scene does. One undo entry each, selecting what it
  made. Ctrl+D, Ctrl+C, Ctrl+X (a copy, then a Delete) and Ctrl+V, and
  Duplicate / Copy / Delete in the Edit menu.
- **The clipboard is the system's, as text**: a header line
  (`voltra-entities 1`) then the copy as RON. egui hands over a paste only
  as `Event::Paste` with the clipboard's text, and only when it has some
  (egui-winit 0.35 `lib.rs`: Ctrl+C/X/V become `Copy`/`Cut`/`Paste`, never
  key events), so a clipboard of the editor's own would never hear Ctrl+V.
  As text it also pastes into a second editor window. Text without the
  header is ignored; with it, it is parsed as a scene file is, through the
  same registry (a component the build does not know survives as
  unknown), capped at 16 MiB.
- **Not yet**: editing a component across the selection; numbering the
  names of duplicates.

Pinned by `clipboard::tests` (a parent and child selected together copied
once with fresh ids, the copy's child under the copy's parent; a duplicated
child under its old parent where it was; a paste into an empty world where
the copy was taken; the text round trips, other text is not a clip, broken
text is an error), `gizmo::follow` tests (each tool's rule, a collapsed axis,
which followers ride on a selected parent), `selection::tests` (primary order, toggle, Shift ranges both
ways and without an order, the box replacing or extending, ids round
trip), `pick::region` tests (wholly inside, origins, HUD never, a layer
by its tiles, topmost last, outlines), `delete::tests` (one entry for the
set and its children, undone back selected in order; nothing selected
records nothing), and the undo and play tests, now over sets.

### An instance follows its file, but for what it overrides

Stage 64. Unity's prefab propagation, by component rather than by field.

**Which record is which.** Every entity a prefab spawn makes carries
`PrefabPart { record, overridden }`: the id of its record in the file (its own
`SceneId` is fresh per instance) and the components, by registered name, the
instance has made its own. Unity's corresponding source object, by id; Godot
matches nodes by path instead, which a rename breaks. The root's
`PrefabInstance` gains `removed`: records the instance deleted from itself.

**`prefab::sync(world, registry, prefab, root)`** puts every tracked component
a part does not override back to the file's value — set where it differs,
taken off where the file no longer has one — spawns the records the file
gained (under their parent record's part), and despawns the parts whose record
it lost, with what was added under them. Untouched: the instance root's
`Transform` (its placement), the structure (`Parent`, the link, the part), an
entity with no part (added to the instance), a component this build does not
know, and a record the instance deleted. Values are compared **as their types
write them** — `ComponentRegistry::normalize` reads a value as its type and
writes it back — because a file is pretty-printed and a world's record is not.

**When.** Once when the world first meets the prefab store (a game's start,
`App::resumed`), after every scene change or additive load, on Scene ▸ Open,
and for one prefab's instances when it hot reloads. A prefab that will not
read leaves its instances as they are rather than despawning every part.

**Overrides are recorded when made, as differences.** The editor calls
`prefab::refresh_overrides` on the selection each frame, just before the
history takes the frame's after side, so an edit and the override it makes are
one entry and undo together. The set becomes exactly what differs from the
record *now*: an edit typed back to the prefab's value stops being one.
Recorded rather than worked out at sync time, because by then the file has
changed and every component the edit did not touch would look overridden.

**Instances saved before parts are adopted** on their first sync: the root is
matched to the file's root of the same name (or its first), each child to a
child record of the same name under the matched parent, and everything that
differs from the file now becomes an override — so nothing they were given
changes, and every later edit to the file reaches them. A record nothing
matched is taken as deleted from the instance.

**Editor.**
- The inspector's Prefab section names the file, lists what the entity
  overrides with a Revert each, and on the root offers **Apply to prefab**
  and **Revert instance**.
- Apply (`prefab::apply`) writes the instance over its file: each entity under
  its record id (an added one becomes a record of its own), parents by
  record, no links or parts, and the file's own root transform kept (Unity
  leaves the root's position out). It is refused if the file would contain
  itself; otherwise every instance of the file is synced. The scene's side
  of it is one undo entry; the file stays written, as a created prefab does.
- Delete on a part records it in `removed`, in the same entry.
- Create prefab from selection links every entity as a part
  (`prefab::link`).

Rejected:
- **Field-level overrides.** Comparing fields needs a value tree that keeps
  enums, which `ron::Value` does not; components are what the inspector
  shows, so they are what an author overrides.
- **Storing a copy of the prefab in each instance as the base of a
  three-way merge.** It doubles every scene, and an edit made while the
  editor was closed needs no base when the overrides are already recorded.

**Not yet.**
- Nested instances are parts of the outer prefab; an edit to the inner file
  reaches them only once the outer one is saved again.
- Moving a part out of its instance is not recorded: a sync spawns its record
  back.
- Overrides on entities edited by something other than the selection — a
  tick, a script — are not recorded until they are selected.

Pinned by:
- `prefab::sync` tests: a fresh instance already in step through pretty text;
  an edit reaching it but not its placement; overrides and own components
  kept; parts lost and deleted; adoption without a change; a broken file left
  alone; `sync_source` reaching only its prefab.
- `prefab::overrides` tests: an edit is an override until taken back; one
  component and all reverted; a deleted part remembered and brought back by a
  whole revert; apply writing over the file with the others following and
  the root's place kept; a linked selection already in step.
- `delete::tests`: a deleted part noted in the same entry.
- `app::scenes::tests`: a scene saved with an older copy of a prefab arriving
  in step.

### Joints are solved as Box2D v3 solves them, inside every sub-step

Stage 65. `Joint2D` (scene) holds two bodies together, or one to the world:
**revolute** (the anchors together, the angle free but for an optional limit
and motor), **distance** (a rod at `length`, a spring about it, or with
`min < max` a range — a rope at `min` zero) and **weld** (anchors and angle
held). Box2D's three, with its math (`b2SolveRevoluteJoint`,
`b2SolveDistanceJoint`, `b2SolveWeldJoint`).

- **A joint lives on any entity and names its bodies** — `a` (none: the
  entity itself), `b` (none: the world, `anchor_b` then a world point) —
  Godot's `PinJoint2D` shape rather than Unity's component on the body: one
  entity holds one component of a kind, and a plank pinned at both ends needs
  two joints. The common case, a joint on the body held to the world, names
  neither. Bodies are what the solver gathers: a rigid body, or a collider
  without one (immovable). A joint naming a body that is not there joins
  nothing rather than holding to the world.
- **The angle, the limit and the motor are `a`'s against `b`'s**: the joint
  lives on the body that turns, held to what it turns on, so a wheel's motor
  speed is the wheel's.
- **Inside every sub-step**, before the contacts: warm started, solved with
  the soft bias (`SolverParams::joint_softness`: twice the capped contact
  frequency, damping ratio 2 — Box2D's `jointSoftness`), and relaxed after
  the positions are integrated; once more in the final relax. **Not** the
  contacts' warm-start-once structure: a soft constraint's
  `impulse_scale · accumulated` term is tuned against a *sub-step's* impulse,
  and accumulated over a whole step it over-damped a 2 Hz spring to a
  quarter of its stiffness (the spring test caught it). A spring sits within
  0.02 of `g/k` with this.
- Limits are speculative inside (exactly the speed that closes the gap) and
  soft past them; a motor's impulse is clamped to `max_torque · h`.
- **Bodies a joint joins do not collide** unless it says `collide_connected`
  — Box2D's default, so a hinge's halves can overlap where they meet. The
  pair is left out before the narrow phase.
- **Sleep**: a joint links its bodies' islands as a contact does (one waking
  wakes the other; they rest together), and a motor drives its island, which
  then never sleeps. `Sleep::wake_islands` and `settle` take body pairs now,
  contacts' and joints' alike.
- **Warm starting** keeps the impulses on the component (`#[serde(skip)]`),
  so they travel with the entity and a loaded joint starts from rest.
- **Ids**: a joint names bodies by `SceneId`, so a prefab spawn, a paste and
  a duplicate remap one between two copied bodies to the copies
  (`Joint2D::remap`), keeping one to a body outside the copy; a prefab sync
  compares a joint in the file's record ids and writes it back in the
  instance's, and Apply writes it in record ids.
- Editor: the inspector's Joint section (kind, ends, anchors, collision,
  limit, motor, spring); Physics ▸ **Join selection** (a hinge at the last
  picked body's centre, to the other selected body or the world, starting at
  the angle they stand at; on a new joint entity if the body has one) and
  **Spawn pendulum**; the physics overlay draws every joint.

Rejected: **joints on the body, Unity's shape** — one per entity here;
**XPBD joints** — the contacts are TGS Soft, and two solvers on one body
fight; **wheel and prismatic joints now** — the same machinery with an axis,
when a vehicle needs them.

- **Not yet**: wheel, prismatic, motor and mouse joints; a revolute joint's
  spring; breaking under force; joints on bodies without a collider turning
  (no inertia without a shape, as before).

Pinned by `tests/joints.rs` (a rod and a pin pendulum keeping their length
through a swing; a rod between two falling bodies; a rope slack then taut; a
spring stretched by `g/k` and settled; a weld holding a plank level; a limit
stopping a hinge and resting on it; a motor at its speed on its axle; joined
bodies passing through each other unless asked; a hanging pendulum sleeping
while a motor's wheel does not), `prefab::sync` (a joint inside a prefab
joining the instance's own bodies, and following an edit) and the scene
component's format test.

### Property animation is clips of eased keys, bound by name from a player

Stage 66. An `AnimationClip` (`voltra_scene::keyframes`, a `*.anim.ron`
file) is a `length`, `looping` and tracks; a `Track` is one `Property` of
one entity — `target`, the path of `Name`s from the player down (`""` is the
player, `"arm/hand"` a grandchild) — and `Key { time, value, ease }`s. An
`AnimationPlayer` (`clip`, `playing`, `speed`; the clock never saved) plays
one clip onto its entity and the ones it names. Unity's `AnimationClip` and
Godot's `Animation` + `AnimationPlayer`, cut to what a 2D game keys.

- **Bound by name, not by `SceneId`**: a clip is shared by every entity that
  plays it, and only names are the same in every copy of a prefab — Unity's
  and Godot's reason. Two children of one name: the first. Resolved once per
  distinct path per sample.
- **A closed `Property` list** (position, rotation, scale; sprite, text and
  tilemap colour; sprite frame; point and global light colour and intensity;
  light radius), each with a `ValueKind` and its read/write arms — not a
  reflected `"Sprite.color"` string: there is no reflection, and a typed list
  is checked at load instead of every sample. Rotation is not wrapped (keys
  at `0` and `2π` are a turn).
- **An ease per key, for the segment that leaves it** (`Step`, `Linear`,
  `EaseIn`, `EaseOut`, `EaseInOut`) — Godot's per-key transition, every tween
  library's vocabulary; no Bézier tangents until there is a curve editor.
  None overshoots, so a sample stays between its keys. Integers (a frame)
  count through the ones between, truncated towards the earlier key: `0` to
  `7` over a second is a flipbook.
- **Loaded leniently**: keys sorted (stably — two at one time cut), and a
  key that is not finite or not the property's kind dropped with a warning;
  `length` zero means the last key's time. A file that will not read is an
  empty clip, and a player of an empty clip neither runs nor ends, so the
  fixed file plays from the start.
- **Applied only while playing**: a paused or ended player writes nothing,
  so other code may move the entity. A clip that does not loop applies its
  end and stops. A missing target or component costs its own track and
  warns once per clip.
- **Per frame, gated on simulation**, after the `SpriteAnimation`s (a clip
  keying a frame wins). A dynamic body's transform is the step's: animate a
  kinematic body, or a child. The store (`AnimationClips`, the shape of the
  others) is lazy in `App`, hot reloaded before prefabs; the browser lists
  `*.anim.ron` as `Animation`, dragged onto the inspector's clip field, never
  into the scene.

- **Editor (66b): a Timeline panel** over the asset browser, for a primary
  selection whose player names a clip: one row per track (its target and
  property, **key** — the value it has now, at the playhead — and **del**),
  keys as diamonds on a strip of time (click selects and moves the
  playhead, drag retimes — sorted on release, since sorting under the
  pointer would change the id egui tracks the drag by — right click
  deletes), the selected key's time, value and ease, and an "add track" row
  offering the named entities under the player (`target::paths`) and the
  properties each has that the clip does not yet animate. Times snap to
  hundredths.
- **The clip is an asset, not the scene**: its edits are held in the store
  (`AnimationClips::get_mut`, played at once by every player of it) and
  marked unsaved until **Save** (`AnimationClips::save`, pretty RON through a
  temporary file) or **Revert** (a reload); they are not in the undo
  history.
- **Preview poses between frames.** The scene is drawn before the editor's
  UI, so `Timeline::pose` writes the clip at the playhead as the editor's
  last act in a frame (after the history's after side) and `Timeline::lift`
  puts the authored values back as its first act in the next. The viewport
  shows the pose; panels, the history and a save see only what was
  authored, so scrubbing is never an edit — Unity's preview mode keeps
  authored values aside for the same reason; Godot, which writes the pose
  into the scene, needs a RESET track. The cost: the gizmo and the
  inspector show the authored values while the preview is on.
- **Not yet**: record mode (an edit in the viewport keying itself), curve
  editing, copying keys, a clip's undo, playing the preview in time.

Rejected: **a state machine or blending now** — Unity's `Animator`, Godot's
`AnimationTree` — a subsystem of its own, when a game needs one; **curves
on arbitrary fields by reflection**; **ids as targets**.

Pinned by `keyframes/tests.rs` (a clip moving its player and a named child
with an ease, looping; a paused player leaving its entity alone; a missing
child costing one track; bad keys dropped and the rest played; a broken file
repaired by a reload; a save read back; a sprite frame flipbook), the unit
tests of eases, values, properties, tracks, target paths and the player's
clock, `app::animation` (playing only while simulating, taking a reload),
and the editor's `timeline` (a pose lifted back to the authored values), its
ruler's scale and its key re-sort.

### Characters are cast and slid, not simulated

Stage 67. `query::cast` (physics) is an exact shape cast — a box or a circle
moved by a vector, the first collider met, its fraction, distance, normal
and, for a shape that starts inside, the overlap's depth. On top of it,
`character::move_and_slide(world, entity, velocity, delta)` moves an entity
carrying `CharacterBody2D` (scene) and a collider, and answers the velocity
to keep and what it touched (`Slide`). Godot's `CharacterBody2D` and
`move_and_slide`; Unity's `CharacterController` and `Rigidbody2D.Slide`.

- **Casts are closed-form**, not stepped: box against box is the separating
  axis theorem over time (each face axis gives an overlap interval; first
  touch is the latest entry before the earliest exit — exact for convex
  polygons); circle against box is a ray against the box grown by the
  radius, with a corner's region taken against that corner's circle;
  circle against circle a ray against the summed radii; box against circle
  the circle cast backwards. No tunnelling, no approximation.
- **Touching is not hitting**, with tolerances: a shape touching a surface
  and moving along it or away does not hit it (a motion within 1e-5 of
  parallel is parallel; an overlap of 1e-5 left to leave is apart). Without
  them a slide along a slope read as pressing into it and stuck. For a
  touch the sweep's face is the normal, not the narrow phase's — whose
  reference face on a corner need not be the one met.
- **The collider is cast grown by the skin** (unit scale, world units), so
  the collider is kept exactly a skin from every surface along its normal at
  any angle, and one that ends nearer overlaps the grown shape and is pushed
  out. Backing off a skin along the motion instead lets a grazing walk creep
  into the gap under gravity tick by tick.
- **The loop**: cast, move to contact, classify the surface by its normal's
  angle from `up` (floor up to `floor_max_angle`, ceiling past its mirror,
  wall between), take the part going into it off the motion and off the
  velocity (a fall ends on landing, a jump on a ceiling), up to
  `max_slides`. On the ground a leaning wall is slid along as if upright, so
  a too-steep slope is not climbed by walking into it — Godot's
  `floor_block_on_wall`; in the air it is slid down. A crease (two surfaces
  facing across a corner) stops the move. Standing on a slope under gravity
  alone holds (`stop_on_slope`).
- **Seams and steps**: a box sliding over a floor of boxes meets the next
  one's corner level with its own bottom, and the exact cast rightly calls
  it a wall — the ghost collision every box-shaped character has. Answered
  as Unity's `stepOffset` and PhysX step climbing answer it: a wall met on
  the ground is tried as a step of up to `step_height` (lift as far as the
  room allows, move across, come down onto a floor; kept if it made
  progress). The same lets a ramp's foot and a low ledge be walked onto.
- **Snap**: a character that stood on a floor, did not land on one, and is
  not moving up reaches down by `snap` — down a slope, off a small step.
- The character is a collider without a rigid body: a static obstacle to
  the bodies the step moves. The inspector warns when it has one.

Rejected: **a dynamic body with friction and a ground ray** — what the
platformer example does, and it slides on slopes and pushes crates; fine for
that game, not for a controller; **a capsule shape first** — the usual cure
for seams, but a third collider shape touches every narrow-phase pair; the
step handles seams for boxes, and a capsule can come with its own stage;
**stepped or bisected casts** — approximate, and they tunnel.

- **Not yet**: moving platforms (the floor's motion carried), pushing
  dynamic bodies, constant speed on slopes, one-way platforms, a capsule
  (all but pushing and constant speed since landed: 68, 75a).

Pinned by `query/shape/tests.rs` (a box at a wall, onto a turned floor, a
slide along a floor not hitting it, the way out of an overlap, a circle at
a face and a rounded corner and missing past one, box/circle pairs, the
nearest through a filter, a fast cast not tunnelling) and
`tests/character.rs` (landing a skin above a floor, walking at full speed, a
wall taking the speed into it, a gentle slope climbed and a steep one a
wall, standing on a slope with and without `stop_on_slope`, a jump ending
on a ceiling, snapping down a slope and falling off a ledge, pushed out of
an overlap, a V corner, a floor of tiles without seams, a low ledge stepped
onto and a high one not). `cargo run -p voltra-core --example character`
walks it.

### Every shape is a hull grown by a radius; boxes and circles keep fast paths

Stage 75a. `Collider` gains `Capsule { half_height, radius }` — upright
along the entity's y, `half_height` the whole half height caps included,
so a box swapped for one keeps its feet where they were, and a circle once
it is no taller than wide — and `Polygon { points, radius }`, a
`ConvexPolygon` of at most eight points plus an optional rounding
(`#[serde(default)]`, so `Polygon(points:[…])` loads). Old scenes load
unchanged: the two variants are added, nothing is renamed.

- **One representation, Box2D v3's.** `Collider::world_shape` places any
  shape as a `WorldShape`: up to eight world points grown by a radius — a
  box its corners and no radius, a circle its centre and its radius, a
  capsule its two cap centres and theirs, a polygon its points (scaled,
  turned, re-wound under a mirroring scale) and its rounding. Bounds,
  outlines (`Collider::outline`, for the debug overlay, the editor and
  shadow casters), contacts and queries all start from it.
- **A polygon is convex by construction.** `ConvexPolygon::new` hulls
  whatever it is given (Andrew's monotone chain, near-duplicates welded,
  straight corners dropped, counter-clockwise) and refuses fewer than three
  corners or more than eight; it deserialises through the same check, so a
  file listing a concave outline loads as its hull and one listing no
  polygon is a component error naming why. Eight, a fixed array, keeps
  `Collider` `Copy` with no allocation per contact — Box2D's
  `B2_MAX_POLYGON_VERTICES` for the same reason.
  `ConvexPolygon::enclosing` cuts a larger hull to eight by replacing the
  edge whose neighbours meet with the least area added, so the result still
  covers every point: what a sprite's traced silhouette becomes (the
  inspector's **Fit to sprite**, through `shadow_caster::sprite_silhouette`).
- **Contacts by kind of pair, not by pair of variants** (`narrow/`):
  polygon–polygon is `b2CollidePolygons` — the separating axis over both
  shapes' normals with the same `FLIP_BIAS` hysteresis, clipping while the
  cores overlap, and once only the rounding overlaps the faces' closest
  points (corner against corner one point along their line); separations
  subtract both radii and each point sits midway between the surfaces.
  A capsule is a two-cornered polygon there, as Box2D makes it against a
  polygon. Anything against a circle goes by its nearest feature
  (`b2CollidePolygonAndCircle`, a segment's nearest point for a capsule);
  capsule–capsule is `b2CollideCapsules`, the two segments' closest points
  clipped to two points where they lie alongside. The general code is
  pinned to answer as `box_box` does over 400 random box pairs.
- **The fast paths stay, measured.** Per pair, `box_box` takes 114 ns and
  the polygon code on the same boxes 284; `box_circle` 28 against 122 —
  building a hull's normals is most of the difference. Boxes and circles
  are most of a level, so `box_box.rs`, `box_circle.rs` and `circle.rs`
  keep their pairs and everything else goes through the hulls.
- **Casts with a capsule or a polygon are conservative advancement over
  GJK** (`query/shape/advance.rs`, `geometry/distance.rs`, Box2D's
  `b2ShapeDistance`): measure the gap between the cores less the radii, move
  as far as it allows at the speed it closes, again. Under a translation the
  gap is convex in time, so a step never overshoots and a gap that is not
  closing never will — which gives "touching is not hitting" for free. Once
  the nearest points lie on an edge, the normal is that edge's: two shapes a
  hair apart have nearest points whose difference is mostly rounding. The
  closed forms for box and circle pairs stay. Rays into a rounded hull take
  the first entry into its convex pieces — the core, a rectangle out of each
  face, a circle on each corner — each exact.
- **Inertia from the shape, about the transform's origin**
  (`solver/inertia.rs`): `b2ComputeCapsuleMass`, and `b2ComputePolygonMass`
  moved to the origin by the parallel axis theorem (a rounded polygon is
  approximated by pushing its corners out, as there). A body's mass sits at
  its transform's origin for every shape — a centre of mass offset would
  move every anchor in the solver — so a dynamic polygon is drawn around its
  origin; a static one can sit anywhere.
- **A character grows round** (`character::inflated`): a circle, a capsule
  and a polygon by their radius plus the skin, which is exactly the
  Minkowski sum; a box keeps growing square, so a box character stands on a
  ledge's very edge as before. A capsule character crosses a floor of tiles
  without a step: its rounded bottom meets no corner as a wall.
- **Particles see boxes and circles**, CPU and GPU alike: a capsule is
  exactly its middle box and two caps, a polygon the turned box around its
  points — the one shape that is not a union of the two, and the corner too
  square is the visible cost.
- **The inspector** has a Collider section: the kind (switching keeps about
  the same size), the sizes, a polygon's points (an edit that would make it
  concave stops at the last convex outline, so a point never vanishes under
  the pointer), add and remove, and Fit to sprite. The viewport draws every
  shape through `debug::draw`.

Rejected: **`center_offset` on the capsule** — no other shape has one, and
the transform is where a shape sits; **a `Vec` of points** — an allocation
per collider clone and `Collider` no longer `Copy`; **folding boxes into
the polygon code** — measured above; **GJK/EPA for the manifold** — it gives
one point and a manifold still has to be clipped, as before; **exact swept
SAT for polygons** — it does not extend to rounded shapes, and conservative
advancement is the time of impact the next stage needs anyway.

Pinned by `voltra-scene`'s `collider/tests.rs`, `collider/polygon.rs`
(hulled in any order, collinear and inner points dropped, eight enforced,
`enclosing` covering a circle within 12 %, RON as a list, a file of no
polygon an error) and `world_shape.rs` (a capsule's outline); physics'
`narrow/polygon.rs` (two points on a polygon floor, the box agreement, a
rounded polygon's separation and midpoint, corner on corner, a capsule lying
and standing, far from the origin), `narrow/round.rs`, `narrow/capsule.rs`,
`geometry/` (segments, GJK), `solver/inertia.rs`, and `tests/shapes.rs`:
every query for every shape and every shape cast at every shape, a capsule
cast onto a turned polygon, a cast from inside and a slide along a touch, a
ray on a cap, a hexagon stack and two lying capsules settling asleep, a
hexagon rolling down a slope, a capsule character up a polygon slope and
across tiles, a polygon sensor and a one-way polygon.

### A fast body is stopped ahead of the solve, and swept after it

Stage 75b. A step collides once, at the poses it starts from, then moves
every body by its solved velocity, so a body crossing more than its own
size in a step can begin on one side of a wall and end on the other with no
contact ever found. `continuous` (physics) closes that, on by default
(`SolverParams::continuous`), with Box2D v3's split: a **fast** body — one
moving more than half its thinnest extent in a step (`b2ComputeShapeExtent`,
Box2D's safety factor) — is swept against **static** geometry, and a
`RigidBody::bullet` (`#[serde(default)]`) against kinematic and dynamic
bodies too, never against another bullet.

- **Two passes, as Box2D's speculative contacts and its time of impact.**
  Before the solve, each awake fast body's predicted move (velocity plus a
  step of gravity) is swept, and what it would meet is handed to the solver
  as a *speculative* contact: the pair's manifold measured as though both
  shapes were grown by half the gap (`narrow::speculative`, the hull code
  with the radii raised and the separations given back), so it has the
  right features and positive separations. The solver already let a
  positive separation close exactly and no further — a rigid stop, in the
  step it happens, with restitution and friction as any contact has. After
  the solve, each fast body's actual move is swept again, and one that
  still meets something — knocked off its predicted path, or turning — is
  moved back to the time of impact a hair short of it, velocity kept; the
  next step's speculative contact stops it.
- **Why not stop at the time of impact and leave the next step's overlap to
  it.** Tried first: an overlap is a soft contact, and at 300 units a second
  the soft push lets the body sink half a tenth-thick floor in a sub-step —
  past the middle, the floor's far face is nearer and the body goes through.
  The speculative contact is rigid; an overlap never forms.
- **The time of impact is conservative advancement with the turn bounded**
  (`continuous/toi.rs`, Mirtich's method, which Box2D's started from):
  advance by the gap over the move's length plus its turn times the shape's
  reach, so no point of the shape covers the gap in that time and the walk
  cannot pass through. A shape overlapping at the start is left to the
  contacts; one within reach is met only if it is moving in. A bullet's
  sweep against a moving body uses the move relative to it before the solve,
  and its end pose after.
- **Filters as contacts have them**: collision layers, sensors (never
  targets), the pairs a joint keeps apart, and one-way colliders — met only
  from the solid side and never while letting the pair through.
- **Speculative contacts are solved, not reported**: a contact about to
  happen is not one yet, so they reach neither `contacts()` nor events.
- **The cost** is a length against an extent for every awake body, and the
  sweeps for the fast few. `examples/stress.rs` runs every scene with it on
  and off: the level and the pyramid are the same within this machine's
  noise, and a hundred balls fired at a thin wall, every one fast, cost
  about 0.05 ms a step.

Rejected: **speculative margins on every contact** — Box2D v3's first line of
defence, but here the narrow phase reports overlaps only, and "touching is
not a contact" is what events, sensors and one-way passes rest on; the
speculative contact is confined to the fast bodies that need it; **bullets
by default** — every fast body against every moving one is the n² the split
exists to avoid; **sub-stepping the whole world at the time of impact** —
Box2D v2's, and the reason v3 dropped it: one fast body re-solves
everything; **a step-length cap on velocity** — hides the bug rather than
solving it.

Pinned by `continuous/toi.rs` (a box at a thin wall where its face reaches
it, a spinning bar stopped just short by its end, away and short of it
nothing, an overlap left alone) and `tests/continuous.rs`: a box, a circle,
a capsule and a polygon at 200 units a second never past a wall a tenth
thick; with it off the circle tunnels; a fast bouncing ball comes back; a
300-unit fall lands on a thin floor; a bar spinning at 60 rad/s neither
passes nor sinks into a wall; a bullet stopped by a thin dynamic plank
(pushing it) and a plain body passing it; a one-way platform catching a
fast fall and letting a fast rise through; a wall on a layer the body does
not see passed.

### One-way colliders remember what they let through; floors carry by pose

Stage 68. `OneWay { direction, arc }` (scene) makes a collider solid only to
what meets it from `direction` — in its own frame, so it turns with it —
within `arc`. Unity's `PlatformEffector2D`, Godot's one-way collision.

- **The step keeps a memory** (`OneWayPasses`, physics, beside the sleep
  state in `PhysicsWorld`): an overlap whose normal does not face the solid
  side is a pass, not a contact, and the pair stays a pass every step until
  it stops overlapping. Without it a body rising through a platform is
  caught the step its overlap's normal first turns up — halfway — and popped
  onto the top; a stateless depth threshold instead would be a number tied
  to speed and step rate. Unity's effector and Box2D's one-way sample keep
  the same state. `step` (stateless) starts it empty each call.
- **Casts** meet a one-way collider only when the face met admits the
  direction and the shape is not already partly through it, so a character
  jumps up through, walks through from the side, and lands on top. Rays,
  points and box overlaps still find it: they ask what is there.
- Dropping through is the game's: clear the platform's layer from the
  character's mask for a moment.

**Moving floors carry characters by their pose, not their velocity**:
`CharacterContact` keeps the floor's position and turn when it was stood
on, and the next `move_and_slide` first moves the character by however far
that floor has moved and turned since (`character/carry.rs`), outright — a
wall it is carried into is pushed out of by the move that follows. A
platform moved by a kinematic body, an animation clip or the game's code
carries the same; Godot's velocity-based carry misses the last two.
`platform_velocity` is what a game adds to a jump off one.

Rejected: **one-way as a collision-layer trick** — layers are symmetric;
**a carry by the floor's `RigidBody` velocity** — see above.

- **Not yet**: carrying the character's own rotation with a turning floor;
  dynamic bodies riding animated (non-kinematic) platforms by friction.

Pinned by `tests/one_way.rs` (a body landing on a platform, one thrown up
from below passing through and landing on top, a solid one still stopping
it, a character jumping up through and standing, a character carried by a
platform moving, rising and turning, and nothing once it is gone), the
cast's one-way test, and the component's format test. The `character`
example has a one-way ledge and a lift moved by `animations/lift.anim.ron`.

### A camera rig is two components, and references go through the registry

Stage 69. `CameraFollow2D` keeps an entity — usually a camera — on a
`target` (`SceneId`): `offset`, exponential `smoothing` (a rate, so the
frame rate does not change where it lands; zero snaps), a `dead_zone` the
target moves in freely, `look_ahead` seconds of the target's velocity, and
`bounds` its view (a `Camera`'s size and the frame's aspect) stays inside —
centred when the view is wider than them. `CameraShake` is Squirrel
Eiserloh's trauma model: `add_trauma`, linear `decay`, offset = trauma² ×
smooth noise (three incommensurate sines) × `max_offset`. Godot's `Camera2D`
smoothing, drag margins and limits; Cinemachine's dead zone and lookahead.

- **Components, not `Camera` fields**: a still camera is the common
  authored case, and any entity may follow (a light trailing the player).
- **Per frame, gated on the simulation** (`camera_rig::update`, after the
  tick and animations, before the audio whose listener rides the camera).
- **A shake never drifts the camera**: last frame's offset is taken off
  before following and the new one added after (`ShakeState::applied`), so
  following works from the rest position. The first frame — and after
  `snap()` — lands on the target instead of travelling.
- A parented follower is placed in world space through its parent's matrix.
  A missing target holds it still and warns once.
- Runtime memory lives in a `pub state` field of a type with private fields
  (`FollowState`, `ShakeState`), as `Joint2D::impulses`: private fields on
  the component itself would forbid `..Default::default()` in a game.

**References between entities are registered, not special-cased.** A
component naming entities by `SceneId` implements `SceneRefs` and registers
with `ComponentRegistry::register_with_refs`; a prefab spawn, paste,
duplicate, prefab sync and Apply rewrite its ids through
`registry.remap_refs` / `rewrite_refs`. Before this, each of those named
`Joint2D`; the camera's target was the second caller, and a third
component would have been a fourth copy everywhere.

Rejected: **follow and shake on `Camera`** (above); **random-jump shake** —
a flicker, not a motion; **a dead zone in screen fractions** — Godot's
margins; world units are independent of the window and testable headless.

- **Not yet**: rotation shake (the `Camera2D` has no rotation), several
  targets framed at once, an editor overlay of the dead zone and bounds.

Pinned by `camera_rig/tests.rs` (landing on the target and easing the same
at any frame rate, a dead zone held then left, bounds clamping a view and
centring one too big, a lead at half a second, a missing target holding,
a parented follower, a shake fading back to rest, a copied target remapped)
and `app::camera_rig` (following only while the game runs). The
`character` example follows with all of it and shakes on a hard landing.

### A state machine chooses the clip; the player plays it

Stage 70. A `StateMachine` (a `*.animator.ron` file) holds parameters
(`Float`, `Bool`, `Trigger`, each with a starting value), states (a name, an
`.anim.ron` clip, a speed) and transitions (`from` a state or any, `to`,
conditions `If`/`IfNot`/`Greater`/`Less`, an optional `exit_time` as a
fraction of the clip). An `Animator` component runs one: a game calls
`set_float`, `set_bool` and `trigger`, and `animator::evaluate` — once per
frame, before the players advance — moves it and has the entity's
`AnimationPlayer` play the new state's clip (adding a player if it has
none). Unity's `Animator` over an `AnimatorController`; Godot's
`AnimationTree` state machine.

- **Logic and playback stay apart**: the animator only picks; playing,
  looping and keys are the player's (stage 66). A flipbook is a clip with a
  `SpriteFrame` track, so states need no second kind of motion.
- **One transition per frame**, the first in file order whose conditions
  and exit time hold; an any-state transition never goes to the state it is
  in. A trigger stays set until a transition it lets through spends it, as
  Unity's do.
- **State by name**, not index, so a hot reload that reorders states keeps
  the entity where it was; a state that is gone restarts it at the start.
- **Loaded leniently**: a transition naming a missing state or parameter,
  or testing one as the wrong kind, is dropped with a warning; a duplicate
  state keeps the first; a file that will not read runs nothing.
- `AnimationClips` and `StateMachines` are one generic `RonStore<T:
  RonAsset>` (load, hot reload, atomic save); prefabs keep their own, being
  versioned and spawned.
- The browser lists `*.animator.ron` as `Animator`; the inspector edits an
  animator's machine and, while playing, its parameters.

Rejected: **blending and crossfades now** — two clips writing one property
need weights per track, a subsystem of its own; **sub-state machines and
layers**; **a graph editor now** — the file is short and hot reloads.

- **Not yet**: crossfades, blend trees, layers, a visual graph editor,
  exit time counting loops of a looping clip.

Pinned by `animator/tests.rs` (starting in the start state with a player
added, a float moving between states at a state's speed, a trigger spent
and an any-state transition not re-entering itself, exit times waiting for
a clip, a reload keeping a state or restarting without it, bad transitions
dropped, a broken file running nothing, the shipped machine reading whole).
The `character` example's idle, run, jump and fall are a machine.

### Navigation is a grid baked from colliders, in a crate of its own

Stage 71. `voltra-nav` (above physics and scene, below core, re-exported as
`voltra_core::nav`) bakes a `NavGrid` — cells over a rectangle, free or
blocked — from the scene's static colliders, and finds paths through it.
Godot's `AStarGrid2D`, Unity's 2D grid pathfinding packages.

- **A grid, not a navigation mesh**: a 2D level is mostly a grid already
  (tiles), and a grid bakes from colliders with nothing but overlap tests. A
  mesh is a stage of its own when open levels need one.
- **Baked from colliders, so from tilemaps too**: a solid layer's generated
  boxes are colliders like any other. An obstacle is solid, on a layer the
  settings look at, and not moving — no rigid body or a static one, not a
  `CharacterBody2D`, not a sensor, not a one-way platform.
- **Grown by the agent** (`clearance`, its half size): a cell is blocked
  when an agent-sized box anywhere in it would overlap an obstacle, so a path
  through free cells is one the agent fits along. Each obstacle is tested
  only against the cells under its bounds, exactly, by the step's narrow
  phase: a turned box blocks what it covers, and touching is not blocking.
- **A\*** over 8 neighbours (or 4), never cutting a blocked corner, with the
  octile heuristic, so the path is a shortest one. A start or goal a little
  inside a blocked cell uses the nearest free cell within four.
  `find_path_closest` goes as near as it can when the goal is walled off.
- **Straightened by line of sight**: a waypoint the one before can see past
  is dropped, by Amanatides–Woo traversal that checks both cells beside a
  corner it crosses and ends at the segment's end even when rounding puts
  an end on an edge in the neighbouring cell (the tests caught that).
- **The game owns the grid** and rebakes it when its walls change
  (`set_blocked` for a door): navigation is a question asked, not a cost
  every frame pays. A `Path` hands out waypoints as the agent reaches them.
- In its own crate: physics answers what collides; how to get somewhere is
  AI, and a crate with code in it.

Rejected: **a navigation mesh now** (above); **baking every frame**;
**hierarchical or jump-point search** — a grid this size is fast with plain
A*, and they can come when a level is not.

- **Not yet**: per-cell costs (mud, water), a debug overlay of the grid in
  the editor, agents avoiding each other, links for jumps or doors.

Pinned by `grid/tests.rs` (cells a wall covers and not those it touches,
clearance growing it, a path round a wall straightened with every leg clear,
an unreachable goal with no path and a closest one, diagonals never cutting
a corner, moving bodies, characters and one-way platforms left out, baking
round everything, a start pressed into a wall, waypoints handed out in turn)
and `tests/tilemap.rs` (a solid layer's tiles blocking a grid a path goes
round). The `topdown` example's chaser finds its way through a room.

### Components are reachable by name, with templates

MCP M1a. `ComponentRegistry::save_one`, `load_one`,
`remove_one` and `normalize` are public: any tool reads or writes any
component by its registered name, as RON, without a function per type.
`register` now answers a `Registration` that takes `.with_template()` —
the type's `Default`, written as saved, answered by `template(name)` — and
`.with_refs()` (what `register_with_refs` is now shorthand for).
`describe()` lists every type with whether it names entities and whether it
has a template.

- **A template is opt-in per registration**, so a game type with no
  `Default` still registers. Every built-in has one but the structure
  components (`Parent`, `PrefabInstance`, `PrefabPart`), which a tool must
  not make up: a parent link is a reparent, a part is a prefab's.
- `Collider` gained a `Default` — a box the size of a default sprite, what
  the editor gives a spawned body.
- Templates stand in for reflection: an AI client learns a component's
  fields from its default value, in the same text it will hand back.

Pinned by `format/registry/tests.rs` (every built-in but structure has a
template that loads into an entity and saves back identical; `describe`
reports references; unknown names and structure answer no template; a game
type without a default registers).

### A headless session is the player without a window

MCP M1b. `voltra_core::Headless` runs an `App` — built as a game builds it,
registry and ticks included — with no window and no device: `open(app,
scene)` loads a scene through the app's registry, `step(frames, delta)` runs
`App::advance` and the end of the frame's input as the window would,
`press`/`release(action)` drive an action through the first of its bindings
in the real input map, `entity_ron(id)` writes an entity as a scene file
does, and `set_component(id, name, ron)` sets any registered component from
text (its handles resolved after).

- **Not a second loop**: every frame is `App::advance` — ticks, fixed steps,
  animators, players, camera rigs, prefab sync, scene requests — so a scene
  behaves headless as it does in the player. The window's start does the
  same device-free setup (`sync_all_instances`, slicings, faces, sounds, the
  input map, sorting layers); textures are not uploaded, nothing draws, the
  audio is the silent one.
- **Input through bindings**, not by writing action states: a fed action
  reaches a game exactly as a player's key would, dead zones and all.
- `App::default` has no gravity (`App::new` sets it); `Headless` is built
  from an `App::new`, as a game is.

Pinned by `app/headless/tests.rs` (the sandbox stepping and its bodies
falling; a component set from text, read back, and refused for bad text,
an unknown name or entity; a fed action moving an entity in a game's fixed
tick and stopping when let go; a missing scene an error).

### A screenshot is the editor's viewport draw, offscreen and read back

MCP M1c. `Headless::screenshot(width, height)` draws the world through the
scene's active camera — lit, graded, the HUD over it — into an offscreen
target and reads it back as RGBA (`Screenshot`, with `pixel`, `save_png`,
`png_bytes`).

- **One draw**: the world-to-target draw the editor's viewport did inline is
  `app::draw::world::WorldDraw`, used by the viewport and the screenshot
  alike, so the two cannot drift. (`draw.rs`'s helpers moved to
  `draw/parts.rs` first, move-only.)
- **A device without a window**: `GpuContext::headless` / `Renderer::headless`
  — no surface; the configuration still carries the format and size a
  window's would (`Bgra8UnormSrgb`, the common sRGB one), so every pipeline
  is built the same. `acquire` answers `None` without a surface. `None` on a
  machine with no adapter: `ScreenshotError::NoDevice`, and a run without
  pictures carries on.
- **`voltra_render::read_rgba8`** is the readback, out of the tests:
  padded rows stepped over, BGRA swapped to RGBA, sRGB bytes as a screen
  shows them; a map that fails logs and answers black rather than panicking.
- The device and the texture store are made on the first shot and reused;
  the targets resize to each shot's size.

Pinned by `app/headless/tests.rs` (a red square in the middle of a 64×64
shot through a camera, the clear colour at the corner, PNG bytes, a second
shot at another size) and `voltra-render/tests/headless_renderer.rs` (a
headless renderer with no surface clearing a target that reads back red).

### Logs are kept in a ring a run can read back

MCP M1d. `logging::init` installs a `Tee`: `env_logger` as before — the
terminal or `log.txt` — with every line it lets through also copied into a
ring of the last 4096 (`logging::capture`). A `LogCapture` is a cursor:
`drain()` answers the lines since it last read, oldest first, and `missed()`
how many fell out of the ring before it got to them. `Headless::take_logs`
is one, made when the session starts: what the engine said while it ran,
for a tool to hand back.

- One ring for the process, every thread's lines: a run's log is the
  engine's, not one thread's. Captures are cursors into it, so any number
  read without taking from each other.
- What reaches the ring is exactly what the logger prints — the same
  filter — so a warn-once latch still warns once.
- A game that installs its own logger captures nothing; said in the docs.

Pinned by `logging/capture.rs` (a line read once, the next one read after
it, a flood past the ring counted as missed) and `app/headless/tests.rs` (a
missing camera target's warning handed back by a session, once).

### The MCP server works on files, through the engine's own formats

MCP M2a–b. `voltra-mcp` is a binary crate — `voltra-mcp
--project <dir> [--assets <dir>]` — serving MCP over stdio with `rmcp` 3.11
(the official Rust SDK; `#[tool_router(router = …, vis = …)]` per group of
tools, added together, and `#[tool_handler(router = self.tool_router)]`,
checked in the vendored source). The log goes to stderr: stdout is the
protocol's.

- **Tools are plain functions** in `tools::*` over a `Project`, answering
  JSON or a `ToolError`; `server::*` only wraps them. An error is a tool
  error the client reads (`is_error`), never a protocol error or a crash.
- **Paths are asset paths**, checked by `AssetPath` (no `..`, no absolute
  path) and then canonicalised against the asset root, so a link out of it
  is refused too.
- **Entities are named** by id (the UUID `scene_read` gives) or by a path of
  names from a root (`"Player/Arm"`); a path two roots answer is refused,
  naming both ids.
- **Edits go through the engine**: load the scene into a `World` through the
  registry, edit, save atomically — or, on any failure, write nothing. A
  component is set from RON through `ComponentRegistry::load_one`, so a bad
  value is refused with the format's own message (a missing field, a span).
  Structure (`Parent`, the prefab links) is refused by name: reparenting has
  its own tool.
- Tools now: `components_list` (names, references, templates), `assets_list`,
  `scenes_list`, `scene_read`, `scene_create`, `entity_spawn`,
  `entity_delete`, `entity_reparent`, `entity_rename`, `component_set`,
  `component_remove`, `prefab_instantiate`, `asset_read`, `asset_write`,
  `run_scene`, `project_build`, `project_test`, `project_dist`. Usage and the
  `.mcp.json` example: `docs/MCP.md`.
- **`run_scene`** (M2c) runs a `Headless` session off the async runtime
  (`spawn_blocking`: an `App` is not `Send`, so it is made and dropped on the
  blocking thread) — frames, actions pressed and let go on their frames,
  then the reported entities as RON or where each named one ended up, the
  log's last 200 lines, and a screenshot as an MCP image block. No adapter
  is said in words, not failed.
- **`asset_write`** writes a RON asset only once it reads as what its name
  says — a scene (and its version), a clip, a state machine, an input map,
  an atlas (sliced), sorting layers (as RON) — atomically, and not over an
  existing file without `overwrite`. Binary assets are not written.
- **Project tools** (M2d) run cargo with arguments built from checked
  pieces — package names, a test filter that is not an option, a manifest
  inside the project — never a client's command line.
- **Engine components only.** The server knows the engine's registry; a
  game's own components are reached through a running editor built with
  them (M3), not by building the server against the game.

Pinned by `voltra-mcp/tests/{discovery,scenes,run}.rs` — a real `rmcp` client
over an in-process pipe: the tool list, templates naming their fields,
assets and scenes listed, a scene read as entities, a path out of the
project refused; a scene made, filled from a template, renamed through a
name path, read back; five failing edits leaving the file byte-identical;
a subtree deleted and an entity freed to a root; a prefab placed and linked,
and refused into itself; a run whose bodies fall, with an image block; a run
reporting an entity in full and naming an unbound action; an asset written
only when it reads, never over a file or as a binary — and `project.rs` (a
link out of the asset root refused) and `tools/project.rs` (cargo arguments
from checked pieces).

### The editor opens any scene, and knows when it has unsaved changes

Stage 72. The editor works on one scene at a time — a `SceneDocument`: the
asset path it came from (`None` while untitled) and the history's position
when it was opened or saved. `scene_file::{new_scene, open, save, save_as}`
are the four operations, shared by the menu, the keys, the asset browser
(double-click) and the bridge; every one stops play first and replaces the
selection and history together.

- **Unsaved changes are the history's position, not a flag.** `History`
  numbers each entry as it is recorded (never reusing a number) and
  answers `position()` — the entry an undo would take back. The scene is
  dirty while that differs from the saved position. Godot's answer
  (`UndoRedo::get_version` against the saved version); Unity's
  `isDirty` flag would need every edit path to set it and could not tell
  that undoing back to the save makes the scene clean again. It holds
  because every scene edit already goes through the history.
- **Ask before letting a scene go.** New, Open (menu, browser, keys) and
  closing the window go through `scene_dialogs::request`: with unsaved
  changes it asks Save / Don't Save / Cancel; Save on an untitled scene
  names a file first and the waiting action runs after that save.
- **Closing is the UI's to agree to.** `App::with_close_guard` turns a
  close request into a question the UI answers (`UiFrame::close_requested`
  / `close` / `cancel_close`); without a UI a close is still immediate, so
  a guard can never trap a window. The title is requested the same way
  (`UiFrame::set_title`, applied after the layout and only when changed) —
  egui's viewport commands have the same shape, and the window stays
  `App`'s.
- **Pickers are the editor's own windows,** not native file dialogs: Open
  lists every scene under the asset root (`voltra_assets::browse::find`,
  also what `scenes_list` in `voltra-mcp` uses), Save As takes an asset
  path. A path outside the asset root cannot be typed at all, and no
  platform dialog crate (GTK on Linux) joins the build — the shape Godot
  chose for the same reason.
- **The bridge refuses to drop work.** `open` and `new` over unsaved
  changes are refused unless `discard_changes`: the person at the editor
  may be mid-edit. `save` takes an optional path (Save As); `status`
  reports the scene and `unsaved_changes`.

Pinned by `undo/history/tests.rs` (the position coming back with undo, a
new edit somewhere new), `document.rs`, `scene_file.rs` (scene paths),
`main.rs` (the command line), `app/window_requests.rs` (a guarded close
waiting for the UI, an unguarded one immediate, a guard with no UI not
trapping), `browse/find.rs`, `bridge/edits/tests.rs` (unsaved changes
kept unless dropped, clean again after undo) and `voltra-mcp/tests/editor.rs`.

### The profiler records scopes per thread, and summarises them as a tree

Stage 73. `voltra-profile` (standard library only, at the bottom of the
graph so any crate can time itself) records named scopes —
`let _scope = voltra_profile::scope("solve");`, timed until dropped — and
counters (`count("contacts", n)`), frame by frame, into a ring of the last
600 frames. `Summary::of` turns a run of frames into the frame's time and a
tree of scopes by path (`update/physics/step/solve`): calls per frame,
average and worst milliseconds, own time (outside nested scopes).

- **Unity's markers, Godot's monitors.** Scopes are Unity's
  `ProfilerMarker.Auto()` — a named marker, a guard — and counters are
  Godot's monitors (entities, contacts). Bevy's `tracing` spans were the
  other option; a tracing subscriber is a framework to configure and its
  spans cost more than a timestamp pair, for a profile that wants frame
  times and a tree.
- **Per thread, no lock.** Each thread records into its own recorder and
  closes its own frames (`end_frame`, called by `App::update` and by
  `Headless::step`). An `App` lives on one thread, so its numbers are its
  thread's, and two Apps — parallel tests, two headless runs on the MCP
  server's blocking pool — never mix. The cost is that a scope on a worker
  thread is read by nobody; none of the frame's work runs on one today.
- **Off until asked for.** A disabled scope is a thread-local flag read.
  The editor's Profiler window starts recording when opened (as Unity's
  does); `editor_profile` starts it, waits for the frames and stops it
  again if it found it off; `run_scene` with `profile` records only its
  own run, on a cleared recorder.
- **What is timed now:** the frame (`update`: input, tick, animation,
  particles, audio, physics with each `step` and its `collide`, `islands`,
  `solve`, `settle`, asset reload; `draw`: ui, present or render) on the
  CPU, where `present` includes waiting for the display — and each render
  pass on the GPU (below).
- **GPU time per pass** (stage 74). `voltra_render::GpuTimer` writes a
  timestamp into the encoder before and after each named pass
  (`CommandEncoder::write_timestamp`): every pass of the render graph by
  its name, plus `scene`, `normals`, `light layers`, `hud`, `overlay` and
  `egui`. When the encoder is finished the timestamps are resolved and
  copied into one of eight readback buffers, mapped after the submit and
  read with a non-blocking poll when they land — a frame or a few later,
  since the GPU runs behind; nothing ever waits for it. Timings arrive in
  the frame they land in (`voltra_profile::gpu`) and summarise per pass
  (average, worst, samples). Timing needs `TIMESTAMP_QUERY` and
  `TIMESTAMP_QUERY_INSIDE_ENCODERS`, asked for when the adapter has both
  and never required; without them the timer times nothing. Encoder-level
  timestamps rather than `timestamp_writes` on each pass descriptor: one
  hook around a graph pass covers whatever it records, however many passes
  that is, without threading a query set through every pass's code — the
  shape of Bevy's render-graph diagnostics. A pass that turns out to draw
  nothing (normals in an unlit frame) reads as near zero rather than
  missing.
- **The tree is ordered as a tree**: a scope right after its parent,
  siblings in the order they first ran — first appearance alone would put
  a child first seen in a later frame (the first physics step) after
  unrelated scopes.

Pinned by `voltra-profile`'s tests (nesting and depths, the history cap,
threads kept apart, the tree with own time and calls, a late child under
its parent), `app/headless/tests.rs` (a profiled session naming
`update/physics/step/solve` and counting entities and contacts),
`voltra-mcp/tests/run.rs` (a profiled run, and none unless asked) and
`voltra-mcp/tests/editor.rs` (recording until the frames are there, and
left off as found).

### The engine builds for the web; every asset read goes through one funnel

The first web stage. `cargo clippy -p voltra-player --target
wasm32-unknown-unknown` is clean and in CI; nothing native changed.

- **`voltra_assets::fs`** is where asset bytes come from:
  `read`/`read_to_string` answer from a pack [`mount`]ed in memory when one
  covers the path (a path inside a pack's root that the pack lacks is
  missing, not looked for on disk), and from the disk otherwise — natively;
  the web has none. Textures, atlases, fonts, clips
  (`voltra_audio::decode_bytes`), scenes, prefabs, RON stores, the input
  map, sorting layers and `build.ron` all read through it. A web build will
  fetch one pack before the game starts; a native test mounts one to test
  that path without a browser.
- **`MaybeSend` / `MaybeSync`** (`voltra-jobs`): `Send`/`Sync` natively,
  nothing on the web, where a job runs where it is spawned and GPU handles
  are not `Sync` — wgpu's `WasmNotSendSync`, for the same reason.
  `TextureSizes` is `MaybeSync`.
- **`web-time`** stands in for `std::time::Instant`, which panics on
  `wasm32-unknown-unknown`, in `voltra-core::time` and `voltra-profile`;
  it is a web-only dependency, so native builds keep `std`'s.
- **`egui-winit` without its defaults** at the workspace, turned back on
  natively by `voltra-core`: the web build leaves out the native clipboard
  (`arboard`), which does not build there.
- **`uuid`'s `js` feature** on the web: ids need randomness, and the
  browser's crypto is the source there.

Pinned by `fs.rs` (a mounted pack answering for its root, the disk for the
rest, gone when unmounted) and the `wasm32` CI job.

### A scene runs in a browser: one pack, an awaited renderer, an sRGB view

The second web stage. The player builds for `wasm32-unknown-unknown`,
fetches its game, and draws the sandbox on WebGPU and on WebGL2;
`crates/voltra-player/web/test/first-frame.mjs` (Playwright) checks both.

- **The game is one pack** (`voltra_assets::pack`, written by `cargo xtask
  pack`): the native build folder without the executable — the shipped
  `build.ron` and `assets/…` — as `"VPAK"`, a version, and length-prefixed
  paths and bytes. Godot's `.pck` and Unity's WebGL `.data` have the same
  shape: one fetch before the game starts, nothing fetched mid-frame, no
  compression (the server gzips). `decode` trusts nothing — every length is
  checked against what is left before allocating, paths must stay inside
  the pack, duplicates and trailing bytes are refused. The web player mounts
  it at `/game` (`voltra_assets::pack::PACK_ROOT`), where a native build's
  executable would sit, and `launch::resolve` reads the manifest from there
  exactly as natively — so the launch rules have one implementation.
- **The renderer is awaited** (`GpuContext::new_async`, `Renderer::new_async`;
  the blocking `new` is native only). `App::resumed` makes the window, then
  `platform::start_renderer` either builds it inline (native) or in a
  `spawn_local` future that leaves it in a slot and wakes the loop through an
  `EventLoopProxy<()>`; `user_event` finishes startup (`finish_resume`). The
  slot rather than the renderer in the event keeps the event type `()` on
  both targets, and `finish_resume` resizes to the canvas and asks for the
  first frame, since both may have happened while the device was on its way.
  The web loop is `spawn_app`ed with `ControlFlow::Wait` — the frame asked
  for at the end of each frame is paced by the browser.
- **WebGPU first, WebGL2 after** (`wgpu::util::new_instance_with_webgpu_detection`,
  `webgl` feature on wasm only). On an adapter below wgpu's default limits
  the device asks for the adapter's own (`required_limits`): native and
  WebGPU keep the defaults.
- **The frame is always drawn sRGB.** A WebGPU canvas offers only linear
  formats; the surface keeps one and frames are drawn through its sRGB view
  (`view_formats`), so `GpuContext::format()` — not `config().format` — is
  what pipelines are built for and `frame_view` what passes draw into.
  Colours match a native window byte for byte (the test looks for the
  sandbox's linear colours sRGB-encoded).
- **No `VIEW_FORMATS` on GL.** wgpu 30's GL backend never reinterprets sRGB
  as linear, so `Texture` uploads its pixels a second time as `Rgba8Unorm`
  for its raw view there (normal maps stay data), and a `RenderTarget`'s raw
  view falls back to the sRGB one (only the editor reads it).
  `texture::reinterprets_srgb` decides, from the backend.
- **What a frame makes, a frame destroys.** On the web a dropped
  `wgpu::Buffer` is freed only when JavaScript collects it. `Mesh` destroys
  its buffers on drop, per-frame pass buffers (lights, shadow edges) go
  through `FrameBuffers`, destroyed by `GpuContext::submit`, and the light
  layers' per-mask uniforms are cached in `LayerPass`.
- **Saves are `localStorage`** under the key the folder rules give the slot
  (`Platform::Web`: `/<game>/saves/<slot>.save.ron`); **the log is the
  console** (`logging/web.rs`, with a panic hook); **the watcher refuses**
  on the web (`watch/web.rs`), which the loop already reads as "hot reload
  off". `env_logger` and `notify` are native-only dependencies.
- The canvas takes the page's size; a manifest's window size is a desktop
  setting.

### In the browser, sound waits for a gesture and any game starts from its pack

The third web stage. The platformer example runs in a browser, is
walked by the keyboard and makes its coin's sound;
`crates/voltra-player/web/test/play.mjs` checks it on WebGPU and WebGL2.

- **`voltra_core::when_assets_ready(start)`** is how a game with its own
  `main` starts on both targets: at once natively; on the web after
  `game.pak` beside the page is fetched and mounted at
  `voltra_assets::pack::PACK_ROOT`. Its error, or the fetch's, is logged and
  shown in the page's `#voltra-status`. The player's web entry uses it too,
  so the fetch has one implementation. On the web
  `voltra_assets::default_root()` is the pack's asset folder (`/game/assets`,
  `PACK_ASSETS`) — a pack's layout is fixed, so a game needs no root of its
  own. `cargo xtask pack` packs a game build like a scene build, and the page
  imports `game.js` (`wasm-bindgen --out-name game`) whichever binary it is.
- **Sound starts on the first key, click or touch.** Browsers keep an
  `AudioContext` suspended until the user interacts; only a call made while
  handling that interaction lifts it (Godot's and Unity's web exports wait for
  the same gesture). cpal's `wasm-bindgen` feature gives WebAudio as the
  device; `Output::open`'s `play` is refused quietly there, so `App` calls
  `Audio::resume` (→ `stream.play()`) from the first pressed key, button or
  touch (`app/web.rs`). It waits while there is no device yet: the page
  remembers the gesture, so a stream opened after it starts on its own.
  Natively `resume` is a no-op on a playing stream.
- **The canvas takes the keyboard** when the game starts (`focus_window`):
  a page's keys go to the focused element, and a canvas has none until
  clicked.
- **Gamepads** need nothing of ours: gilrs reads the browser's Gamepad API
  on wasm by itself.
- **Testing the gesture headless.** Headless Chromium ignores the autoplay
  policy (with or without `--autoplay-policy=user-gesture-required`) and
  reports user activation from the first load, so `play.mjs` applies the
  rule itself from an init script — a context made before the first trusted
  key or pointer starts suspended, and `resume` before one waits — and
  watches the samples cpal copies into each `AudioBuffer`. Without
  `Audio::resume` the test fails; with it the context runs and the coin's
  samples are not silent.

### A web build is a folder of four files, made by `cargo xtask dist --web`

The last web stage; how to use it is [WEB.md](WEB.md).

- **`dist --web`** builds the manifest's target — the player without
  `windowed`, or the game's own binary or example — for
  `wasm32-unknown-unknown` in release, runs `wasm-bindgen --target web
  --out-name game`, writes `index.html` (the player's
  `web/index.html`, titled after the game) and packs the manifest and assets
  (`pack.rs`) into `dist/<name>-web/`. The pack is the folder's mark, as
  `build.ron` is a native build's: `prepare_build_dir` empties nothing else.
  Godot's web export and Unity's WebGL build ship the same three things — a
  page, the wasm with its loader, the game's data.
- **The CLI is checked, not installed.** `wasm-bindgen`'s CLI must equal the
  crate's version in `Cargo.lock`; xtask reads both before the long build and
  names the `cargo install` line when they differ. Installing it from xtask
  would put a minutes-long compile behind a typo'd flag, and CI installs a
  prebuilt one (`taiki-e/install-action`).
- **No wasm-opt, no size profile.** The release wasm is about 13 MB, 4 MB
  gzipped, which hosts compress on the fly. Shrinking it (`opt-level = "z"`,
  LTO, `wasm-opt`) would change the native release too or add a tool; it waits
  for a game that needs a smaller download.
- **No threads**, so no `COOP`/`COEP` headers: any static host serves it.
  Threads on the web wait for a game that needs them.
- The web tests build through `dist --web`, so CI runs the command a person
  ships with; a `python3 -m http.server` of the folder runs the game.

### A running editor is driven through a bridge, not by speaking MCP

MCP M3. The editor, started with `--bridge [port]`, listens on
`127.0.0.1` (port 47810 unless given) for `voltra-bridge` commands — one
JSON object per line, a `Request { id, command }` in and a
`Response { id, ok, error }` out. `voltra-mcp`'s `editor_*` tools are its
client. Without the flag nothing is opened.

- **Split as the established bridges split.** The Unity and Godot MCP
  bridges put a small listener inside the editor (a package, an editor
  plugin) and the MCP server in a separate process that connects to it;
  the editor never links the protocol. Ours is the same shape, for the same
  reason and one more: the editor stays free of `rmcp` and `tokio`, and the
  bridge protocol is ours to keep small — a closed `Command` enum, not a
  general RPC — so it changes only when a tool needs it.
- **Applied in the frame, not on the socket's thread.** Each connection
  gets a thread that reads a line, queues it as a `Pending` and waits for
  its answer; `Editor::ui` polls the queue once a frame — after the history
  has watched, before any panel draws — and answers each request with what
  it did. A client's requests land one at a time, in order.
- **An AI edit is a person's edit.** Spawn, delete, reparent, rename and
  component set/remove go through `voltra_scene::edit` — the same
  operations the file tools use — wrapped in `History::begin` /
  `commit_including`, one entry per call, labelled `AI: spawn`, `AI: set
  component`…: Ctrl+Z takes it back, and the Edit menu says what it would
  undo. A refused edit records nothing. Edits are refused while playing, as
  the inspector's are; reads answer the live world.
- **Play through the toolbar's paths.** `play`, `pause`, `step` (pauses
  first; 1–600 fixed steps) and `stop` (the authored scene back) call what
  the toolbar calls. Input while playing holds an action for N frames
  (`voltra_core::input::HeldActions`, ticked in `App::update` and shared
  with `Headless` through `drive_action`), as a player would press it.
- **Seeing is the viewport as drawn.** `UiFrame::capture_viewport` reads
  back the graded viewport target — the scene lit and post-processed, no
  overlay — as a PNG, base64 in the answer and an image block in MCP.
- **Explicit, not a silent fallback.** The file tools and the `editor_*`
  tools are separate names rather than one set that falls back to files
  when no editor answers: a file written under an editor holding the same
  scene open would be overwritten by its next save. No editor is an
  answer naming the `--bridge` flag.
- **Local only.** Bound to loopback, and each peer is checked to be
  loopback all the same.

Pinned by `voltra-bridge`'s `client.rs` and `protocol.rs` tests (a request
queued for the frame and its answer carried back, no editor listening an
error, every command reading back from its line),
`voltra-core/src/input/drive.rs` (an action held its frames then let go,
held again restarting the count) and
`voltra-editor/src/bridge/edits/tests.rs` (a spawn and a component set by
name path each one entry that undo restores; a refused edit records
nothing) and `voltra-mcp/tests/editor.rs` (tools reach a stand-in editor
and carry its answers, the screenshot as an image block; no editor is an
answer naming the flag).

### The game is a crate the editor loads; its stamp decides whether it may

Stage 76. A game is a crate exposing one value through `voltra_core::Game`:
`register` names its components, `start` comes before its first turn,
`update` and `fixed_update` are the two turns `with_update` and
`with_fixed_update` already gave closures. Its own binary links it and
hands it to `App::with_game`; the editor loads the same crate's `cdylib`
and runs a fresh one for every play. Fyrox's plugin model, the closest
engine to ours: Rust all the way, the editor hosts the game's code, a
shipped build links it statically. `games/platformer` is the first: a
`cdylib` + `rlib` library, a binary, and `scenes/platformer.ron`.

- **A game acts on the scene, not on entities it made.** The platformer
  finds its `Walker`, its `Coin`s (now with a `value`), its `Goal` (naming
  the next scene) and its `CoinCounter` by querying; the level is a scene
  file authored like any other. In the editor the scene is whatever the
  author made, and Play runs the game on it — the shape Unity's
  `MonoBehaviour`s and Godot's scripts have.
- **Two symbols** (`voltra_core::export_game!`): `voltra_game_stamp`, a C
  function answering a `#[repr(C)]` `Stamp`, and `voltra_game_new`, a Rust
  function answering a `Box<dyn Game>` — called only once the stamp
  matches. Rust has no stable ABI, and the world is keyed by `TypeId`,
  which hashes the compiler, the crate metadata and every feature in: a
  library whose `Transform` has another `TypeId` would find no transforms,
  silently. So the stamp carries the plugin protocol's version, `rustc -V`
  (a build script of `voltra-core`), the engine version, the profile, and a
  fingerprint of the `TypeId`s and layouts of the types a game shares with
  its host; `Stamp::check` names the first that differs — the compiler
  before the types it changes. In practice: build the editor and the game
  in **one** `cargo build -p voltra-editor -p <game>`, so features unify.
  Each half otherwise carries its own copy of the engine, and that is
  sound exactly when the types are the same: `tests/plugin.rs` loads the
  library cargo built for the test run (which is why the crate has no
  dev-dependencies) and plays a level read by the host with it.
- **The library's `log` forwards to the host's** through one C function per
  record: a library carries its own `log`, with no logger in it, and a Rust
  trait object passed between two copies of the crate that defines it is
  not one.
- **Nothing of a library outlives it.** It is loaded from a copy (Windows
  will not let a build overwrite a loaded library; a loader asked twice for
  one path hands back the first load), and copies left by earlier
  processes are removed at the next load. A `GameLibrary` dropped as it is
  stays loaded for the rest of the process; only the unsafe `close` unloads
  it, and each game it made keeps it loaded until the game drops. The
  editor unloads by writing the scene out through the game's registry,
  `World::clear` — every entity and every storage, since a storage runs
  the code of the crate that created it — reading it back through the
  engine's (the game's components kept as unknowns), resetting both
  registries, and only then closing. Loading is the same rebuild the other
  way, and a scene the game's types refuse is read back as it was.
- **Where the game is**: `build.ron`'s `game.package`, its library beside
  the editor's executable — where the one `cargo build` puts both — or
  `--game` naming a library or a build manifest (`--game
  platformer.build.ron`). Game ▸ Reload / Unload; the menu says which is
  loaded, or why none could be.
- **The game's components in the inspector** (`inspector/game_components`):
  every registered name the engine does not have, edited as the RON a scene
  file holds and applied the moment it reads as the type, the reason shown
  while it does not — Godot's fallback for a type with no editor of its
  own; removed with ✕, added from its template.
- **Found on the way**: the asset watcher counted inotify's access events —
  every open and close of a read — as changes, so a reload read the file
  and raised another, for ever, and each reload synced prefab instances
  over their unsaved edits. A read is now no change (`tests/watch.rs`).

Not yet, and why: **the profiler inside a game** — the library's
`voltra-profile` is its own copy, so its scopes are not in the editor's
profile; **a game's threads** — none may outlive its library, and nothing
here starts any.

Rejected: **an embedded language** (Lua, rhai) — two languages and a
doubled API, against the point of the engine; **wasm-hosted scripts**
(wasmtime) — heavy, no host inside a browser, a call boundary on every
tick; **the engine as one shared `dylib`** (Bevy's `dynamic_linking`) —
the game and the editor would share one copy, but only when built
together, which the stamp already requires, at the price of shipping the
toolchain's `libstd` beside the editor; **trusting a version string** —
two builds of one version with different features are different types.

Pinned by `voltra-core`'s `game.rs` (a game registers and starts before its
first turn; hooks before the game; a Step steps it), `game/stamp.rs` (each
difference named, the compiler first), `game/plugin.rs` (a record crosses
as its level, target and text), `game/library.rs` (a missing library says
build it; a file that is no library is refused and leaves no copy),
`voltra-ecs`'s `clear`, the editor's `game/project.rs`, `game/rebuild.rs`
(a game component unknown and back; a scene the game cannot read left as
it was) and `inspector/game_components.rs`, and `games/platformer`'s
`tests/level.rs` and `tests/plugin.rs`. Checked by hand: the editor under
Xvfb with `--game platformer.build.ron --bridge`, the coins set to 5
through the bridge, Play, walked right — `Coins 5 / 15` — and Stop.

### A changed game is rebuilt and swapped in mid-play, through the registries

Stage 77. The editor watches the game crate named by the manifest
(`voltra_core::game::watch`, found with `cargo metadata`), and a change to
a `.rs` file or the manifest — not under `target`, and not a read, which
inotify reports too — queues a build. The build runs off the frame
(`game::builder`, cargo's stderr read on its own thread); the old code keeps
running until it is done. A library that built is opened and installed
in place of the old one, mid-play too; a change made during a build queues
one more, so the last edit is the one built.

- **The swap is the scene's own round trip** (`game::swap::rebuild`): the
  world written out through the old library's registry, cleared — entities
  and storages — and read back through the new one's, then the old library
  closed. What survives is what a scene file keeps: every entity with a
  `SceneId` and its registered components, a body's velocity included.
  The play session goes on with a fresh game from the new library, whose
  `start` runs again. Fyrox reloads the same way, by serializing its scene
  across the swap; Unity's domain reload does too, and loses what it does
  not serialize, as this does.
- **State that must outlive an edit lives in a component.** The
  platformer's count moved from its `Platformer` struct to `CoinCounter
  { taken, total }`, which a save, a play snapshot and a reload all carry;
  `start` sets the total only on a counter that has none.
- **Built with the editor.** The build is `cargo build -p <game>` plus
  `-p voltra-editor` when the editor is in the game's workspace, with the
  editor's profile: every dependency then resolves the features the
  running editor was built with, or the stamp would refuse the result.
  When only the game changed, cargo relinks nothing of the editor; an
  engine edit relinks it too, and the new library is refused by its stamp
  — on Windows the relink itself fails, a running executable being locked
  — and either way the editor says so and keeps the old code.
- **The game lives in the editor's workspace.** Found by the scratch-crate
  test: built from a workspace of its own, a game's engine crates came out
  with every flag the editor's had and other `-C metadata` — cargo hashes a
  path dependency by its path *from the workspace's root* — so other
  `TypeId`s, and the stamp refused it (`Mismatch::Types`, as it should).
  A game and the editor that plays it share one workspace; the template of
  78 puts the editor in the game's.
- **A failed build is shown, not hidden**: the Game menu says the old code
  runs and lists the compiler's errors with where they are
  (`builder::summary`), and the log has them. Game ▸ Rebuild builds on
  demand.
- **A scene the new types cannot read** — a component whose saved value
  the edited type no longer accepts — is read back through the old
  library's, which wrote it; the old game goes on and the menu says why.

Rejected: **patching code in place** (hot-patching functions, as
`hot-lib-reloader` or Live++ do) — the world would keep values of the old
types, whose layout the edit may have changed; the round trip through text
is the one migration that cannot misread a field; **reloading only between
plays** — the iteration loop this stage exists for is a level being played
while its code is edited; **building the game alone** — its dependencies
would resolve fewer features than the editor's, and every rebuild be
refused.

Pinned by `game/builder.rs` (a command that succeeds is built; one that
fails answers cargo's own words; the summary is the errors and where they
are), `game/watch.rs` (an edited source is a change; notes and build
output are not), the editor's `game/source.rs` (built with the editor when
they share a workspace; this workspace's platformer found by `cargo
metadata`), `games/platformer/tests/plugin.rs` (a reload mid-play keeps
the walker where it was and the count, and the new code walks it on) and
`tests/hot_reload.rs`, ignored by default for its build: a scratch crate
built, played, its tick's constant edited, the edit seen, rebuilt, swapped
in mid-play with its `Mover`'s distance carried across and the new speed
applied; then a broken edit that fails to build, names the error, and
leaves the last good code running. Checked by hand: the editor under Xvfb
playing the platformer, `WALK_SPEED` edited in `walker.rs`, rebuilt and
swapped in three seconds later, the walker at the new speed and the
counter as it was.

### A new game is a crate in the engine's workspace, made from a template

Stage 78. `cargo xtask new-game <name> [--title T]` writes
`games/<name>/`: `Cargo.toml` (`cdylib` + `rlib`, a `[[bin]]`, workspace
dependencies, no dev-dependencies), `src/lib.rs` (a `Game` moving a
`Player` component by the move actions, `export_game!`), `src/main.rs`
(through `when_assets_ready`, opening `scenes/main.ron`), `build.ron`
(`game: (package: …)`, its own `assets`), the first scene, a `README.md`,
and `tests/plugin.rs`, which loads the crate's own library as the editor
does and plays the scene headless with a key held. The engine's starting
assets are copied in: the input map and sorting layers the loop reads by
convention, the default font and its licence, a texture. It is formatted
after it is written, and refuses a folder that is there already.

- **In this workspace, not a project of its own** — the finding of 77: a
  game and the editor that plays it must share a workspace root, or every
  engine type has another `TypeId` and the stamp refuses the library.
  `games/*` is a member glob, so a new game builds at once with the editor
  beside it. A game in a repository of its own needs the editor built in
  that repository too — an editor launcher crate depending on the editor
  as a library, Fyrox's `editor` package — which is a stage of its own.
- **A game brings its assets.** `--game games/<name>/build.ron` makes the
  editor's asset root the folder that manifest names: the game's scenes,
  input map and sprites are the ones it opens and watches. `dist` and
  `dist --web` take the manifest as they are.
- **Names**: lowercase letters, digits and single dashes, starting with a
  letter; the type is the name in CamelCase (`space-trader` →
  `SpaceTrader`), refused where it would shadow a type the template uses
  (`game`, `tick`, `player`, `app`); the title is the name's words
  capitalised unless given.

CI generates `ci-game` and runs its tests and clippy (Linux), and builds
it for the web (the web job). Pinned by `xtask/src/new_game.rs` (a name
becomes a package, a crate, a type and a title; bad names refused; every
file filled in, the assets copied, the manifest planned by `dist` and
`dist --web` as it is, and never written over a game) and the editor's
`game/project.rs` (the asset root beside the manifest). Checked by hand: a
generated game played in the editor under Xvfb through the bridge.

### UI images are sprites; panels 9-slice and stack their children

Stage 79. A menu built from a skin sheet: panels and button faces that keep
a crisp frame at any size, and children laid out in rows and columns.

- **An image is a `Sprite` on a `UiRect`** — the plan's `UiImage`. A
  sprite already names a texture, a region or an atlas frame, carries a
  tint, resolves its handles on load and hot reload, and has an inspector
  with pickers; a second image type would have duplicated all of it for
  the same five fields. The world-only fields (pixels per unit, normal map,
  light layers) are ignored in screen space, as `Transform` is.
- **`UiSlice`** draws the element's sprite as nine pieces: corners at
  their own size, edges stretched along one axis, the middle along both
  (`center: false` leaves a frame around nothing). Borders are `Edges` in
  texels of the drawn frame, drawn at `scale` points per texel; borders
  wider than the rect shrink in proportion until they meet, as Unity's do;
  a mirrored sprite draws its right border on the left. On the element, as
  Godot's `NinePatchRect` margins, rather than on the image as Unity's
  sprite `border`: an atlas here is a list of frames with no per-image
  metadata, and one frame sliced two ways costs nothing this way. Without
  the frame's size in texels — no texture store, a missing frame — it is
  stretched whole. A linear sampler blends a border's last texel into the
  first pixels of the stretched piece next to it, as in every engine;
  skins pad their borders or use a nearest-filtered texture.
- **`UiStack`** places the `UiRect` children of its element one after
  another — `Row` left to right, `Column` top to bottom — `spacing` apart
  inside `padding`, the run justified `Start`/`Center`/`End` along the
  stack and each child aligned (or `Stretch`ed, the default as Godot's
  fill) across it. Godot's `BoxContainer`, Unity's layout groups. A child
  keeps the size its own `UiRect` resolves to in the padded box; its
  position is the stack's. Laid out inside `UiLayout::from_world`: the
  first child reached arranges the whole stack, since where one goes
  depends on the rest. Children past the end overflow rather than vanish.
- **Order is `UiRect::slot`**, then entity index. The hierarchy keeps no
  sibling order (a stored list was rejected for `Parent`), and a scene file
  writes entities in id order, so a menu's reading order has to be said.
  `0` is not written. Rejected: sibling order in `Parent` — it would change
  every scene and the hierarchy panel for one consumer; a list of ids on
  the stack — duplicate and prefab instancing remap `Parent`, not ids
  inside other components.
- **`UiButton::frames`** swaps the sprite's atlas frame by state — Unity's
  sprite-swap transition — frames of the button's own atlas, since a skin
  is one sheet. The tint still applies, so an image button keeps its
  colours white. `ButtonFrames::in_a_row(first)` is how a skin is cut.
- Inspector: *9-slice* and *Stack* sections with add and remove buttons,
  the state frames under the button's colours, the slot on a stacked
  child's rect. *Scene ▸ Spawn HUD column*: a centred panel that stacks.

Pinned by `ui/slice.rs` (nine pieces, corners at size, borders scaled by
points and pixels, borders that meet, no middle, mirroring, bad numbers),
`ui/stack/tests.rs` (columns and rows, slots over spawn order, centring
and end, grandchildren inside a stacked parent, overflow, a despawned
child), `ui/button/tests.rs` (a frame per state, none without frames) and
`tests/headless_menu.rs`: a pause menu — a 9-sliced panel stacking a title
and three image buttons spawned out of order — written to a scene file and
read back, laid out in slot order, a click on a label clicking its button,
the arrow keys walking the column, and drawn on the GPU with the panel's
and buttons' frames 4 points thick and each button's middle its state's.
Checked by hand: the editor under Xvfb, a pause menu spawned through the
bridge with the coin sheet as its skin, drawn 9-sliced in slot order. That
check found the bridge's edits never looked up the handles of what they
wrote — a textured sprite spawned by a tool drew the placeholder, a text
nothing — so an applied edit now resolves the scene's assets.

### Sliders, toggles, text fields and scrolls share the buttons' focus

Stage 80. An options screen: a volume slider, a fullscreen toggle, a name
field and a list of saves that scrolls — by the mouse, the keyboard and a
pad.

- **`UiControls`** (was `UiButtons`) holds the one hover, press and focus
  every control shares, as Godot's viewport keeps one focus owner, and
  this frame's `clicked`, `changed` and `submitted`.
  `update(world, layout, fonts, &UiInput)` takes the pointer (with the
  wheel and Shift), the navigation and the typing; it stays a pure
  function in `voltra-scene`. A control is a `UiButton` (a toggle is one),
  a `UiSlider` or a `UiTextField`; one with nothing drawn still takes the
  pointer — an invisible hit area — and a click on a child reaches the
  control above it.
- **`UiToggle`** rides on a `UiButton`, as Godot's `CheckBox` is a button
  in toggle mode: a click flips `on` and counts as a change; while on, the
  button's colour is multiplied by `on_tint`, and `on_frames` replaces its
  frames.
- **`UiSlider`**: `value` in `min..max`, snapped to `step`; a press or a
  drag along its rect sets it (a column rises from the bottom); while
  focused, the navigation along it steps it and across it moves the focus
  on, as Godot's sliders do. It draws nothing itself: children marked
  `UiSliderPart::Fill` stretch to the value and `Handle` sits on it, as
  Unity's `fillRect` and `handleRect` — any picture, 9-sliced or flat.
  Marker components rather than references: a `Parent` link is remapped
  on duplicate and prefab instancing, an id inside a component is not.
- **`UiTextField`** is Godot's `LineEdit`. The entity's `Text` is its
  style and the field writes what it shows into the text's content — the
  text, the IME composition at the caret, or the placeholder drawn at half
  alpha — so the font atlas prepares what is shown. Editing is in
  characters, never bytes: arrows, Home/End, Shift to select, Ctrl (Option
  on macOS) for words, Backspace/Delete, Ctrl+A, Enter to submit; a click
  places the caret and a drag selects. Control characters are left out:
  one line. The text scrolls sideways to keep the caret in the box and is
  cut to it; the caret is a whole-pixel bar, not blinking. Caret stops
  come from the face's metrics (`shape::caret_stops_at`), no glyph needed.
  No system clipboard yet: nothing in the engine reaches one.
- **Typing** is `Input::typing`: a key press's `text` (not with Ctrl or
  the logo key, not a control character) and the IME's `Commit` are typed;
  `Preedit` shows at the caret; editing keys count their repeats. The loop
  calls `set_ime_allowed(true)` only while a field has the focus — winit's
  own advice for games, since an allowed IME swallows keys while it
  composes — and puts the candidate window under the field. A game reading
  its own keys can check `tick.ui.editing(world)`.
- **`UiScroll`** shifts its UI descendants by `offset` and masks them to
  its rect. The mask is applied on the CPU: every UI quad is axis-aligned,
  so one cut to the mask with its UVs moved in proportion draws what a
  scissor would, without a draw call per mask (Godot's `clip_contents`
  scissors; Unity's `RectMask2D` clips in the shader). `UiLayout` records
  each element's clip, the nearest scroll above it and each scroll's
  content box; a masked-off part is not picked. The wheel scrolls the
  innermost scroll under the pointer; the focus moving to a control —
  by the navigation or `focus()` — scrolls every scroll above it just
  enough to show it, which is how keys and pads scroll; the offset stays
  between none and the content's reach, a stack's padding included.
- **Headless controls**: `Headless::with_screen(size)` lays the UI out
  windowless, and `input_mut()` feeds the cursor, buttons, wheel, keys,
  pad and typing as a window would.
- Editor: inspector sections for each, *Scroll children* on any UI
  element, and *Scene ▸ Spawn HUD toggle / slider / text field / scroll
  list*.

Rejected: **a widget tree with its own event loop** (egui, iced) — the
game's UI is entities the editor authors and scenes save; **an ECS
system per control with its own focus** — Godot and Unity both have one
focus per screen, and two focuses is a bug a pad finds at once.

Pinned by `ui/controls/tests.rs` and `controls/tests/widgets.rs`
(buttons; a toggle flipping and tinting; a slider dragged off its track
and stepped, passing the focus across; a field typed into, submitted and
left by a click elsewhere, keeping left and right; the wheel within the
content; the focus scrolling a list, a hidden row unclickable),
`text_field/tests.rs` (characters not bytes, selections, words, limits,
the composition until it commits), `text_field/view.rs` (scrolling to
the caret), `slider.rs`, `scroll.rs`, `layout/tests.rs` (a scroll moving
and masking, nested masks, a stack inside a scroll), `mesh/clip.rs`,
`mesh/tests.rs` (the caret after the text, the selection under it, a long
text scrolled and cut to its box, the faint placeholder), core's
`input/typing.rs`, and `voltra-core/tests/ui_options.rs`: the options
screen by the mouse (slider pressed and dragged, toggle by its label,
the field clicked and typed in, the list wheeled and a row clicked), by
the keyboard (arrows and Enter through the default map, typing, the list
scrolling to the focus) and by a pad (D-pad and South). Checked by hand:
the player under Xvfb with `xdotool` — real X11 clicks, typing,
Backspace, Shift+Home, the toggle, the slider and the wheel.

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
| Sampler | `mipmap_filter` takes `MipmapFilterMode`, a separate type from `FilterMode` |
| Pipeline | `VertexState::buffers` is `&[Option<VertexBufferLayout>]` — the entries are wrapped in `Option` |
| Device poll | `PollType::Wait` is a struct variant; use `PollType::wait_indefinitely()` |
| Buffer readback | `Buffer::get_mapped_range` returns `Result` |
| Timestamps in encoders | `CommandEncoder::write_timestamp(&QuerySet, index)` needs **both** `Features::TIMESTAMP_QUERY` and `TIMESTAMP_QUERY_INSIDE_ENCODERS` requested; ticks × `Queue::get_timestamp_period()` = ns (1.0 on the web) |
| Query resolve | `resolve_query_set(&set, range, &buffer, offset)` into a `QUERY_RESOLVE \| COPY_SRC` buffer, 8 bytes (`QUERY_SIZE`) a query; copy to a `MAP_READ` buffer to read |
| Non-blocking map | `buffer.map_async(MapMode::Read, range, callback)` then `device.poll(PollType::Poll)` each frame; the callback must be `Send + 'static` |
| Compute pipeline | `ComputePipelineDescriptor { label, layout, module, entry_point: Option<&str>, compilation_options, cache }` |
| Compute pass | `begin_compute_pass(&ComputePassDescriptor { label, timestamp_writes })`; `dispatch_workgroups(x, y, z)` |
| Buffer copy | `copy_buffer_to_buffer(src, offset, dst, offset, size)` — `size` is `impl Into<Option<u64>>` |
| Downlevel | `adapter.get_downlevel_capabilities().flags`; vertex-stage storage needs `DownlevelFlags::VERTEX_STORAGE` |
| D3D12 / FXC | A WGSL loop that writes a vector component by a **runtime index** (`v[i] = …`) fails pipeline creation: FXC must unroll it and cannot (`X3511`). Write the components out. Windows CI hits this; reproduce locally with `WGPU_BACKEND=dx12 WGPU_DX12_COMPILER=fxc cargo test` |
| Data textures | A normal map must be sampled through a non-sRGB view (`Texture::raw_view`, `Textures::raw_bind_group`); the sRGB view decodes it as colour. The view format must be listed in `view_formats` at creation, which `Texture::from_rgba8` does — except on GL, which cannot, where it uploads a second `Rgba8Unorm` copy |
| `Min` / `Max` blend | Both factors must be `BlendFactor::One`. `R16Float` is blendable on every device (`guaranteed_format_features`); `R32Float` only with `Features::FLOAT32_BLENDABLE` |
| Headless tests | `headless_device()` needs an adapter; in a Linux container without a GPU, `apt-get install mesa-vulkan-drivers` (lavapipe) makes the GPU tests run instead of skipping |
| WGSL names | `from` is a reserved word (naga `keywords/wgsl.rs`); builtins like `step` may be shadowed but are best avoided |
| Web backends | `wgpu::util::new_instance_with_webgpu_detection(desc).await` drops `BROWSER_WEBGPU` when the browser has no working WebGPU, so the `webgl` feature's GL backend serves; detection must happen before the instance exists |
| Web canvas formats | A WebGPU canvas offers `bgra8unorm` / `rgba8unorm` / `rgba16float`, never sRGB: configure the linear format with its sRGB variant in `view_formats` and draw through that view |
| Web buffers | `Drop` for a web `Buffer`/`Texture` is a no-op — the browser frees it when the JS wrapper is collected. Call `destroy()` on anything made per frame (after the submit) |
| Web limits | `Limits::default()` fails `request_device` on WebGL2; ask for `adapter.limits()` when `Limits::default().check_limits(&adapter.limits())` is false |
| GL `VIEW_FORMATS` | The GL backend (WebGL2 included) never sets `DownlevelFlags::VIEW_FORMATS`; a texture declaring `view_formats` fails validation there |
| Device lost | `device.set_device_lost_callback(callback)`, a `Fn(DeviceLostReason, String)`; natively it fires only on loss or an explicit `destroy`, not on drop |
| Headless Chromium | WebGPU canvases need `--enable-unsafe-webgpu --enable-features=Vulkan --use-vulkan=swiftshader --use-angle=vulkan --enable-unsafe-swiftshader`; on ANGLE's GL path the swap chain's shared image cannot be made and the device is lost on the first present |

When touching wgpu, read the vendored source under
`~/.cargo/registry/src/index.crates.io-*/wgpu-30.0.0/src/api/` or query Context7
rather than trusting a tutorial.

### winit 0.30 on the web

| Thing | winit 0.30.13 (`src/platform/web.rs`, `platform_impl/web/`) |
| --- | --- |
| Run | `EventLoopExtWebSys::spawn_app(app)` returns at once; `run_app` throws to unwind |
| Canvas | `WindowAttributesExtWebSys::with_append(true)` appends it to `<body>`; focusable (tabindex) by default |
| Size | A requested inner size is written into the canvas's **style** and pins it; leave it out and size the canvas in CSS — a `ResizeObserver` sends `Resized` |
| Proxy | `EventLoop::create_proxy` only (not on `ActiveEventLoop`); the web proxy is an `mpsc::Sender<T>` plus a wake, so `T` need not be `Send` |

### egui 0.35 API notes

Same problem, same rule: read the source, not a tutorial.

| Thing | egui 0.35 |
| --- | --- |
| Frame | `Context::run` is gone; `Context::run_ui(input, \|ui\|)` hands out a root `Ui` |
| Panels | `SidePanel` and `TopBottomPanel` are merged into `Panel::{left,right,top,bottom}` |
| Panel host | `Panel::show` and `CentralPanel::show` take a `&mut Ui`, not a `&Context` |
| Panel size | `default_size`, not `default_width` / `default_height` |
| Menus | `MenuBar::new().ui(ui, …)`; close an open menu with `ui.close()` |
| Images | `ImageData` has only the `Color` variant; the `Font` one is gone |
| Widget input | `egui::Image` is inert unless given `.sense(Sense::drag())`; without it the `Response` reports no drag and no hover |
| Scroll | Read `InputState::smooth_scroll_delta`, not the raw one — a `ScrollArea` zeroes it once it has consumed it, which is what scopes the wheel to a panel |
| Pointer position | `Response::hover_pos` is in global screen points; subtract `response.rect.min` for widget-local ones |
| Shortcuts | `InputState::consume_shortcut` matches modifiers *logically* — extra Shift and Alt are ignored, so `Ctrl+Z`'s pattern also accepts `Ctrl+Shift+Z`. Test the most specific one first |
| Colour picker | `color_edit_button_*` edits inside a popup, so its own `Response` is never dragged or focused while the value moves. Its popup id is `ui.auto_id_with("popup")` read *before* the button is allocated; `auto_id_with` does not advance the counter, so the caller can read the same id and ask `Popup::is_id_open` |

### `cpal` 0.18 and `symphonia` 0.6 API notes

Both differ from most training data and from every tutorial written against
`cpal` 0.15 or `symphonia` 0.5.

| Thing | 0.18 / 0.6 |
| --- | --- |
| `cpal::SampleRate` | a `u32` type alias, not a newtype — `sample_rate: 48_000`, not `SampleRate(48_000)` |
| `build_output_stream` | takes `config: StreamConfig` **by value**, plus a trailing `timeout: Option<Duration>` |
| Starting a stream | `stream.play()` is required on every backend; before 0.18 some started themselves |
| Error type | one `cpal::Error` with an `ErrorKind`, in place of the old per-operation error enums |
| Picking a format | `default_output_config()` may not be `F32`; filter `supported_output_configs()` by `sample_format()` and take `try_with_standard_sample_rate()` |
| `symphonia` probe | `get_probe().probe(&hint, mss, fmt_opts, meta_opts)` returns the `FormatReader` directly, not a `ProbedFormat` with a `.format` field |
| Track selection | `reader.default_track(TrackType::Audio)`, which falls back to the first track with a known codec |
| Decoder | `get_codecs().make_audio_decoder(params.audio()?, &AudioDecoderOptions)` — audio codec params are a variant of `CodecParameters` now |
| End of stream | `next_packet()` returns `Ok(None)`; it is not an `IoError` of kind `UnexpectedEof` any more |
| Samples out | `decoded.copy_to_vec_interleaved(&mut vec)` **resizes** its destination, so decode into a scratch `Vec` and extend from it |
| `AudioSpec` | not `Copy`; read `rate()` and `channels().count()` rather than storing the spec |
| Web | Without the `wasm-bindgen` feature cpal on `wasm32-unknown-unknown` has only the null host (no device). With it, the WebAudio host makes an `AudioContext` per stream and schedules buffers on timers; `play()` is `ctx.resume()`, which a browser honours only during a user gesture. `audioworklet` needs atomics (threads) |

### `fontdue` 0.9 notes

| Thing | 0.9.5 |
| --- | --- |
| `Layout` | does **not** kern, and `HorizontalAlign` does nothing without `max_width` — we lay out ourselves |
| Kerning | `Font::horizontal_kern_indexed(left, right, px) -> Option<f32>` |
| Bitmap | `rasterize_indexed(glyph, px) -> (Metrics, Vec<u8>)`: one coverage byte per pixel, rows from the top |
| `Metrics` | `xmin`/`ymin` are the bitmap's bottom-left from the pen on the baseline, y up |
| Errors | `Font::from_bytes` fails with a `&'static str`, not an error type |
| `Font` | `Clone`, not `Debug` |

### `gilrs` 0.11 notes

| Thing | 0.11.2 |
| --- | --- |
| Names | `Button::LeftTrigger` is the **bumper**; `LeftTrigger2` is the analog trigger |
| Events | `next_event()` never blocks; `EventType::{ButtonPressed, ButtonReleased, ButtonChanged(_, value, _), AxisChanged(_, value, _), Connected, Disconnected, …}` — `#[non_exhaustive]`, so match with `_` |
| Ids | `GamepadId` → `usize::from(id)`; stable for a pad while connected, reused by UUID on reconnect |
| Errors | `Gilrs::new()` returns `Error::NotImplemented(dummy)` on unsupported platforms — usable, but we just skip pads |
| Filters | default filters apply a small stick dead zone and axis-to-button thresholds before we see values |
