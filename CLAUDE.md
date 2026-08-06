# CLAUDE.md

Voltra Engine — a Rust game engine on `wgpu` + `winit`. Rewrite of a C++/OpenGL
engine; the C++ tree survives in git history under the tag `v0-cpp-final`.

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before changing crate
boundaries, and [docs/CONVENTIONS.md](docs/CONVENTIONS.md) before naming
anything. Both are short.

## The engine is 2D. 3D comes later

The README calls this a 2D/3D engine and 3D is genuinely on the roadmap, but
**nothing is built for it yet and nothing should be.** Every subsystem written
now targets 2D and only 2D.

That is not aspiration, it is the current state of the tree:

- `Vertex::position` is `[f32; 2]`.
- There is no depth buffer anywhere — `depth_stencil: None` in every pipeline
  and `depth_stencil_attachment: None` in every pass. What covers what is decided
  by draw order, the painter's algorithm, and nothing else.
- `Transform` is `Vec2` plus `Mat3`. `Camera2D` is orthographic.
- `Mat4` and `Vec3` appear only in `camera.rs`, because WGSL wants a 4×4 in the
  uniform. They are not a third dimension arriving early.

What this means when writing code:

- **Do not build 3D scaffolding speculatively.** No `Transform3D` beside
  `Transform`, no depth attachment "ready for later", no z-axis on a component
  that has no use for one. Same rule as the empty-crates one below: it rots.
- **Do not bend a 2D design to accommodate an unbuilt 3D one.** That produces
  two half-designs. Solve 2D properly; 3D gets its own design when there is code
  for it.
- **Do not name a 2D concept after an axis it is not.** Godot's `z_index` is a
  sorting key with no relation to any Z coordinate, and the collision confuses
  people permanently. Unity's `sortingOrder` is the better model. When a real Z
  exists, the name must still be free.
- 3D is not a variant of 2D here. Picking becomes a ray cast rather than a
  point-in-quad test, sorting becomes a depth test rather than an ordered draw,
  and the camera gains a frustum. Each is a new subsystem, not a parameter.

## Commands

```sh
cargo run -p voltra-editor                                  # launch the editor
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings       # must be clean
cargo fmt --all
```

Run all three of `fmt`, `clippy`, `test` before reporting work as done.

The editor is a GUI app with an infinite event loop — never run it in the
foreground. Launch it detached, give it a few seconds, check the log, kill it.

## Layout

```
Cargo.toml           virtual manifest — workspace members + shared dep versions
assets/              runtime assets (shaders, textures, scenes)
crates/
  voltra-render/     GPU layer: device, surface, passes     — owns wgpu
  voltra-core/       platform layer: event loop, window     — owns winit
  voltra-editor/     the editor binary
docs/                ARCHITECTURE.md, CONVENTIONS.md
```

## Hard rules

- **No `src/` at the workspace root.** The root manifest is virtual.
- **Only `voltra-core` depends on `winit`. Only `voltra-render` depends on
  `wgpu`.** Other crates use the re-exports. If a change would make
  `voltra-render` import `winit`, the design is wrong — pass a
  `wgpu::SurfaceTarget` instead.
- **No ECS, scene-graph or engine-framework crates.** Writing those in-house is
  the point of this project. Leaf libraries (math, serde, physics, `egui`) are
  fine. `egui-wgpu` is not usable — it is pinned to wgpu 29 and would give the
  build two incompatible copies of wgpu; the backend in
  `voltra-render::egui_backend` is ours.
- **All versions live in root `[workspace.dependencies]`**; member crates write
  `dep.workspace = true`. Never pin a version inside a member crate.
- **New crates only when there is code for them.** Do not scaffold empty crates
  from the planned list in ARCHITECTURE.md.
- No `unwrap()` outside tests. `expect("why this cannot fail")` when the
  invariant is real. Log via `log`, never `println!`.
- **One concept per file, one responsibility per folder.** CONVENTIONS.md sets
  the bar and it is not optional: split a module into a directory once it passes
  roughly 300 lines or grows a second concept, preferring `foo.rs` + `foo/` over
  `foo/mod.rs`. If describing a file needs the word "and", it is two files.
  Do not pile new code into `voltra-core` or `voltra-editor` because they happen
  to be where the wiring lives — a subsystem gets its own module directory, and
  its own crate once it stops being describable without naming another one.
  Split *before* adding to an oversized file, in its own move-only commit, so
  the split and the new behaviour never share a diff.
- **Build it robust and scalable, never "good enough to move on".** This is a
  long-lived engine, not a demo. A shortcut taken to close a task becomes the
  thing the next subsystem is built on. Concretely: no hardcoded values where
  the engine will need a parameter, no special case where the general case is
  the same amount of work, no silent failure where an error should propagate,
  no data structure chosen for one caller when the second caller is already
  planned. Handle the empty, the resized and the despawned case at the time the
  code is written. If the robust version is genuinely much larger, say so and
  ask — do not quietly ship the shortcut.

## Verify graphics APIs, do not recall them

`wgpu` 30, `winit` 0.30 and `egui` 0.35 are newer than the model's training data.
wgpu 30 broke nearly every tutorial online (they target v25 and older), and egui
0.35 merged the panel types and changed how a frame is run. Writing this code
from memory produces plausible code that does not compile.

Before writing GPU or UI code, do one of:

1. Query **Context7** (MCP) for the current `wgpu` / `winit` / `egui` docs.
2. Read the vendored source directly — it is on disk and authoritative:
   `~/.cargo/registry/src/index.crates.io-*/wgpu-30.0.0/src/api/`,
   `.../egui-0.35.0/src/`.

The differences already found are tabulated at the end of
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), one table per crate. Add to them
whenever you hit a new one.

Colour space is the other thing not to guess at. sRGB conversions happen in
three places — the texture format, the sampler and the shader — and applying one
twice is a visible darkening that no validation layer reports. The rules that
hold here are written up under "egui for the editor UI" in ARCHITECTURE.md, and
`crates/voltra-render/tests/headless_egui.rs` pins them with mid-tone pixels,
which are the only values that can tell a double conversion from a correct one.

## Look it up before inventing it

Two different triggers, same response — go and read, do not reason from memory:

- **You are not certain.** An API signature, a colour-space rule, a wgpu type, a
  crate's current behaviour. Query Context7 or read the vendored source. See
  "Verify graphics APIs" above.
- **The decision is important and someone has already solved it.** Camera
  controls, asset hot reload, render graph shape, scene format, undo, gizmos,
  input scoping, ECS storage. Search the web and find out how the established
  engines do it — **Unity, Unreal, Godot, Bevy** — before choosing. They have had
  the bug reports we have not.

Say what you found and why the shape they use is or is not right here. Copying
their answer without the reason is as bad as inventing one. Where their solution
does not fit our layering, name the difference and adapt it deliberately — this
is a wgpu engine with a hand-written ECS, not a clone of any of them.

Findings worth keeping go in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) under
"Decisions", with the alternatives that were rejected.

## Models and delegation

**Think on Opus 5. Execute wide on Sonnet 5.**

Use Opus for the work where being wrong is expensive: architecture and crate
boundaries, ECS storage design, render graph shape, unsafe reasoning, lifetime
and borrow puzzles, debugging something whose cause is not yet known.

Delegate to subagents with `model: "sonnet"` once the shape of the work is
already decided and the task is well specified:

- searching the codebase or the registry sources for an API or usage
- mechanical refactors across many files
- writing tests against a signature that already exists
- porting a known C++ subsystem to an already-agreed Rust design
- independent tasks that can run in parallel

Guidance for delegating well:

- Fan out only when the subtasks are genuinely independent. Sequential work with
  shared context is faster in the main thread.
- A subagent starts cold. Give it the file paths, the target API, the acceptance
  check (`cargo clippy --workspace -- -D warnings`), and the conventions link —
  it cannot see this conversation.
- Review what comes back. Subagents miss the layering rules above; a returned
  diff that makes `voltra-render` import `winit` gets rejected, not merged.
- Keep the final architectural call on Opus, even when Sonnet wrote the code.

## Plugins and skills

Installed and expected to be used (all user-scope; skip any that is absent):

| Tool | Use it for |
| --- | --- |
| **context7** (MCP) | Current docs for `wgpu`, `winit`, any crate. Mandatory before writing graphics code — see above. |
| **superpowers** | Workflow skills: `brainstorming` and `writing-plans` before a subsystem, `test-driven-development` for pure logic (ECS, math), `systematic-debugging` when a bug's cause is unknown, `subagent-driven-development` and `dispatching-parallel-agents` when fanning out, `verification-before-completion` before saying done. |
| **security-guidance** | Runs on edits and commits. Take its findings seriously in asset loading and deserialization paths. |
| **claude-md-management** | `/revise-claude-md` when this file drifts from reality. |
| **caveman** | Response style. **Always on** — see below. |

**caveman is the default register in this repo.** Answer terse, no filler, no
pleasantries, fragments fine. It applies from the first reply of a session
without being asked, and it does not decay over a long conversation. Full
technical substance stays: code blocks, API names, error strings and commit
messages are written normally and never compressed. Drop out of it only for
security warnings, destructive-action confirmations, and multi-step sequences
where terseness would make the order ambiguous — then resume. Off only if the
user says "stop caveman" / "normal mode".

**Invoke superpowers before starting work, not after.** Gather the context and
decide the shape first: `brainstorming` before any new feature or subsystem —
including ones that look small — then `writing-plans` if it spans more than a
couple of files, `test-driven-development` for pure logic, and
`systematic-debugging` the moment a cause is unknown. Finish with
`verification-before-completion`. Improvising the workflow is what produces the
shortcuts the hard rule above forbids; `systematic-debugging` in particular
beats guessing at a graphics bug — GPU issues punish speculation.

**Context7 is a lookup, not a store.** Query it to *read* current `wgpu` /
`winit` / `egui` docs before writing graphics or UI code — that part is
mandatory. It cannot save anything. Findings that must survive the session go in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) (API differences, design decisions)
or in a doc comment next to the code they explain.

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
