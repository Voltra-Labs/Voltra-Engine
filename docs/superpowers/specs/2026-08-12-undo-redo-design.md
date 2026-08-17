# Undo and redo (stage 14)

## The problem

Three parts of the editor mutate the scene and none of them is reversible:

- the translate gizmo, which writes `Transform::translation` every frame of a drag
- the inspector, which writes `Transform`, `Sprite` and the texture path, and
  despawns the entity through `Delete`
- the menu bar, which spawns sprites and bodies, and empties the scene through
  `Scene ▸ Clear`

The gap is already recorded in the tree. `play.rs` says *"Not undo, and it must
not be described as one: there is no undo stack in this editor"*, and
ARCHITECTURE.md rejects "Stop as an undo entry" for the same reason. Play mode
gave the editor a way back from a **simulation**; it gave it nothing back from
an **edit**.

Every editor stage after this one — rotate and scale gizmos, multi-selection,
reparenting in the hierarchy — adds another site that mutates the world. Each
one added before undo exists is another site to retrofit.

## What the established engines do

| Engine | Model | What one entry holds |
| --- | --- | --- |
| **Unity** | `Undo.RecordObject(obj, name)` before the mutation | The **serialized object**, diffed against its post-change state |
| **Unreal** | `FScopedTransaction` + `UObject::Modify()` | `FTransaction::FObjectRecord` — the **serialized properties** of each marked object |
| **Godot** | `UndoRedo::create_action` + `add_do_property` / `add_undo_property` | **Typed calls**: the do-list and the undo-list, replayed |
| **Bevy** | no editor | — |

Two of the three serialize the affected objects; only Godot builds typed
inverse operations. The split matters here because this engine already has the
serializer the first model needs: `ComponentRegistry` converts any registered
component to and from RON without the caller naming its type, and
`EntityRecord` is already the shape of "one entity and everything it carries".

Taken from them:

- **Unity/Unreal's serialized-record model** over Godot's typed commands. A
  typed `enum Edit { SetTransform{..}, SetSprite{..}, .. }` needs a variant per
  component and a match arm per mutation site; the registry makes both
  unnecessary. Godot's shape is right for an engine whose editor predates a
  generic property serializer, which is not this one.
- **Unreal's scope**: one interaction is one entry. `FScopedTransaction` opens
  on the first change and closes when the interaction ends, which is what makes
  a gizmo drag a single undo step rather than one per frame. Unity reaches the
  same place from the other side with `Undo.CollapseUndoOperations`. Opening a
  scope and holding it open while the interaction continues is the simpler of
  the two and is what this design uses.
- **Unity's rule that loading a scene clears the history.** The stack describes
  entities of the scene that was open; after `Scene ▸ Open` those entities are
  gone and the ids in the stack address nothing.
- **Unreal's rule that PIE is outside the transaction buffer.** Play mode
  already restores a snapshot on Stop, so edits made during play are discarded
  anyway; recording them would put entries in the history that Stop silently
  undid.

Not taken:

- **Godot's `merge_mode`.** Merging is only needed because Godot commits an
  action per property write; an interaction-scoped edit never produces the
  duplicates that merging exists to collapse.
- **Unreal's non-persistent transaction buffer as a stated limitation.** It is
  a limitation there because the buffer holds raw property offsets. Ours holds
  RON, which would survive being written to disk — but nothing asks for that,
  so the history stays in memory and the design does not pretend otherwise.

## Decisions

### 1. An entry is a set of per-entity before/after records

```rust
struct Edit {
    label: &'static str,
    entities: Vec<EntityChange>,
    selected_before: Option<SceneId>,
    selected_after: Option<SceneId>,
}

struct EntityChange {
    id: SceneId,
    /// `None` when the entity did not exist on that side of the edit.
    before: Option<EntityRecord>,
    after: Option<EntityRecord>,
}
```

`Option` on both sides is what makes one type cover every action:

| Action | `before` | `after` |
| --- | --- | --- |
| Move, edit a field | `Some` | `Some` |
| Spawn | `None` | `Some` |
| Delete | `Some` | `None` |
| `Scene ▸ Clear` | `Some` × N | `None` × N |

Applying an edit in either direction is then one function over one side, and
there is no per-action code anywhere.

Memory is proportional to what was **touched**, not to the scene — which is the
reason this is not simply "a `SceneFile` per step" the way the play-mode
snapshot is. A 500-entity scene with a 128-deep history of single-sprite moves
holds 128 single-entity records, not 64 000 entity records.

**Rejected: whole-scene snapshots.** Trivially correct and already written, but
memory is O(scene × depth) and every undo despawns and respawns the entire
scene, which invalidates every `Entity` handle, re-resolves every texture and
re-resolves the selection on every keystroke of `Ctrl+Z`.

**Rejected: typed commands with inverses (Godot).** A variant per component and
a case per call site, in a codebase whose registry exists precisely so that
serialization does not need either.

### 2. Identity is `SceneId`, never `Entity`

An undo that revives a deleted entity must revive it *as the same entity*: its
scene identity has to survive, because a later entry in the stack refers to it.
`Entity` is an index and a generation, both recycled by the allocator, and a
respawn produces a different one. The scene format already made this call —
"`Entity` … must never reach a file" — and the history is the same problem
without the file.

Consequence, stated because it is load-bearing: **entities without a `SceneId`
are outside undo entirely**, never captured and never restored. That is the
same predicate `save`, `Scene ▸ Clear` and the play snapshot already use, so
transient runtime spawns opt out by doing nothing.

### 3. The `before` side is the frame, not the interaction

Unreal opens its scope *before* the mutation because retained-mode UI can:
`Modify()` is called by the code that is about to write. An immediate-mode
panel cannot. By the time a `DragValue` reports `dragged()`, it has already
written the new value into the component — egui only reports a drag once the
pointer has passed the drag threshold, so a `before` captured at that point is
already several pixels of movement late, and undo would restore to a position
the user never left the sprite in.

So the baseline is taken **at the top of the frame**, before any panel runs:

- `watch(view, ids)` records what the ids looked like when the frame started.
  In practice `ids` is the selection, which is the only entity the gizmo or the
  inspector can reach.
- `claim(label)` is how a panel says "the interaction that is changing things
  is still going, and this is what to call it". No capture, no ids — a string.
- `end_frame(view)` compares the watched ids against the baseline. If anything
  changed, the change is folded into the open edit — opening one, with the
  baseline as its `before`, if there is not one already. The edit is then
  committed **unless a claim this frame says the interaction is still live**.

A gizmo drag is one entry because the viewport claims `"Move"` on every frame
the drag holds an entity, and the entry closes on the frame the claim stops
arriving — the release frame. A change with no claim at all (a checkbox, a
committed text field) opens and closes inside one frame, which is correct: it
*was* one interaction.

**Rejected: `begin(key)` / `commit()` around each interaction.** The obvious
port of `FScopedTransaction`, and it is what this design started as. It needs
every call site to detect the *start* of an interaction one frame before the
widget admits to it, which egui does not offer, and it leaves an `EditKey` enum
whose only remaining job is to notice that two interactions overlapped —
something one pointer cannot do.

The cost of the watcher is serializing one entity per frame while something is
selected. That is a few hundred bytes of RON against a frame that already
rebuilds every sprite vertex.

**Actions the watcher cannot see** — spawn, `Delete`, `Scene ▸ Clear` — touch
ids that are not the selection, or that do not exist yet. They record
themselves explicitly with `begin(label, view, ids)` and
`commit_including(view, ids)`, where `commit_including` takes the ids
discovered while the action ran: anything absent from the `before` map gets
`before: None`, which is what makes a spawn a record. An explicit edit marks
the frame as recorded so the watcher does not also log the same change.

**Undo and redo mark the frame recorded too.** They change the world after
`watch` ran, and without the flag `end_frame` would notice and push the undo
itself onto the stack.

### 4. Applying an edit makes each entity *equal* to its record

`apply_record` is not "insert these components". It makes the entity match the
record exactly:

1. No entity with that `SceneId` → spawn one and insert the id.
2. For every component in the record: registered → deserialize and insert;
   unregistered → keep verbatim in `UnknownComponents`, exactly as loading a
   file does.
3. For every **registered name absent from the record** → remove that component
   from the entity.
4. `UnknownComponents` is removed when the record carries none.

Step 3 is why `ComponentRegistry` gains a `remove` function beside `save` and
`load`. Nothing in today's UI removes a component, but "undo an add-component"
is the same code path as "undo an edit" and costs one closure in `register`;
without it, undoing an add would leave the component behind and the undo would
silently be a partial one.

Records are captured by the same code that writes a scene file: `to_scene_file`
becomes a caller of `record_entity`, and `apply_record` and `spawn_entities`
share the component loop that reads a record's values back. There is no second
serialization path to keep in step with the first.

`spawn_entities` deliberately does **not** become a caller of `apply_record`.
Loading always spawns; `apply_record` reuses an entity that already carries the
id. `Scene ▸ Open` loads the new scene *before* despawning the old one, so an
id present in both would land on the old entity and then be despawned with it —
a file's entity silently disappearing on open.

### 5. Undo is an editing operation; play blocks it

`Ctrl+Z` and `Ctrl+Y` do nothing outside `PlayState::Editing`, and the history
survives a `Play → Stop` round trip untouched — Stop restores the pre-play
snapshot, so every entry in the stack still describes the scene it is looking
at.

**Rejected: clearing the history on Play.** More conservative and strictly
worse: it costs the user their entire history for pressing a button that, by
design, changes nothing permanent.

**Rejected: undo during play.** It would operate on the simulated world and be
thrown away by the next Stop, so the entry would describe something that did
not survive.

### 6. `Scene ▸ Open` clears the history; `Clear` is one entry

Open replaces the scene, and every `SceneId` in the stack belongs to the scene
being closed. Unity's answer, and the only coherent one.

`Scene ▸ Clear` stays undoable as a single entry holding every despawned
entity's record — it is a delete of everything, and a delete is undoable.

`Scene ▸ Save` does not touch the history.

### 7. After any apply: cancel the drag, reset physics, re-resolve textures

The same three consequences play-mode Stop already handles, for the same
reasons, and they are unconditional rather than conditional on the edit having
spawned anything:

- **Cancel any gizmo drag.** A `Drag` holds an `Entity` and a grab offset; an
  undo that respawns the entity leaves both stale.
- **Reset physics.** The solver's contact cache is keyed by `Entity`, and a
  respawned entity is a different key. Nothing is simulating in `Editing`
  anyway, so the cost is a warm-start that was not being used.
- **Re-resolve sprite textures.** `Sprite::texture_handle` is
  `#[serde(skip)]` — a record carries the path, not the handle. Without this an
  undone texture edit draws flat white.

### 8. A capture that fails clears the history

`record_entity` returns a `Result`: a component whose `Serialize` fails is a
real, if unlikely, outcome. Dropping just the failed entry would leave a stack
that lies — `Ctrl+Z` after it would restore a state from *before* the
unrecorded action and silently discard that action's work.

So a failed capture logs an error and clears both stacks. The user loses undo,
which is visible and recoverable; they do not lose work to a history that
misrepresents itself.

A failed **apply** cannot happen in practice — the RON was produced this
session, this build, this registry — and is logged loudly rather than swallowed,
matching `Play::stop`.

### 9. Depth: 128 entries, oldest dropped

Blender ships 32 global steps, Unity and Godot are effectively unbounded. 128 is
chosen against this design's per-entry cost, which is the touched entities
rather than the scene: the common entry is one entity's RON, a few hundred
bytes. A constant with the reasoning beside it, not a hidden literal — a
preferences panel is the second caller and it is foreseeable.

## Structure

```
crates/voltra-scene/src/format/record.rs   # record_entity, apply_record, entity_with_id
crates/voltra-scene/src/format/registry.rs # + a remove fn per registered type
crates/voltra-editor/src/undo.rs           # module doc, re-exports, UndoHost
crates/voltra-editor/src/undo/edit.rs      # Edit, EntityChange, Side, apply
crates/voltra-editor/src/undo/history.rs   # the two stacks, the open edit, the cap
crates/voltra-editor/src/undo/shortcut.rs  # Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z, scoped
```

`record_entity` and `apply_record` go in `voltra-scene` rather than the editor:
they are the scene format's own operations at entity granularity, `save_one` and
`load_one` are `pub(crate)` and should stay that way, and the file format is the
only place that should know what "an entity equals a record" means.

The history itself stays in `voltra-editor`, not a `voltra-undo` crate. Undo is
an editor concept in exactly the way play mode is: a shipped game has neither.
It moves to its own crate when a second binary needs it.

`UndoHost` is a narrow trait — `world`, `reset_physics`, `resolve_sprite_textures` —
implemented for `UiFrame`, mirroring `PlayHost` and for the same two reasons: a
`UiFrame` cannot exist without a `wgpu::Device`, so the trait is what makes the
transitions testable at all, and it names exactly what undo is allowed to touch.
Deliberately **not** shared with `PlayHost`, whose `set_simulating` and
`request_steps` undo has no business calling.

## Integration points

| Site | Call | Label | Closes |
| --- | --- | --- | --- |
| `editor.rs`, frame top | `watch([selected id])` | — | — |
| `viewport.rs`, while the gizmo holds an entity | `claim` | `"Move"` | the release frame, at `end_frame` |
| `inspector.rs`, while a field is dragged or focused | `claim` | per field: `"Move"`, `"Rotate"`, `"Scale"`, `"Set colour"`, `"Set sort order"` | at `end_frame` |
| `inspector.rs` texture commit | `claim` | `"Set texture"` | the same frame, at `end_frame` |
| `inspector.rs` `Delete` | `begin` + `commit` | `"Delete"` | immediately |
| `menu_bar.rs` spawners | `begin` + `commit_including([new id])` | `"Spawn sprite"` / `"Spawn body"` / `"Spawn floor"` | immediately |
| `menu_bar.rs` `Clear` | `begin` + `commit` | `"Clear scene"` | immediately |
| `menu_bar.rs` `Open` | `clear` | — | — |
| `editor.rs`, frame end | `end_frame` | — | — |

Two new UI surfaces: an **Edit menu** with `Undo <label>` / `Redo <label>`,
greyed when the stack is empty and while not editing — Unity, Unreal and Godot
all show the label, and it is the only discoverable place the shortcuts are
written down — and the **shortcuts** themselves, polled once per frame in
`Editor::ui` before the panels.

Shortcut scoping follows the rule the viewport's `W` key already set: ignored
while `egui_wants_keyboard_input()`, so `Ctrl+Z` inside the texture `TextEdit` is
egui's own text undo and not the scene's.

## Testing

Pure logic throughout, so unit tests in the file under test, per CONVENTIONS.md.

**`voltra-scene/src/format/record.rs`**
- a record round-trips an entity's components
- applying a record to a world without that `SceneId` spawns the entity
- applying a record removes a registered component the record does not carry
- unknown components survive a record round trip
- `UnknownComponents` is dropped when the record carries none

**`voltra-editor/src/undo/edit.rs`**
- the four `before`/`after` shapes each apply in both directions
- an edit whose sides are equal is recognised as a no-op

**`voltra-editor/src/undo/history.rs`**, against a `FakeHost` in the shape
`play.rs` already uses
- undo then redo returns the world to where it was
- a new edit clears the redo stack
- a change with a claim on every frame produces **one** entry, not one per frame
- a change with no claim commits inside the frame it happened
- `end_frame` with nothing changed pushes nothing
- an explicit `begin`/`commit` stops the watcher recording the same change twice
- an undo does not itself become an entry
- the cap drops the oldest entry and keeps the newest
- a failed capture clears both stacks
- undo and redo are refused outside `Editing`
- undo of a delete restores the entity **and** the selection

The gizmo and the panels are verified by driving the real editor, as
[CLAUDE.md](../../../CLAUDE.md) requires: launch detached, move a sprite,
`Ctrl+Z`, check the log, kill it.

## Out of scope

- **Persisting the history to disk.** Nothing asks for it.
- **A visible history panel.** The Edit menu's labels are the whole UI.
- **Undo for camera movement, tool changes and the selection on its own.**
  None of the three engines records viewport navigation, and for the same
  reason: it is not a change to the scene.
- **A dirty flag / unsaved-changes title.** Related, genuinely useful, and its
  own stage — it needs a save-point marker in the history, which this design
  makes possible and does not implement.
- **Multi-selection, reparenting, rotate and scale gizmos.** They are new call
  sites for an API this stage defines; they do not change it.
