# CLAUDE.md

Voltra Engine — a Rust game engine on `wgpu` + `winit`. Rewrite of a C++/OpenGL
engine; the C++ tree survives in git history under the tag `v0-cpp-final`.

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before changing crate
boundaries, and [docs/CONVENTIONS.md](docs/CONVENTIONS.md) before naming
anything. Both are short.

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
