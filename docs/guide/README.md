# The Voltra guide

From a clean machine to a game in a browser. Read in order the first time;
each chapter stands alone after that.

1. [Install](01-install.md) — the toolchain, the system packages, the editor.
2. [A first scene](02-first-scene.md) — sprites, the inspector, saving, playing.
3. [A first game](03-first-game.md) — a game crate: components, turns, hot reload.
4. [Physics](04-physics.md) — bodies, colliders, contacts, characters.
5. [UI](05-ui.md) — the HUD, menus, controls, text in other languages, touch.
6. [Audio](06-audio.md) — sources, one-shots, buses.
7. [Export](07-export.md) — a native build and a web build.

Three sample games sit beside the engine in `games/`: `platformer`
(a walker, coins, a goal), `breakout` (physics and collision events) and
`dodge` (timers, tweens and sensors). Each is a crate this guide's third
chapter makes from scratch, grown.

The commands in a shell block under a `<!-- ci -->` line are run by CI on
every change (`cargo xtask guide`), in order: if this guide stops working,
the build says so.
