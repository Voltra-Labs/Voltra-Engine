# 1. Install

## Rust

Voltra is a Rust workspace on the stable toolchain. Install it with
[rustup](https://rustup.rs), then check:

```sh
rustc --version   # 1.97 or newer
cargo --version
```

## System packages

The audio layer links ALSA on Linux and the gamepad layer links udev; both
are found through `pkg-config`. On Debian or Ubuntu:

```sh
sudo apt-get install -y libasound2-dev libudev-dev pkg-config
```

macOS and Windows need nothing beyond Rust. The renderer runs on Vulkan,
Metal, DirectX 12 or OpenGL — whichever the machine has.

## For web builds

Chapter 7 builds for the browser. That needs the wasm target and the
`wasm-bindgen` command at the exact version of the crate in `Cargo.lock`:

```sh
rustup target add wasm32-unknown-unknown
grep -A1 'name = "wasm-bindgen"' Cargo.lock    # the version to install
cargo install wasm-bindgen-cli --version <that version>
```

## The engine

```sh
git clone <the repository> voltra
cd voltra
cargo build -p voltra-editor
cargo run -p voltra-editor
```

The editor opens on a small demo scene. The first build compiles the whole
engine and takes a few minutes; after that, a change rebuilds in seconds.

The layout, in short: `crates/` holds the engine (`voltra-core` is the
loop and the window, `voltra-scene` the components, `voltra-render` the
GPU, `voltra-physics`, `voltra-audio`, …), `assets/` the files scenes name,
`games/` the sample games. `docs/ARCHITECTURE.md` explains why each piece
is the shape it is.
