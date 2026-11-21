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
| ~~28~~ | ~~Sleeping bodies and a real broad phase~~ | **Landed.** Decisions and numbers in ARCHITECTURE.md. |
| ~~29~~ | ~~Export: `xtask`, and a build manifest~~ | **Landed.** Decisions in ARCHITECTURE.md. |

Defensible reorderings, if the goal changes: aiming at **a playable demo**
soonest is 23 → 24 → 26 → 29 and tiles later; aiming at **content tooling** is
25 first and audio later. What should not move is 24 before 25/26 and 28 after
whatever generates the bodies.

---

## Cross-cutting, unscheduled

Things that will want a stage of their own eventually, listed so they are not
mistaken for oversights: multi-pass effects (bloom, blur) on a real render graph, and
3D — which per CLAUDE.md
is a new subsystem with its own design, not a parameter added to any of the
above.
