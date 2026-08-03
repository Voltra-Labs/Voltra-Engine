# Conventions

Baseline is [RFC 430](https://rust-lang.github.io/rfcs/0430-finalizing-naming-conventions.html)
and the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html).
This document only records what those leave open or where we are stricter.

## Casing

| Item | Case | Example |
| --- | --- | --- |
| Crate / package | `kebab-case` | `voltra-render` |
| Directory under `crates/` | same string as the package | `crates/voltra-render/` |
| Module / file | `snake_case` | `gpu_context.rs` |
| Type, trait, enum variant | `UpperCamelCase` | `GpuContext`, `LoadOp` |
| Function, method, field, variable | `snake_case` | `request_redraw` |
| Constant, static | `SCREAMING_SNAKE_CASE` | `MAX_FRAMES_IN_FLIGHT` |
| Shader file | `snake_case.wgsl` | `flat_color.wgsl` |

Acronyms count as one word: `GpuContext`, not `GPUContext`; `Uuid`, not `UUID`.

**The kebab/snake trap:** the package `voltra-render` is imported as
`voltra_render`. Cargo does that translation for you — never rename the package
to match the import.

Crate names never carry a `-rs` or `rust-` affix.

## Folder rules

- The workspace root is a **virtual manifest**: no `src/` at the top level, ever.
- Every crate lives at `crates/<name>/` and keeps its own `src/`, even when it
  holds a single file.
- Directory name and package name are identical strings, so a rename is one
  `git mv` plus one `Cargo.toml` line.
- `assets/` at the repo root is for runtime data (shaders, textures, scenes).
  Never put assets inside a crate's `src/`.
- `docs/` holds prose. Code comments explain *why*; docs explain *shape*.

## Module layout inside a crate

`lib.rs` declares modules and re-exports the public surface. It contains no
logic:

```rust
//! One-line crate purpose.

pub mod context;
pub mod renderer;

pub use context::GpuContext;
pub use renderer::Renderer;
```

Split a module into a directory only once it exceeds roughly 300 lines or grows
a second concept. Prefer `foo.rs` + `foo/` over `foo/mod.rs`.

One concept per file. `context.rs` owns the device and swapchain; `renderer.rs`
owns the frame. If a file needs "and" to describe it, split it.

## Dependencies

All third-party versions live in the root `[workspace.dependencies]`. Member
crates write:

```toml
[dependencies]
wgpu.workspace = true
```

Never pin a version inside a member crate — that is how a workspace ends up
compiling two copies of `wgpu`.

A crate that wraps a third-party API re-exports it (`pub use wgpu;`) so
downstream crates do not declare it themselves.

## Errors and logging

- Library crates return `Result` with their own error enum. Panicking is
  acceptable only for genuinely unrecoverable startup failures (no GPU adapter),
  and the message must say what failed.
- No `unwrap()` in committed code outside tests. `expect("why this cannot fail")`
  is fine when the invariant is real.
- Log through the `log` crate, never `println!`. Target names come free from the
  module path, so `RUST_LOG=voltra_render=debug` just works.

## Comments

Comment the non-obvious: why a branch exists, why an API is used oddly, what
invariant holds. Do not narrate what the code already says.

```rust
// Android and iOS resume more than once; only the first pass builds.
if self.window.is_some() {
    return;
}
```

Public items get `///` doc comments. Crates and modules get `//!` headers.

## Tests

- Unit tests live in the file they test, in `#[cfg(test)] mod tests`.
- Integration tests live in `crates/<name>/tests/`.
- Anything requiring a GPU device is gated so CI without an adapter still passes.

## Git

- Branch names: `feature/<topic>`, `fix/<topic>`, `refactor/<topic>`,
  `docs/<topic>`.
- [Conventional Commits](https://www.conventionalcommits.org/) with a scope
  matching the crate, minus the `voltra-` prefix:

```
feat(render): add render pipeline and WGSL shader loading
fix(core): skip redraw while the window is minimised
docs(architecture): record wgpu 30 API differences
```

- Subject in imperative mood, no trailing period, 50 characters or fewer. Body
  only when the *why* is not obvious from the diff.

## Before every commit

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo clippy` must be warning-free. A warning that is genuinely wrong gets an
`#[allow(...)]` with a comment explaining why — never a blanket allow at crate
level.
