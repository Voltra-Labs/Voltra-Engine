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
  ┌──────▼─────┐   ┌───────▼───────┐  identity, texture cache, loading
  │ voltra-ecs │   │ voltra-assets │
  │ (no deps)  │   └───────────────┘
  └────────────┘           │
                   ┌───────▼───────┐  owns wgpu
                   │ voltra-render │
                   └───────────────┘
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

**Rule:** exactly one crate may depend on `winit` (`voltra-core`) and exactly one
may depend on `wgpu` (`voltra-render`). Everything else consumes them through
re-exports, so a version bump is a one-line change.

### Current crates

| Crate | Owns | Key types |
| --- | --- | --- |
| `voltra-ecs` | Entity handles and component storage. No dependencies at all | `World`, `Entity`, `SparseSet` |
| `voltra-assets` | Asset identity, the texture cache, loading from the asset root | `Handle`, `Assets`, `AssetPath`, `Textures` |
| `voltra-render` | GPU device, swapchain, frame recording, the egui backend | `GpuContext`, `Renderer`, `RenderTarget`, `EguiBackend` |
| `voltra-scene` | Scene components and their geometry | `Transform`, `Sprite`, `SpriteBatch`, `pick::sprite_at`, `SceneFile`, `ComponentRegistry`, `SceneId`, `RigidBody`, `Collider`, `PhysicsMaterial`, `CollisionLayers`, `Sensor`, `Name`, `Parent`, `WorldTransforms` |
| `voltra-physics` | Simulation over those components: integration, contact detection, the solver that resolves them, and the questions a game asks of them | `PhysicsWorld`, `PhysicsClock`, `candidate_pairs`, `Contact`, `Overlaps`, `step`, `SolverParams`, `ImpulseCache`, `Touching`, `CollisionEvent`, `query::ray`, `QueryFilter`, `RayHit` |
| `voltra-core` | Event loop, OS window, input, frame timing, the egui seam | `App`, `UiFrame`, `EguiLayer`, `Input`, `Clock` |
| `voltra-editor` | Editor binary and its panels | `main`, `Editor` |
| `voltra-player` | Player binary: a scene, a window, no editor | `main`, `args::Args`, `scene::load` |
| `voltra-testkit` | Headless GPU scaffolding for tests. `publish = false`, and only ever a `[dev-dependencies]` entry | `headless_device`, `read_texture`, `scratch_root`, `write_png` |

### Planned crates

Added only when there is code to put in them:

| Crate | Purpose | Blocked on |
| --- | --- | --- |
| `xtask` | Repo automation written in Rust instead of shell | when scripts appear |

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

**`Scene ▸ Open` loads before it despawns.** The obvious implementation clears
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

### The broad phase is O(n²), and detection shipped before resolution

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
its collider** — `I = m(hx² + hy²)/3` for a box, `I = m·r²/2` for a disc, both in
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

`crates/voltra-core/examples/platformer.rs` is the API's first user: walk with
`A`/`D`, jump with `Space` when there is something flat underfoot, and a camera
that eases after the walker. It is an example rather
than a crate because it is documentation that has to keep compiling, and
`cargo clippy --workspace --all-targets` builds it.

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

When touching wgpu, read the vendored source under
`~/.cargo/registry/src/index.crates.io-*/wgpu-30.0.0/src/api/` or query Context7
rather than trusting a tutorial.

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
