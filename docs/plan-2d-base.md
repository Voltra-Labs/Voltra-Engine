# Stages 75 onward — a solid 2D base

Working file (CLAUDE.md): one section per stage, deleted as each lands and
its decisions reach ARCHITECTURE.md. Each stage gets its own branch and pull
request; a stage that grows past one review is split (75a, 75b) rather than
merged half-done.

## Where the engine stands

Stages 1–86 and W1–W4 (README roadmap): batched, lit and post-processed
rendering; CPU and GPU particles; rigid bodies of four shapes with joints,
continuous collision, characters and one-way platforms; tilemaps with auto-tiling; audio; prefabs; property
animation with a timeline and state machines; navigation; an editor with
undo, gizmos and a timeline; the player; native and web export; an MCP
server; a profiler.

What a game still cannot do, found by reading the tree (not guessed):


And nothing tells a newcomer how to make a game with it.

## The order, and why

| Stage | Scope | Why here |
| --- | --- | --- |
| 75a | Capsule and convex polygon colliders | Physics is the most structural piece; every later stage that moves things leans on it |
| 75b | Continuous collision for fast bodies | Same subsystem, while it is open |
| 76 | The game as a plugin: the editor plays it | From here on every feature can be tried in the editor with real game code |
| 77 | Hot reload of the game plugin | The iteration loop a scripting language would have bought, in Rust |
| 78 | `cargo xtask new-game`: a project template | Makes 76–77 the default way to start, not an expert path |
| 79 | UI 1: images, 9-slice panels, layout | Menus and HUDs, with 80–82 |
| 80 | UI 2: slider, toggle, text field, scroll | |
| 81 | Text: wrapping and localization | |
| 82 | Tweens and timers | Small; used by everything above |
| 83 | Sprite materials: a game's own shader | Hit flashes, outlines, dissolves |
| 84 | Several cameras; drawing into a texture | Split screen, minimaps, pixel-perfect |
| 85 | Editor: grid and snapping, a console, project settings | Daily comfort building levels |
| 86 | Touch input | Phones and tablets, now that the web build exists |
| 87 | A user guide, sample games, version 0.1 | A base is solid when someone else can use it |

Deliberately left out: a scripting language (games are Rust; 76–78 give Rust
the iteration loop instead — see 76), 3D (CLAUDE.md), networking (a
subsystem of its own).

---

## 87 — A guide, sample games, version 0.1

**Scope.** A user guide in `docs/guide/` (install, first scene, first game
crate, physics, UI, audio, export), every public item documented
(`missing_docs` on the public crates), two more small sample games beside
the platformer, a changelog, and a `v0.1.0` tag.

**Done when** someone following the guide from a clean machine ships a game
to the web — checked by CI running the guide's commands.
