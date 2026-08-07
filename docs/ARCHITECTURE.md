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
        ┌──────────────────┐
        │  voltra-editor   │  binary — wires everything together
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │   voltra-core    │  platform: event loop, window, input, time
        │  (owns winit)    │
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │   voltra-scene   │  components and the geometry they become
        └─┬──────────┬───┬─┘
          │          │   │
┌─────────▼────┐  ┌──▼───▼───────────┐  ┌──────────────────┐
│  voltra-ecs  │  │  voltra-render   │◄─┤  voltra-assets   │
│  (no deps)   │  │  (owns wgpu)     │  │  cache, loading  │
└──────────────┘  └──────────────────┘  └──────────────────┘
```

`voltra-scene` is the only crate that knows about both entities and vertices.
Keeping that knowledge in one place is what lets `voltra-ecs` stay free of
rendering and `voltra-render` stay free of entities.

`voltra-assets` points into `voltra-render` because the thing it caches *is* a
GPU texture — caching decoded bytes and re-uploading per sprite would cache the
cheap half. It reaches `Device` and `Queue` through `voltra_render::wgpu` and
declares no `wgpu` of its own, so the one-crate-per-backend rule holds.

**Rule:** exactly one crate may depend on `winit` (`voltra-core`) and exactly one
may depend on `wgpu` (`voltra-render`). Everything else consumes them through
re-exports, so a version bump is a one-line change.

### Current crates

| Crate | Owns | Key types |
| --- | --- | --- |
| `voltra-ecs` | Entity handles and component storage. No dependencies at all | `World`, `Entity`, `SparseSet` |
| `voltra-assets` | Asset identity, the texture cache, loading from the asset root | `Handle`, `Assets`, `AssetPath`, `Textures` |
| `voltra-render` | GPU device, swapchain, frame recording, the egui backend | `GpuContext`, `Renderer`, `RenderTarget`, `EguiBackend` |
| `voltra-scene` | Scene components and their geometry | `Transform`, `Sprite`, `SpriteBatch`, `pick::sprite_at`, `SceneFile`, `ComponentRegistry`, `SceneId` |
| `voltra-core` | Event loop, OS window, input, frame timing, the egui seam | `App`, `UiFrame`, `EguiLayer`, `Input`, `Clock` |
| `voltra-editor` | Editor binary and its panels | `main`, `Editor` |

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
       ├─ App::update                 input → camera
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
therefore its own alpha. Here `Sprite` holds a colour and the renderer binds one
texture for the whole batch, so there is no per-sprite alpha to test. A quad test
is not an approximation here; it is the exact answer until sprites get their own
textures.

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
