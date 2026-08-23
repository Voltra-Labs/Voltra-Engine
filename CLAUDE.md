# CLAUDE.md

Voltra Engine — a Rust game engine on `wgpu` + `winit`. Rewrite of a C++/OpenGL
engine; the C++ tree survives in git history under the tag `v0-cpp-final`.

On demand (do not preload every task):
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — crate boundaries, API quirks, decisions
- [docs/CONVENTIONS.md](docs/CONVENTIONS.md) — naming

## The engine is 2D. 3D comes later

The README mentions 2D/3D and 3D is on the roadmap, but **nothing is built for
it yet and nothing should be.** Every subsystem written now targets 2D only.

Current tree facts:

- `Vertex::position` is `[f32; 2]`.
- No depth buffer — `depth_stencil: None` / `depth_stencil_attachment: None`.
  Draw order (painter's algorithm) decides coverage.
- `Transform` is `Vec2` + `Mat3`. `Camera2D` is orthographic.
- `Mat4` / `Vec3` appear only in `camera.rs` because WGSL wants a 4×4 uniform.
  They are not a third dimension arriving early.

Rules:

- **No speculative 3D scaffolding.** No `Transform3D`, no depth attachment
  "ready for later", no unused z-axis on components.
- **Do not bend 2D designs for unbuilt 3D.** Solve 2D properly; 3D gets its own
  design when there is code for it.
- **Do not name a 2D concept after an axis it is not.** Prefer Unity-style
  `sortingOrder` over Godot-style `z_index`. Keep real-Z names free.
- 3D is a new subsystem later (ray pick, depth test, frustum), not a parameter.

## Commands

```sh
cargo run -p voltra-editor                                  # launch the editor
cargo run -p voltra-player -- assets/scenes/sandbox.ron      # run a scene, no editor
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings       # must be clean
cargo fmt --all
```

Before calling work done: `fmt`, then tests (and clippy) **scoped to what
changed** when possible; full workspace clippy when touching shared surfaces
or before a PR. The editor and the player both have an infinite event loop —
launch detached, check the log, kill it; never run one in the foreground.

## Layout

```
Cargo.toml           virtual manifest — workspace members + shared dep versions
assets/              runtime assets (shaders, textures, scenes)
crates/
  voltra-render/     GPU layer: device, surface, passes     — owns wgpu
  voltra-audio/      mixer, decoding, output device         — owns cpal+symphonia
  voltra-core/       platform layer: event loop, window     — owns winit
  voltra-editor/     the editor binary
  voltra-player/     the player binary: runs a scene, no editor linked
docs/                ARCHITECTURE.md, CONVENTIONS.md
```

## Hard rules

- **No `src/` at the workspace root.** The root manifest is virtual.
- **Only `voltra-core` depends on `winit`. Only `voltra-render` depends on
  `wgpu`. Only `voltra-audio` depends on `cpal` and `symphonia`.** Other crates
  use the re-exports. If a change would make `voltra-render` import `winit`,
  the design is wrong — pass a `wgpu::SurfaceTarget` instead.
- **No ECS, scene-graph or engine-framework crates.** Writing those in-house is
  the point. Leaf libraries (math, serde, physics, `egui`) are fine.
  `egui-wgpu` is unusable (pinned to wgpu 29); use
  `voltra-render::egui_backend`.
- **All versions live in root `[workspace.dependencies]`**; members use
  `dep.workspace = true`. Never pin a version inside a member crate.
- **New crates only when there is code for them.** Do not scaffold empty crates
  from the planned list in ARCHITECTURE.md.
- No `unwrap()` outside tests. `expect("why this cannot fail")` when the
  invariant is real. Log via `log`, never `println!`.
- **One concept per file, one responsibility per folder.** Split a module into
  a directory around ~300 lines or a second concept; prefer `foo.rs` + `foo/`
  over `foo/mod.rs`. If describing a file needs "and", it is two files. Do not
  pile subsystems into `voltra-core` / `voltra-editor` just because wiring lives
  there. Split *before* growing an oversized file, in a move-only commit.
- **Robust by default.** No hardcoded values the engine will need as parameters,
  no special case when the general case is the same work, no silent failure,
  no one-caller data structure when a second caller is already planned. Handle
  empty / resized / despawned when writing the code. If the robust version is
  much larger, say so and ask.

## Verify graphics and audio APIs, do not recall them

`wgpu` 30, `winit` 0.30, `egui` 0.35, `cpal` 0.18 and `symphonia` 0.6 all
differ from most training data and tutorials. Before writing **GPU, editor UI
or audio** code:

1. Query **Context7** (MCP), or
2. Read vendored sources under `~/.cargo/registry/src/index.crates.io-*/`.

Record lasting API findings in ARCHITECTURE.md (tables at the end). Colour
space: sRGB can be applied in texture format, sampler, and shader — double
conversion darkens mid-tones; rules live under "egui for the editor UI" in
ARCHITECTURE.md and are pinned by `crates/voltra-render/tests/headless_egui.rs`.

## Look it up before inventing it

- Uncertain API / colour-space / crate behaviour → Context7 or vendored source
  (GPU/UI paths above).
- Important design already solved elsewhere (camera, hot reload, undo, gizmos,
  ECS storage, …) → check Unity / Unreal / Godot / Bevy, then adapt to our
  layering on purpose. Say why their shape fits or does not. Lasting decisions
  go under "Decisions" in ARCHITECTURE.md.

Do **not** run a web/engine survey for routine local fixes.

## Models and delegation

**Prefer Opus for architecture** (crate boundaries, ECS storage, render graph,
unsafe, unknown-cause debugging). **Prefer a cheaper model for mechanical
execution** when the shape is already decided.

Delegation via `cursor-orchestrator` (Grok in an isolated worktree) is
**optional**, for parallel or mechanical slices — not mandatory on every task.
Built-in Agent is fine when orchestrator is absent or overhead is not worth it.

If using orchestrator:

```
mcp__cursor-orchestrator__spawn_agent
  repo:   "Voltra-Engine"              # under ORCH_REPOS_ROOT = D:\Proyectos
  model:  "cursor-grok-4.5-high"       # -medium mechanical; -low search
  mode:   "full"                       # Windows: ask | full only
  prompt: <full briefing — agent starts cold>
```

Review `get_job_diff` before `apply_job`. Subagents may commit / amend their
own HEAD only — no rebase, reset --hard, force-push, or plumbing
(`hash-object`, `commit-tree`, …). Verify claims with `git log`, not the report.

Brief subagents with paths, acceptance check, and hard rules: 2D only,
`winit` only in core, `wgpu` only in render, workspace versions, no
`unwrap()` outside tests, one concept per file.

## Plugins and skills

Use when they help; **do not ritualize every task**:

| Tool | Use it for |
| --- | --- |
| **context7** | Current `wgpu` / `winit` / `egui` docs before GPU/UI code |
| **cursor-orchestrator** | Optional delegated work in an isolated worktree |
| **superpowers** | New subsystems / unknown bugs: brainstorm → plan → TDD → verify. Skip for small local fixes |
| **security-guidance** | Take findings seriously on asset load / deserialization |
| **claude-md-management** | `/revise-claude-md` when this file drifts |
| **caveman** | Default terse register (below) |

**caveman is the default register.** Terse, no filler; full technical substance
in code, APIs, errors, commit messages. Off only if the user says
"stop caveman" / "normal mode".

**Superpowers:** use for new features/subsystems and unknown-cause bugs. Skip
approval gates (go spec → plan → implement; report at the end). Ask open
questions up front when the answer changes the work. Do **not** invoke the
full skill chain for a one-file fix.

**Context7** is a lookup, not a store. Persist findings in ARCHITECTURE.md or
a doc comment next to the code.

## Session hygiene (tokens)

- Prefer short sessions: finish 1–2 plan tasks, then new chat or `/compact`.
- Avoid `Continua` on a huge context after a session-limit hit — start fresh
  with paths + task id instead.
- Do not preload ARCHITECTURE.md / skill files unless the task needs them.
- Superpowers specs/plans are working files. Delete them once the feature is
  in the tree and the decision is in ARCHITECTURE.md.

## Git

Branch off `main`: `feature/<topic>`, `fix/<topic>`, `refactor/<topic>`,
`docs/<topic>`. Never commit straight to `main`; open a PR.

[Conventional Commits](https://www.conventionalcommits.org/), scope = crate
without the `voltra-` prefix, subject imperative and ≤50 chars:

```
feat(render): add render pipeline and WGSL shader loading
fix(core): skip redraw while the window is minimised
```

Do not commit, push, or open a PR unless asked.
