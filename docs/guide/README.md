# The Voltra guide

From a clean machine to a game in a browser. Read in order the first time;
each chapter stands alone after that.

1. [Install](01-install.md) — the toolchain, the system packages, the editor.
2. [A first scene](02-first-scene.md) — sprites, the inspector, saving, playing;
   editing many at once, search, prefabs and variants, atlases, tilemaps,
   cameras that follow, frame a group, shake and turn.
3. [A first game](03-first-game.md) — a game crate: components, turns, hot reload.
4. [Physics](04-physics.md) — bodies, colliders, contacts, characters that push
   and climb, tile shapes, joints that spring and break.
5. [UI](05-ui.md) — the HUD, controls, the window, an options screen, text in
   other languages, rich text, touch.
6. [Audio](06-audio.md) — sources, one-shots, buses, streamed music, effects.
7. [Export](07-export.md) — a native build and a web build.
8. [Networking](08-networking.md) — sessions, replication, interest, prediction,
   lag compensation, a lobby, LAN discovery, the editor's network play.
9. [Effects and animation](09-effects-and-animation.md) — particles, lines and
   trails, state machines and crossfades, blend trees, layers, recording keys,
   bones, skinned sprites.
10. [Big worlds](10-big-worlds.md) — regions streamed around the player, loading screens.
11. [3D](11-3d.md) — begun in 0.5: the 3D world's coordinates, `Transform3D`,
    parenting in two dimensions.

Three sample games sit beside the engine in `games/`: `platformer`
(a walker, coins, crates to push, a goal), `breakout` (physics and
collision events) and `dodge` (timers, tweens and sensors), and one
networked: `arena` (two to four players over UDP or WebSockets, a lobby
first, shots tested where the shooter saw them). Each is a crate this guide's third
chapter makes from scratch, grown.

The commands in a shell block under a `<!-- ci -->` line are run by CI on
every change (`cargo xtask guide`), in order: if this guide stops working,
the build says so.

Working on the engine itself rather than a game? `docs/CONVENTIONS.md` has
the rules, the checks to run before a commit, and the benchmarks.
