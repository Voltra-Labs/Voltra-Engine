# The MCP server — M1 to M3

An MCP server through which an AI client does in a Voltra project what a
person does in the editor, and checks its own work: edit scenes and assets,
run the game for a few seconds, look at a screenshot and the log, build and
test. The *what* is in [plan-23-onward.md](plan-23-onward.md) ("An MCP
server"); this file is the *how*, in three stages, so the order and the
acceptance checks are not lost between sessions.

Written after stage 71, when 2D was close enough to done that the tool
surface will not move under its users. Delete this file once M3 is in the
tree and its decisions are in ARCHITECTURE.md (CLAUDE.md: plans are working
files).

## Where the engine stands

What already carries an MCP server, so no stage reinvents it:

- **Every component has a name and a text form.** `ComponentRegistry` saves,
  loads, normalises and removes any registered component by name, as RON.
  One generic "read/write component" tool covers all of them, today's and
  tomorrow's — no tool per component.
- **Entities have stable identities.** `SceneId` survives save, load,
  undo and copies; `keyframes::target::resolve` finds an entity by a path of
  names; `format::entity_with_id` finds one by id.
- **References follow copies.** `register_with_refs` / `SceneRefs`: a tool
  that copies entities gets joints and camera targets remapped for free.
- **Entities already have a text exchange format**: the clipboard's
  `voltra-entities 1` (stage 63), with fresh ids on paste.
- **Every asset is a RON file that is validated when read and hot
  reloaded**: scenes, prefabs, atlases, input maps, sorting layers, clips
  (`.anim.ron`), state machines (`.animator.ron`). A tool can write the file
  and the running editor picks it up.
- **Undo records explicit edits**: `History::begin` / `commit` /
  `commit_including`, `spawn::record`. An edit from outside the frame can be
  one entry.
- **Rendering without a window works** — but only inside tests
  (`voltra-testkit::headless_device`, `read_texture`, `write_png`).

What is missing, and is M1:

- `save_one` / `load_one` are `pub(crate)`; there is no template of a
  component's fields for an AI to fill in (no reflection).
- No headless session: `App::advance` is private; nothing loads a scene,
  steps it N frames, feeds it input and answers what happened.
- No screenshot outside the tests.
- The log goes to stderr or a file; nothing can hand back "what happened in
  those 60 frames".
- No way into a running editor from another process.

## Rules for all three stages

- **CLAUDE.md holds**: 2D only; `winit` only in core, `wgpu` only in render;
  every version in root `[workspace.dependencies]`; no `unwrap()` outside
  tests; one concept per file; robust by default.
- **One tool call, one undo entry** when the editor is live (M3), labelled
  with the tool's name. An AI edit is undone like a person's.
- **Generic over specific.** Components through the registry by name; assets
  through their stores. A tool per component or per asset kind is a tool
  that falls behind the engine.
- **Local only.** stdio for the MCP client; the editor bridge listens on
  loopback only, off unless asked for. File writes confined to the project
  root — a path that resolves outside it is refused, symlinks included.
- **Destructive tools say so** in their description (delete, overwrite, dist,
  run arbitrary cargo), and nothing deletes outside `assets/`.
- **Errors are answers**, not crashes: a tool that fails returns the
  engine's own error text (a `SceneError`, a parse span), so the AI can fix
  its input.
- **Verify protocol APIs, do not recall them** — same rule as wgpu: read the
  vendored `rmcp` source (or Context7) before writing against it, and record
  findings in ARCHITECTURE.md's API tables.

## M1 — engine prep (no protocol yet)

Each piece earns its place without MCP: CI can run scenes headless, tests
can screenshot, games can show their log. Four PRs, roughly in this order.

### M1a — components by name, with templates — done

Landed: public `save_one`/`load_one`/`remove_one`/`normalize`, `register` →
`Registration::{with_template, with_refs}`, `template(name)`, `describe()`;
`Collider: Default`. Kept the existing method names.

- Make `ComponentRegistry::save_one`, `load_one`, `remove_one` public
  (rename if the public names read better: `get_ron`, `set_ron`, `remove`).
- `ComponentRegistry::template(name) -> Option<Box<RawValue>>`: the
  component's `Default`, written as RON — what an AI fills in. Registration
  captures it for types that are `Default`; one that is not answers `None`
  and says so. Every built-in component is `Default` today; check.
- `ComponentRegistry::describe()`: names, whether each has refs, whether it
  has a template. Enough for a tool listing without reflection.
- Tests: every registered default component has a template that loads back
  into an entity; an unregistered name answers `None`, not an error.

### M1b — a headless session — done

Landed as `voltra_core::Headless` (`new`, `open`, `step`, `press`/`release`,
`world`, `entity_ron`, `set_component`, `into_app`); no `fixed_steps(n)` or
axis feed yet — `step` with a fixed delta covers exact steps.

A `Session` (name to settle: `voltra_core::Headless`?) owning an `App`
without a window or a device:

- `Session::open(asset_root, scene: &AssetPath) -> Result<Self, SceneError>`
- `step(frames: u32, delta: f32)` — runs the same `advance` the loop runs,
  simulation on; `fixed_steps(n)` for exact physics steps.
- `press(action)` / `release(action)` / `axis(…)`: input actions fed in, as
  a game reads them.
- `world() -> &World`, plus `entity_ron(id) -> String`: an entity as the
  clipboard writes it.
- Prefab sync, hot reload of stores, scene requests from ticks: all work as
  in the player — this is the player without a window, not a second loop.
- Uses `App::advance` made `pub(crate)` + a thin public wrapper; does not
  duplicate frame order.
- Tests: a scene with a falling body steps and lands; an animator changes
  state from a fed action; a scene that will not read is an error, not a
  panic.

### M1c — screenshots to PNG — done

Landed: `GpuContext::headless`, `Renderer::headless`, `voltra_render::read_rgba8`,
the shared `WorldDraw`, `Headless::screenshot` → `Screenshot`
(`pixel`, `save_png`, `png_bytes`). The editor's viewport readback is M3's.

- `voltra-render`: render a frame into an offscreen `RenderTarget` and read
  it back (the path `headless_render.rs` already proves), as a public API,
  not test-only. `read_texture` moves out of testkit or gets a production
  twin.
- `Session::screenshot(size) -> Result<RgbaImage-ish, …>` and
  `.save_png(path)`: the scene through its active camera, lit, post-processed
  — the same draw the player makes. Needs a device: `Session` gains an
  optional device (lavapipe in CI), and screenshot fails cleanly without one.
- Editor side (for M3): the viewport's target read back the same way.
- Tests: a lit sample scene screenshot has the light where it is
  (`lights_scene.rs` checks become reusable).

### M1d — capturable logs — done

Landed: `logging::Tee` over `env_logger`, `logging::capture()` →
`LogCapture::{drain, missed}`, `Headless::take_logs`. **M1 is done.**

- `voltra_core::logging`: a ring buffer sink alongside stderr/file —
  `logging::capture()` returns a guard; `drain()` gives the lines since. A
  `Session` drains after each `step` so a tool can return "what the engine
  said while it ran".
- Warn-once latches (missing camera, missing target) still latch: a test
  pins that capture does not change what is logged.

**M1 is done when** a test can open `assets/scenes/sandbox.ron`, step it
120 frames, set a component by name from RON, screenshot it to PNG and read
back the log — with no window.

## M2 — `voltra-mcp`, file and headless mode

Progress: **M2a–b landed** — the crate, stdio, discovery (`components_list`,
`assets_list`, `scenes_list`, `scene_read`) and scene editing (`scene_create`,
`entity_spawn/delete/reparent/rename`, `component_set/remove`). Next: M2c
(`run_scene`, `asset_read/write`), M2d (project tools, prefabs, docs).

A new crate, `crates/voltra-mcp`, a binary speaking MCP over stdio. No
editor needed: it works on the project's files and a `Session` from M1.
This is what CI and "edit the project while the editor is closed" use.

- **SDK**: `rmcp` (the official Rust MCP SDK) + the `tokio` runtime it
  needs + `serde_json`, all in root `[workspace.dependencies]`. Check its
  current API in the vendored source first.
- **Config**: `voltra-mcp --project <dir>`; the asset root and `build.ron`
  found from there. A Claude Code `.mcp.json` example in the docs.

Tools (names to settle; `scene_*` / `asset_*` / `run_*` / `project_*`):

| Group | Tools |
| --- | --- |
| Discovery | `components_list` (names, templates, refs), `assets_list(dir)`, `scenes_list` |
| Scenes (files) | `scene_read(path)` (entities with ids, names, parents, components as RON), `scene_create`, `entity_spawn(scene, components)`, `entity_delete`, `entity_reparent`, `entity_rename`, `component_set(scene, entity, name, ron)`, `component_remove` — each one loads, edits through the registry, validates, saves atomically |
| Prefabs | `prefab_instantiate(scene, prefab, at)`, `prefab_apply` (instance → file) |
| Assets | `asset_read(path)`, `asset_write(path, ron)` — validated by loading through the asset's own store before the write lands; `tilemap_paint(scene, layer, cells)`, `atlas_slice(texture, grid)` |
| Run | `run_scene(path, frames, inputs[])` → final entity states asked for, collision events, log lines, and a screenshot as MCP image content |
| Project | `project_build`, `project_test(filter)`, `project_dist` — cargo with fixed arguments, output returned, never an arbitrary command line |

- Entities addressed by `SceneId` (as written in the file) or by name path;
  both accepted, ids returned.
- Every write tool returns what changed (the entity's new RON), so the AI
  can confirm without a second read.
- Tests: an in-process client drives each tool against a scratch project
  copy; a malformed RON input returns the parse error with its span; a path
  outside the project is refused.

**M2 is done when** an MCP client can, against a fresh checkout and with no
editor open: list components, add a sprite with a light to a scene, run the
scene for a second, get the screenshot back and see the light in it.

## M3 — the live editor bridge

The same tools, against the running editor, so edits appear live and land
in undo.

- **Transport**: the editor opens a loopback socket (opt-in: a flag or an
  editor setting) speaking a small internal protocol — not MCP; the MCP
  server stays in `voltra-mcp` and becomes a client of the editor when one
  is listening, falling back to file mode when not. The editor binary stays
  free of the MCP protocol and its runtime.
- **Before choosing the shape**, read how Godot's and Unity's MCP bridges
  split editor plugin from server (CLAUDE.md: look it up before inventing
  it) and record why ours fits or differs in ARCHITECTURE.md.
- **Command queue**: requests arrive on a background thread, queue, and are
  applied in `Editor::ui` at a fixed point (after `history.watch`, before the
  panels), each wrapped in `History::begin` / `commit_including` under the
  tool's name. Replies carry the result after the frame.
- **Play mode**: `play`, `pause`, `step(n)`, `stop` through the existing
  `Play` paths; while playing, edits are refused (as the inspector's are)
  and reads come from the live world.
- **Seeing**: `screenshot(view: scene|game)` reads back the viewport target
  (M1c); `selection_get` / `selection_set`.
- **Input while playing**: actions injected into `Input` for N frames.
- Tests: a scripted client spawns, edits and deletes through the socket;
  each call is one undo entry and undo restores the scene; the socket
  refuses non-loopback peers; the editor without the flag opens nothing.

**M3 is done when** an MCP client connected to a running editor adds an
entity the person sees appear, presses play, reads where it fell, takes a
screenshot, stops — and the person presses Ctrl+Z and the entity is gone.

## Open questions (answer in the stage that needs them)

- Name of the headless type (`Session`, `Headless`, `HeadlessApp`). M1b.
- Whether `run_scene` screenshots by default or on request (image tokens
  cost). M2.
- Whether game-specific components (registered by a game's binary) can be
  reached: the MCP server only knows the engine's registry unless it is
  built against the game. Likely: M2 engine-only, M3 gets them for free from
  the running game/editor. Decide in M2.
- Profiler numbers: there is no profiler yet; out of scope until there is.
- Logic is compiled Rust: the AI changes behaviour by editing code and
  `project_build`; hot reloading game code is not part of this plan.
