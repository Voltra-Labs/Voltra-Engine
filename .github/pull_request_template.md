<!--
Title: Conventional Commits, scope = crate without the `voltra-` prefix,
imperative, ≤ 50 chars. e.g. `feat(render): add bloom to the post chain`.
Delete any section that does not apply rather than leaving it empty.
-->

## Summary

<!-- One or two sentences: what this changes and why. Name the stage if it is one (e.g. "Stage 53: 2D shadows"). -->

## Changes

<!-- What a reviewer needs to know, grouped by crate or concept. Public types, file formats, editor UI. -->

-

## Design

<!--
How it works and why this shape. If an established engine (Unity, Unreal,
Godot, Bevy) solved it, say which and why their shape fits or does not.
Alternatives rejected, in a line each.
-->

- Decision recorded in `docs/ARCHITECTURE.md`: <!-- section name, or "n/a" -->

## Compatibility

<!-- Delete if nothing applies. -->

- [ ] Scene / prefab / build files: existing files still load; new fields default and are not written when default
- [ ] Public API changes in a crate other crates use (list them)
- [ ] New dependency, added to root `[workspace.dependencies]` only

## Verification

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace` — <!-- N passing -->
- [ ] GPU tests ran on an adapter, not skipped <!-- which: hardware, lavapipe, DX12/FXC -->
- [ ] Checked in the editor / player window <!-- or say why not -->

<!-- New tests, in a line each: what they pin. Numbers for performance work (before → after, how measured). -->

## Project rules

- [ ] 2D only: no speculative 3D, no depth attachment, no axis-named 2D concepts
- [ ] `winit` only in core, `wgpu` only in render, `cpal`/`symphonia` only in audio
- [ ] No `unwrap()` outside tests; logging through `log`
- [ ] One concept per file; any split of an existing file is its own move-only commit
- [ ] Empty, resized and despawned cases handled; no silent failure
- [ ] GPU / UI / audio APIs checked against the vendored source or Context7, lasting findings in ARCHITECTURE.md

## Not yet

<!-- What this deliberately leaves for later, so it is not mistaken for an oversight. -->

-

## Screenshots

<!-- For anything visible: before / after. Delete otherwise. -->
