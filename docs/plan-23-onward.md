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
mistaken for oversights: 3D — which per CLAUDE.md
is a new subsystem with its own design, not a parameter added to any of the
above — and an MCP server, below.

### An MCP server: the editor driven by an AI

**When:** once 2D is finished or close to it. Not before: every subsystem
added after it would need its tools written twice, and a tool surface that
moves under its users is worse than none.

**What:** an MCP server through which an AI client can do anything a person
can in the editor — and a little more, since it can read what it did:

- Scenes: open, save, new; list, spawn, delete, reparent and rename
  entities; read and write any component by its registered name (the
  `ComponentRegistry` already knows every one and how it serialises).
- Assets: list the asset root, import, author atlases, prefabs, tilemaps
  (paint cells, terrains), input maps, sorting layers, audio buses.
- Play mode: start, step, stop; read the world mid-play; send input actions.
- Seeing the result: a screenshot of the viewport or the game view, the log,
  the profiler numbers — so the AI checks its own work instead of guessing.
- Project: build, run the tests, `cargo xtask dist`.

**Shape to decide then, not now:**

- Every edit goes through the same command path the inspector uses, so it
  lands in undo history as one entry per tool call and an AI edit can be
  undone like a person's.
- Where the server lives: most likely a crate of its own (`voltra-mcp`)
  talking to a running editor over a local socket, so the editor binary
  stays free of the protocol and a headless mode (no window, the same
  world) can serve CI. Look at how Godot's and Unity's MCP bridges split
  editor plugin from server before choosing.
- Scope and safety: local only by default; file writes confined to the
  project; destructive tools (delete, overwrite, dist) say so in their
  descriptions.

No crate, no stubs until then (CLAUDE.md: new crates only when there is
code for them).
