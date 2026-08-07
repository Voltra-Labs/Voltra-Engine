# Scene serialization

Date: 2026-08-07
Branch: `feature/scene-serialization`
Status: approved, not yet implemented

This is 2D, per the scope section at the top of CLAUDE.md and the "When 3D
arrives" entry in ARCHITECTURE.md. A 2D transform is written as two floats, not
three, and the file records which kind of entity it holds so a future 3D scene is
a new record shape rather than a reinterpretation of this one. A file format is
the one artefact that cannot be refactored freely later — files written in the
old shape already exist — which is why that decision was settled before this spec
was written.

## Scope

The README lists stage 9 as "Scene serialization and the asset pipeline". Those
are two subsystems and this spec covers only the first: writing a world to a file
and reading it back. Loading, caching and hot-reloading assets is the planned
`voltra-assets` crate and gets its own spec.

## Problem

The editor can build a scene — spawn sprites, move them, colour them, set draw
order, select them — and loses all of it on close. Nothing persists.

Underneath that, a harder problem shapes the design. `World` stores
`HashMap<TypeId, Box<dyn ErasedStorage>>`, and `ErasedStorage` exposes only
`remove_entity` and a downcast. There is no way to enumerate component types or
to serialize a storage without knowing `T` at compile time. Serialization
therefore cannot be a method on `World`.

It also cannot live in `voltra-ecs`. That crate has zero dependencies, on
purpose, and ARCHITECTURE.md records it as a property worth keeping. Adding
`serde` there would end it.

## Prior art

Checked, not recalled.

- **Unity** — `.unity` scenes are YAML. Entities and components carry `fileID`s;
  cross-scene references use a GUID plus fileID. A component whose script is
  missing loads as "Missing (Mono Script)", and **saving then drops its data** —
  a long-standing complaint and the reason people lose work by opening a scene on
  a machine without a package installed.
- **Godot** — `.tscn` is a text format, deliberately diffable and mergeable.
  Nodes are addressed by path within the tree. A node whose script is missing
  keeps its serialized properties rather than shedding them.
- **Bevy** — `DynamicScene` serializes through a type registry keyed by type name
  and writes RON. Components not present in the registry cannot round-trip.

Adopted: Godot's preservation of unrecognised data, Bevy's registry keyed by
name, and RON as the format. Rejected: Unity's drop-on-save, explicitly — it is
the single behaviour in this area that destroys work.

## Design

### 1. Format: RON, authoring-first

Text, meant to be read in a pull request and merged in git. Speed and size are
not goals; a compact runtime format, if it is ever wanted, is a build step over
this one and does not change it.

RON rather than JSON or TOML: it is Rust-native, works with `serde` derives,
expresses nested structs and enums without contortion, and diffs by line. JSON
has no comments and is noisy at depth. TOML handles nested arrays of tables
badly, which is exactly this shape.

```
(
    version: 1,
    entities: [
        (
            id: "018f3a2b-7c41-7000-8000-2b1d4e5f6a70",
            components: {
                "Sprite": (color: (1.0, 1.0, 1.0, 1.0), sort_order: 0),
                "Transform": (translation: (0.0, 0.0), rotation: 0.0, scale: (1.0, 1.0)),
            },
        ),
    ],
)
```

`version` is written from the start. Adding it later means guessing what an
unversioned file meant.

Components live in a `BTreeMap<String, ron::Value>` so the key order is
alphabetical and stable. A `HashMap` would reorder between runs and fill diffs
with noise that is not a change.

`Box<ron::value::RawValue>` is what both known and unknown components are held as
while in memory: a known one becomes a `RawValue` on the way out and its own type
on the way in, and an unknown one stays a `RawValue` throughout. One
representation, so a single code path writes both.

**`RawValue`, not `ron::Value`.** This spec originally said `Value`, and that was
wrong in three ways that only surfaced when the vendored 0.12.2 source was read:

- `Value` has no struct variant, only `Map`. A component would come out as
  `{"rotation": 0.0, "translation": (0.0, 0.0)}` rather than the
  `(translation: (0.0, 0.0), rotation: 0.0)` shown above — braces and quoted
  keys instead of RON struct syntax, which undermines the readable-format
  decision this file opens with.
- `Map` is backed by a `BTreeMap`, so a component's fields would be re-sorted
  alphabetically rather than kept in declaration order.
- `Value`'s deserializer does not support enums at all. Neither component has one
  today, so nothing would break yet — it would break on the first one that did,
  silently until then.

`RawValue` holds the RON text itself. It preserves struct syntax and field order,
carries enums because it never interprets them, and makes preservation of an
unknown component exact rather than approximate. `RawValue::from_rust::<T>` and
`RawValue::into_rust::<T>` are the two conversions, and it derives `Eq`, `Ord`
and `Hash`, which is everything the maps here need.

**One `PrettyConfig`, defined once and used by every save.** Formatting is the
serializer's choice, so the only way two saves of the same world can be identical
is for there to be exactly one configuration, in one place, that nobody passes a
variant of. `struct_names` is off — the example above has no `SceneFile(` prefix
and that is deliberate, since the type name adds nothing a reader of a `.ron`
scene needs.

### 2. Identity: a `SceneId(Uuid)` component

`Entity` is an index and a generation, both recycled by the allocator at runtime.
They are not identity and must never reach a file.

Entities that persist carry `SceneId(Uuid)`. Entities that do not — runtime
effects, transient spawns — simply lack it and are skipped, which means no
exclusion list has to be maintained anywhere.

**UUID v7, not v4.** A v7 carries a timestamp in its high bits, so sorting by id
sorts by creation. Writing entities in `SceneId` order therefore makes the file
deterministic *and* appends new entities at the end rather than scattering them
through the diff. With v4 those two properties are mutually exclusive.

### 3. A registry, not a monolithic serializer

```rust
let mut registry = ComponentRegistry::new();
registry.register::<Transform>("Transform");
registry.register::<Sprite>("Sprite");
```

`register` requires `T: Serialize + DeserializeOwned + 'static` and stores the
name beside a save and a load function specialised for that type. Adding a
serializable component is one line, not an edit to a function that knows about
every component.

The registry is also what makes this possible without touching `voltra-ecs`: it
*is* the list of types, so saving walks the registry and calls the `World::get`
and `World::insert` that already exist. No new ECS API, no change to
`ErasedStorage`.

Registration is explicit rather than automatic. A component that is not
registered is not persisted, which is a property worth having deliberately.

### 4. Unknown components are preserved and reported

Both, not either. What Unity gets wrong is not that it warns — it is that the
warning is all that survives.

On load, a component name the registry does not know is stored, unparsed, in an
`UnknownComponents` component on that entity, and `log::warn!` names it once. On
save, those raw values are merged back into the entity's map alongside the
components that were understood.

So a build that does not know `Physics` can open a scene, move a sprite, save,
and leave `Physics` byte-identical in the file. The user changed what they
understood and did not touch what they did not.

The cost is that the loader cannot parse straight into known types and discard
the rest; it holds the raw `ron::Value` tree alongside. That cost is the feature.

### 5. Placement

`voltra-scene`, in a module directory. Scene components and their on-disk shape
change together, and "saves a scene to a file and reads it back" cannot be
described without naming the scene, so it is not yet its own crate.

```
crates/voltra-scene/src/
  sprite.rs          Sprite            (gains serde derives)
  transform.rs       Transform         (gains serde derives)
  batch.rs           SpriteBatch
  pick.rs            sprite_at
  scene_id.rs        SceneId, UnknownComponents
  format.rs          the on-disk model: SceneFile, EntityRecord
  format/
    registry.rs      ComponentRegistry
    save.rs          world -> SceneFile -> file
    load.rs          file -> SceneFile -> world
    error.rs         SceneError
```

`voltra-ecs` is untouched.

### 6. Editor wiring

`Scene ▸ Save` and `Scene ▸ Open` in the existing menu bar. Both take a `&Path`;
the editor passes a default from a named constant. No path is embedded in the
save and load functions themselves — the second caller is a file dialog, which is
already foreseeable.

A native file dialog needs another dependency (`rfd`) and its own decisions about
threading and modality. Out of scope; the default path stands in until then.

### 7. Dependencies

Three, all leaf libraries, all in the root `[workspace.dependencies]`:

- `serde` with `derive`
- `ron`
- `uuid` with `v7` and `serde`

None of them has an opinion about how a game is structured, so none costs design
freedom — the same test the `glam` decision already applied.

## Errors

`SceneError`, returned by both save and load. No `unwrap` outside tests.

Distinguish, because they call for different responses:

- **I/O failure** — the path does not exist, is unreadable, is unwritable.
- **Malformed file** — RON that does not parse, with the parser's message kept
  rather than flattened.
- **Wrong version** — a `version` this build does not handle, naming both the
  found and the supported value.
- **A component that fails to deserialize** — the name is registered but its data
  does not fit the type. This is *not* the unknown-component case and must not be
  silently preserved: the name says the build understands this component, so data
  it cannot read is a real error and propagates.

That last distinction is the one worth getting right. Unknown means "not mine, do
not touch"; known-but-invalid means "mine, and broken".

## Testing

All headless. No GPU, no window, no editor.

- **Round trip through the world.** Build a world, save, load into a fresh world,
  and compare every component of every entity. `SceneId`s must survive unchanged.
- **Preservation, stated as something achievable.** Byte-identical output against
  an arbitrary input file is *not* a guarantee this design can make: formatting is
  the serializer's choice, so a hand-written file with different indentation will
  never round-trip to itself. Two tests replace it, and together they say what
  actually matters:
  - **The unknown component's data survives.** Load a file containing a component
    the registry does not know, save, load again, and assert the unknown value is
    equal to what went in — compared as a parsed `ron::Value`, not as text.
  - **Saving is idempotent.** Save a world, load it, save again, and assert the
    two outputs are **identical byte for byte**. Any instability — map ordering,
    float formatting, entity order — shows up here as a diff.
- **Unknown components are logged**, once per name.
- **A known component with invalid data is an error**, not a preserved unknown.
- **Entity order is by `SceneId`**, and a newly created entity lands at the end
  of the file rather than in the middle.
- **Component keys are alphabetical**, so two saves of the same world are
  identical.
- **Entities without a `SceneId` are skipped**, and loading does not invent one
  for them.
- **The empty world** saves to a valid file and loads back empty.
- Each error variant: a missing file, malformed RON, an unsupported version.

The editor menu wiring is thin glue and is checked by running the editor.

## Out of scope

- The asset pipeline — `voltra-assets`, its own spec.
- A native file dialog.
- A binary runtime format.
- Prefabs, nested scenes, and cross-scene references. `SceneId` exists so these
  are possible later; none is built now.
- Migration between `version` values. Version 1 is the only version; a second one
  brings the question of what to do with the first, and that decision belongs
  with the change that forces it.
- Undo/redo.
