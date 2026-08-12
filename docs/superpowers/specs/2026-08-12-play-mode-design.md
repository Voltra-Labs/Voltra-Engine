# Stage 13 — Play mode

**Date:** 2026-08-12
**Status:** design

## Scope

The editor gains an edit state and a play state. Simulation runs only in play,
and stopping puts the scene back exactly as it was when play began.

Today `voltra-editor/src/main.rs` calls `.with_physics()` unconditionally, with
the comment *"there is no play mode yet to put this behind"*. That is the whole
problem: a box placed with the gizmo starts falling while it is being placed,
and nothing can undo it. Authoring and simulating are the same mode, so neither
works.

In:

- `PlayState`: `Editing`, `Playing`, `Paused`, and the transitions between them.
- A scene snapshot taken on Play and restored on Stop, through the existing
  `SceneFile` round trip.
- Play / Pause / Step / Stop, on their own toolbar above the viewport.
- A runtime simulation switch in `voltra-core`, replacing the build-time
  `with_physics()` opt-in as the thing that decides whether a frame steps.
- Editing while playing, discarded on Stop, with the viewport tinted so the
  discard is never a surprise.
- Selection surviving the restore, matched by `SceneId`.

Out, and stated so their absence is not read as a bug:

- **Keep Simulation Changes.** Unreal's opt-in for promoting play-time edits
  back into the edit scene. It needs a per-entity diff and a UI to resolve it;
  it is a stage of its own and this one must not pretend to have it.
- **Undo.** There is no undo stack in the editor at all. Stop is not undo and
  must not be described as one: it restores the snapshot, nothing else.
- **A separate game process.** Godot's answer, and the right one once a runtime
  binary exists. There is no runtime binary; there is one process.
- **Scripting, game input, or anything else "play" will eventually gate.**
  Physics is the only simulation there is. The switch is written so a second
  subsystem joins it without a second concept.
- **Time scale.** A 0.25x / 2x multiplier interacts with the fixed clock's debt
  and belongs with whatever stage needs slow motion.
- **Play from a saved file.** Play simulates the world in memory, not the last
  save. A scene never saved still plays.

## What is already here

- `voltra_scene::format::to_scene_file(world, registry) -> Result<SceneFile>`
  and `from_scene_file(&SceneFile, registry, world) -> Result<()>`. Both work
  in memory; `save` and `load` are thin wrappers that add a file. The load is
  all-or-nothing: a failure despawns everything it spawned.
- `SceneId(pub Uuid)` is what makes an entity part of the scene. `to_scene_file`
  writes exactly the entities that carry one, and `Scene ▸ Clear` despawns
  exactly those. The snapshot inherits that rule for free.
- `UnknownComponents` preserves components this build does not know, so a round
  trip is not lossy for a scene authored by a newer build.
- `PhysicsWorld::reset()` forgets every accumulated impulse and contact — added
  for scene loads, which is what a restore is.
- `App` owns `physics: bool`, set once by `with_physics()`, read by
  `step_physics` each frame. There is no runtime setter.
- `UiFrame` is the editor's only reach into `App`, rebuilt each frame.
- `Editor` owns `selected`, `camera`, `tool`, `gizmo`, `show_colliders`.

## The state machine

```rust
/// Whether the editor is authoring the scene or simulating it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    #[default]
    Editing,
    Playing,
    Paused,
}
```

| From | Action | To | What happens |
| --- | --- | --- | --- |
| `Editing` | Play | `Playing` | Snapshot taken, then simulation on |
| `Playing` | Pause | `Paused` | Simulation off; the scene stays where it is |
| `Paused` | Play | `Playing` | Simulation on. No new snapshot |
| `Paused` | Step | `Paused` | Exactly one fixed step runs |
| `Playing`/`Paused` | Stop | `Editing` | Snapshot restored, simulation off |
| `Editing` | Step | `Editing` | Nothing. Step outside play is not a stepper |

Play from `Editing` is the only transition that takes a snapshot, and Stop is
the only one that consumes it. Pause is a switch on simulation, not on state
that has to be saved — which is why Play from `Paused` must *not* re-snapshot:
that would silently make the paused mid-air scene the thing Stop restores.

Illegal transitions are unrepresentable in the UI (the toolbar shows Play or
Pause, never both) and are no-ops in the state machine, not errors. A no-op
keeps the buttons from needing to know the state twice.

## The snapshot

```rust
/// The scene as it was when play began, and what Stop puts back.
pub struct PlaySnapshot {
    scene: SceneFile,
    /// Which entity was selected, by scene identity rather than by `Entity` —
    /// the restore despawns and respawns, so every handle is stale after it.
    selected: Option<SceneId>,
}
```

Taken with `to_scene_file(world, &registry)` at the moment Play is pressed.

**A snapshot that cannot be taken refuses the transition.** `to_scene_file`
returns `Result`, and the failure is real: a component whose `Serialize` fails.
If it errors, the editor logs it and stays in `Editing`. Entering play without
a way back is the one outcome this stage exists to prevent, so the error path
is the design, not an afterthought.

The registry is `ComponentRegistry::with_defaults()`, and the `Editor` owns one
rather than building it per call as `menu_bar` does today. Snapshot and restore
must use the same registry as each other; sharing the field is how that is
guaranteed rather than remembered. `Editor` keeps its `#[derive(Default)]` —
`ComponentRegistry`'s own `Default` is `with_defaults()`, not an empty registry,
so the derive cannot quietly produce one that persists nothing.

### What the snapshot does and does not hold

- **Entities with a `SceneId`** — held, restored, and any spawned during play
  are despawned by the restore.
- **Entities without a `SceneId`** — not held and not touched. They are
  transient by the same definition `Clear` already uses.
- **Unknown components** — held, through `UnknownComponents`.
- **Editor state** (camera, tool, `show_colliders`) — not scene state. The
  camera keeps its position across Stop, as Unity's scene view does.
- **Physics state** (impulses, contacts, clock debt) — not held. Reset instead,
  below.

## Stop: the restore

In order, and the order is load-bearing:

1. **Simulation off.** Before anything is despawned, so no step can run against
   a half-restored world.
2. **Despawn** every entity carrying a `SceneId`.
3. **`from_scene_file`** the snapshot back in.
4. **`PhysicsWorld::reset()`**, which must also clear the fixed clock's
   accumulator — see below.
5. **Re-resolve the selection**: find the entity whose `SceneId` matches
   `snapshot.selected`. An entity selected during play that did not exist at
   snapshot time leaves the selection empty.
6. **Cancel any gizmo drag.** `Gizmo::drag` holds an `Entity` and a grab offset;
   both are stale the moment step 2 runs.

Step 3 cannot fail in practice — the file was produced in this process, by this
build, with this registry, moments ago, so neither the version check nor an
unknown component can trip. It is still a `Result`, and the failure is logged
loudly rather than swallowed: at that point the scene is gone and the user needs
to be told, not left looking at an empty viewport wondering.

### The clock debt

`PhysicsWorld::reset()` clears impulses and contacts but not `PhysicsClock`'s
accumulator, which can hold up to one step of banked time. Stop leaves that
behind today, so the next Play would begin by running a step of the *previous*
session's owed time against the restored scene. `PhysicsClock` gains a `reset`
that zeroes the accumulator, and `PhysicsWorld::reset` calls it. The step length
and the cap are configuration and stay.

## The core switch

The editor cannot step physics itself: `App::update` runs before the UI closure
every frame, and `PhysicsWorld` is `App`'s. So `voltra-core` gains a runtime
switch, and — this is the layering point — **it does not gain the word "play"**.
Play, pause and stop are editor concepts; a shipped game has one mode. What core
gains is the ability to be told whether to simulate this frame.

```rust
impl App {
    /// Whether each frame runs the physics steps it owes.
    pub fn set_simulating(&mut self, simulating: bool);
    pub fn is_simulating(&self) -> bool;

    /// Run `count` fixed steps on the next frame regardless of the switch.
    ///
    /// What a paused editor's Step button asks for. Additive, so two presses
    /// in one frame run two steps rather than one.
    pub fn request_steps(&mut self, count: u32);
}
```

`with_physics()` keeps its name and becomes the *initial* value of the switch.
`voltra-editor` stops calling it: the editor starts in `Editing`, which is the
correct default for an authoring tool and the reason this stage exists.

`UiFrame` mirrors the three, since it is the editor's only reach into `App`:
`simulating()`, `set_simulating(bool)`, `request_steps(u32)`, backed by
`&'a mut bool` and `&'a mut u32` fields.

`App::step_physics` becomes:

```rust
fn step_physics(&mut self, delta: f32) {
    // Requested steps run first and unconditionally: a Step press while paused
    // must advance the world by exactly one step, and `advance` would run zero.
    for _ in 0..std::mem::take(&mut self.pending_steps) {
        self.physics_world.step_once(&mut self.world, self.gravity);
    }

    if self.simulating {
        self.physics_world.advance(&mut self.world, self.gravity, delta);
    }
}
```

`PhysicsWorld::step_once` is new and is what `advance` already does inside its
loop — one fixed step, no accumulator, no cap. `advance` is rewritten in terms
of it so there is one place a step happens.

**One frame of latency, by construction.** The UI runs after `update`, so a Play
pressed this frame first steps on the next. At 60 Hz nobody can see it, and the
alternative — the UI reaching back into the frame that already ran — is worse.
Say it in the doc comment so the next reader does not "fix" it.

## The toolbar

A dedicated panel between the menu bar and the viewport, buttons centred, which
is where Unity and Unreal both put it. The state of the window is the most
important thing on screen while it is not `Editing`, and a menu that has to be
opened to see it is not a state indicator.

```
┌──────────────────────────────────────────────┐
│ Scene   Physics            viewport 1280x720 │  menu bar
├──────────────────────────────────────────────┤
│                ▶   ⏭   ⏹                     │  toolbar (new)
├──────────────────────────────────────────────┤
```

Three buttons, not four: the first is Play or Pause depending on the state.

- **Play/Pause** is one button that swaps its glyph and its action with the
  state, as every transport control does.
- **Step** is enabled only in `Paused`. Enabled-but-inert buttons teach the
  wrong thing about what the state means.
- **Stop** is disabled in `Editing`.
- Every button carries a tooltip naming what it does to the scene — Stop's says
  it discards play-time changes, because that is the destructive one.

`panels/toolbar.rs` is a new file and `panels.rs` declares it. It is layout and
dispatch only: it reads `editor.play` and calls the transitions, exactly as
`menu_bar.rs` reads and calls.

### The tint

While not `Editing`, the viewport image is drawn tinted (Unity's play-mode
tint, and the fix for the oldest complaint in that editor: work lost because
play mode was not obvious). Applied in `panels/viewport.rs` through egui's image
tint — a UI-level tint, not a scene-level one, so nothing in the render path or
the scene knows about play mode.

## Editing while playing

Allowed. The inspector, the gizmo, the hierarchy and both spawners keep working
in `Playing` and `Paused`, and everything they do is discarded by Stop. That is
Unity's and Unreal PIE's behaviour, and it is the behaviour that makes play mode
useful: dragging a body while gravity acts on it is how a scene gets tuned.

Two consequences to write down rather than discover:

- A dragged transform is fought by the solver, which is authored behaviour, not
  a bug. The gizmo sets the transform; the next step reads it.
- `Scene ▸ Open`, `Scene ▸ Clear` and `Scene ▸ Save` in play mode would each
  mean something incoherent — Open discards the snapshot's world, Save writes a
  mid-flight scene as if it were the authored one. **All three stop play
  first**, which is Unity's answer (opening a scene exits play mode) and cheaper
  than three special cases. The stop is silent for Open and Clear, which are
  destructive anyway; Save stops first so what lands on disk is the authored
  scene, and logs that it did.

## Files

| File | Change |
| --- | --- |
| `crates/voltra-editor/src/play.rs` | New. `PlayState`, `PlaySnapshot`, and the transitions as methods on the state |
| `crates/voltra-editor/src/panels/toolbar.rs` | New. The transport panel |
| `crates/voltra-editor/src/editor.rs` | `play: PlayState`, `snapshot: Option<PlaySnapshot>`, `registry: ComponentRegistry`; `ui` calls the toolbar between menu bar and viewport |
| `crates/voltra-editor/src/panels/viewport.rs` | Tint while not `Editing` |
| `crates/voltra-editor/src/panels/menu_bar.rs` | Open / Clear / Save stop play first; use the editor's registry |
| `crates/voltra-editor/src/main.rs` | Drops `.with_physics()` |
| `crates/voltra-core/src/app.rs` | `set_simulating`, `is_simulating`, `request_steps`, `pending_steps`, the rewritten `step_physics` |
| `crates/voltra-core/src/app/ui_frame.rs` | The three accessors |
| `crates/voltra-physics/src/world.rs` | `step_once`; `reset` also resets the clock |
| `crates/voltra-physics/src/clock.rs` | `reset` |
| `docs/ARCHITECTURE.md` | Decisions: the snapshot mechanism, and why core has a switch rather than a mode |
| `README.md` | Roadmap row, and the controls table gains the transport |

`play.rs` splits into `play.rs` + `play/` the moment it grows a second concept
— the likely one being the diff a future Keep Changes needs.

## Tests

Headless, no GPU, in `voltra-editor` unless noted. The state machine and the
snapshot are pure logic over a `World`, which is the whole reason they live
apart from the panels that call them.

**The state machine**

- `play_from_editing_takes_a_snapshot`
- `play_from_paused_does_not_replace_the_snapshot` — the regression that would
  make Stop restore a mid-air scene.
- `step_in_editing_does_nothing`
- `stop_in_editing_does_nothing`
- `a_failed_snapshot_leaves_the_editor_editing` — a world whose serialisation
  fails must not enter play.

**Snapshot and restore**

- `stop_restores_a_moved_transform`
- `stop_despawns_what_play_spawned`
- `stop_respawns_what_play_despawned`
- `an_entity_without_a_scene_id_is_left_alone` — both directions: not captured,
  not despawned.
- `unknown_components_survive_the_round_trip`
- `the_selection_survives_by_scene_id`
- `a_selection_made_during_play_is_cleared_by_stop`
- `stop_forgets_the_contact_impulses` — `cached_pairs() == 0` after.

**The core switch** (in `voltra-core`)

- `a_frame_does_not_step_while_simulation_is_off`
- `a_requested_step_runs_while_simulation_is_off`
- `two_requested_steps_run_two_steps`
- `requested_steps_are_consumed_once`

**The clock** (in `voltra-physics`)

- `a_reset_clock_owes_nothing` — debt banked before the reset does not run
  after it.
- `step_once_advances_exactly_one_step`

## Rejected

- **Duplicating the `World` (Unreal's PIE).** The most faithful answer: play
  simulates a copy and the editor's world is immutable by construction, so
  restore is dropping the copy and nothing can be lost in a round trip. It needs
  a type-erased clone in `voltra-ecs` — every component type registering a
  clone fn — and a second world for the renderer and every panel to be told
  about. The RON round trip reuses a format that is already the definition of
  what a scene is, and is already tested both ways. Revisit if snapshot cost
  ever shows up in a frame: the scenes here are tens of entities.
- **Snapshotting to a temporary file.** Puts disk errors and a temp path in the
  one button that must not fail, to save an in-memory `SceneFile` that already
  exists.
- **A read-only inspector during play.** Less code, and it removes the reason
  to have play mode in an editor at all rather than in a separate binary.
- **Unreal's Keep Simulation Changes now.** Needs a per-entity diff and a UI to
  resolve it. A later stage, and the file layout leaves room for it.
- **Godot's separate game process.** The correct answer once there is a runtime
  binary to launch, and it also survives a crash in game code. There is no
  runtime binary, and a crash here is a crash of one process either way.
- **`PlayState` in `voltra-core`.** Would put an editor concept in the platform
  layer, which a shipped game would carry forever. Core gets a boolean and a
  step request; the editor gets the mode.
- **Stop as an undo entry.** There is no undo stack, and pretending Stop is one
  sets an expectation the next stage would have to break.
- **Re-snapshotting on Pause.** Makes Pause destructive and Stop unpredictable.
- **Keying the restored selection by `Entity`.** The handles are all stale after
  a despawn-and-respawn; `SceneId` is what identity means here.
