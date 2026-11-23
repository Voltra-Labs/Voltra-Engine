# Exporting to the web — W1 to W4

A Voltra game running in a browser tab: `cargo xtask dist --web` produces a
folder any static server can host, and the player inside it runs a scene
with sprites, lights, physics, sound and input, on WebGPU where the browser
has it and WebGL2 where it does not. The editor stays native.

Working file (CLAUDE.md): delete it once W4 is in the tree and its
decisions are in ARCHITECTURE.md.

## Where the engine stands (checked, stage 74)

`cargo check --target wasm32-unknown-unknown` per crate:

- **Compile today:** `voltra-ecs`, `voltra-jobs`, `voltra-profile`,
  `voltra-render`, `voltra-audio`.
- **Do not compile:** `uuid` wants a randomness source on wasm (its `js`
  feature, target-specific); `voltra-assets` requires `Sync` of a
  `wgpu::TextureView`, which the WebGPU backend's is not (`size.rs`, the
  `TextureSizes` impl). Everything above them follows.

What compiles but would fail at run time, and is the real work:

| Problem | Where | Answer |
| --- | --- | --- |
| `std::time::Instant` panics on `wasm32-unknown-unknown` | `voltra-core::time`, `voltra-profile`, physics clock, audio | `web-time` (a leaf crate: `std::time` on native, `performance.now()` on the web) everywhere |
| `std::thread::spawn` panics without atomics | `voltra-jobs::pool` (texture preload) | A pool of zero workers runs jobs inline on wasm |
| `std::fs` does not exist in the browser | every asset store, scene and prefab load, input map, sorting layers, fonts, clips | One funnel, `voltra_assets::fs::{read, read_to_string, exists}`: `std::fs` natively, an in-memory pack on the web, filled before the game starts |
| Device creation is async on the web; `pollster::block_on` cannot block | `GpuContext::new`, `App::resumed` | `App` creates its renderer through a future on the web (`wasm_bindgen_futures::spawn_local`) and finishes `resumed` when an `EventLoopProxy` event brings it back |
| The window is a canvas | `App::resumed` | `WindowAttributesExtWebSys::with_append(true)`, sized to the page |
| Hot reload watches files | `notify` | Not built on wasm (target-specific dependency) |
| `env_logger` writes to stderr | `voltra-core::logging` | A `log` logger writing to the browser console on wasm |
| Saves go to a per-user folder | `voltra-core::saves` | `localStorage`, keyed by game and slot |
| Sound needs a user gesture | `cpal` | `cpal`'s `wasm-bindgen` feature (WebAudio); the stream starts on the first key or click |
| No compute shaders on WebGL2 | GPU particles | Already falls back to CPU particles on a downlevel adapter; check the light and post shaders against WebGL2's limits |

## Rules for all four stages

- CLAUDE.md holds: 2D only; `winit` only in core, `wgpu` only in render,
  `cpal`/`symphonia` only in audio; versions in root
  `[workspace.dependencies]`; no `unwrap()` outside tests.
- **Target-specific code behind `cfg(target_arch = "wasm32")`, in its own
  file** (`foo/web.rs` beside `foo/native.rs`), not sprinkled through
  functions.
- **Native behaviour does not change.** Every W stage keeps
  `cargo test --workspace` green on Linux and Windows.
- **Verify, do not recall** — winit's web platform, wgpu's web backends,
  cpal's wasm support and wasm-bindgen all read from vendored source first,
  and findings go into ARCHITECTURE.md's API tables.
- New dependencies are leaf crates (`web-time`, `wasm-bindgen`,
  `wasm-bindgen-futures`, `web-sys`, `js-sys`), target-specific.

## W1 — the engine builds for wasm32 — done

Landed: `voltra_jobs::{MaybeSend, MaybeSync}` and inline jobs on wasm;
`uuid`'s `js` feature on wasm; `web-time` behind `voltra-core::time` and
`voltra-profile`'s clock; `egui-winit`'s defaults native only (the
clipboard); `voltra_assets::fs` with mountable packs and every game-time
read through it (`voltra_audio::decode_bytes` for clips); CI lints the
player for wasm32. `notify` and `env_logger` still compile for wasm and
are left to W2, which gives the player its web entry point.

- `uuid` with `js` on wasm; the `TextureSizes` bound relaxed or the impl
  moved so `voltra-assets` builds on the WebGPU backend.
- `web-time::Instant` in place of `std::time::Instant` across the
  workspace.
- `voltra-jobs`: zero workers on wasm, jobs run inline.
- `voltra_assets::fs`: the funnel, native only for now (the web half is
  W2), and every asset read routed through it.
- `notify` and `env_logger` native only; a console logger on wasm.
- CI: `cargo check -p voltra-player --target wasm32-unknown-unknown`.

**Done when** the player checks for wasm32 in CI, and nothing native
changed (the workspace tests are untouched and green).

## W2 — a scene runs in a browser

- Async renderer creation in `App` on the web, canvas appended to the page
  and following its size.
- The asset pack: `xtask` writes `assets.pak` (paths and bytes, nothing
  clever); the web player fetches it before the app starts and installs it
  behind `voltra_assets::fs`.
- WebGPU, with WebGL2 through wgpu's `webgl` feature when
  `navigator.gpu` is missing; shaders checked against WebGL2.
- Saves in `localStorage`.
- **Done when** a Playwright test (Chromium is preinstalled) loads the
  built page, waits for the first frame and finds the sandbox's sprites in
  a screenshot.

## W3 — sound and input in the browser

- cpal's `wasm-bindgen` backend; the stream opens on the first gesture.
- Keyboard focus on the canvas; gamepads through gilrs's web support.
- **Done when** the Playwright test presses an action and sees the player
  move, and a clip plays without an error in the console.

## W4 — `cargo xtask dist --web`

- Builds the player for wasm32 in release, runs `wasm-bindgen` (the CLI's
  version must equal the crate's; `xtask` checks and says which to
  install), writes `index.html` (canvas, a loading line, errors shown on
  the page), packs `build.ron`'s assets.
- **Done when** `dist/<name>-web/` served by any static server runs the
  game, and `docs/` says how to publish it.

## Open questions (answer in the stage that needs them)

- `wasm-bindgen-cli` on CI and on people's machines: install in xtask, or
  document. W4.
- Whether the pack is one file or many fetches: one file is simplest and
  is the default; streaming large games is later. W2.
- Threads on the web (`atomics`, COOP/COEP headers): not before a game
  needs them.
