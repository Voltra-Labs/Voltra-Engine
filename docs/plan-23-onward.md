# Stages 23 onward — the road after sprite sheets

Working file: a sketch per stage, not a plan to execute. Each one gets its own
`plan-NN-*.md` with files, commits and tests when it is next. Delete a section
once its stage has landed and its decisions are in ARCHITECTURE.md.

## Where the engine stands after 22

Runs a game: ECS, scenes on disk, sprites with sheets and animation, a 2D
camera, rigid bodies with filters, sensors, events and queries, two game ticks,
an editor with hierarchy/inspector/gizmos/undo/play/asset browser, and a player
binary, and since 23 it can make a sound — mixed through buses, faded, limited,
reloaded and auditioned from the inspector — and since 24a it can stamp out
copies of a prefab from a tick, and since 24b author one in the editor and
drop it into a scene, and since 25 draw, paint and stand on tilemap layers.
What it still cannot do: print a number, read a gamepad, or be handed to
someone who does not have Rust installed.

## The order, and why

| Stage | Scope | Why here |
| --- | --- | --- |
| ~~23a~~ | ~~Audio: the crate, the store, the components, one-shots~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| ~~23b~~ | ~~Audio: buses, tweens, editor audition, hot reload~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| ~~24a~~ | ~~Prefabs: spawn, ids, parent remap, tick API~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| ~~24b~~ | ~~Prefabs: editor authoring and the instance link~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| ~~25a~~ | ~~Tilemaps: data, format, mesh~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| ~~25b~~ | ~~Tilemaps: collision and the paint tool~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| ~~26a~~ | ~~In-game UI: fonts, glyph atlas, `Text` in the world~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| ~~26b~~ | ~~In-game UI: screen-space pass, `UiRect`, a HUD~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| ~~27a~~ | ~~Input actions: the map, evaluation, rebinding~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| ~~27b~~ | ~~Gamepads through `gilrs`, analog bindings and dead zones~~ | **Landed.** Decisions in ARCHITECTURE.md. |
| 28 | Sleeping bodies and a real broad phase | `broad.rs` already names its own limit at ~200 bodies. A tilemap's generated colliders are what will cross it. |
| 29 | Export: `xtask`, and a build manifest | Turning the tree into a folder someone else can run. Last, because it packages whatever exists. |

Defensible reorderings, if the goal changes: aiming at **a playable demo**
soonest is 23 → 24 → 26 → 29 and tiles later; aiming at **content tooling** is
25 first and audio later. What should not move is 24 before 25/26 and 28 after
whatever generates the bodies.

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
mistaken for oversights: particles (needs 22, wants 24), the tilemap's
per-chunk mesh cache, camera culling, auto-tiling and animated tiles (after
25; see its decisions), sorting layers and
parallax, a render graph and post-processing, scene transitions and additive
loading in the player, save games, a job system, and 3D — which per CLAUDE.md
is a new subsystem with its own design, not a parameter added to any of the
above.
