# Publishing a game on the web

A Voltra game runs in a browser tab: on WebGPU where the browser has it and
WebGL2 where it does not, with its sprites, lights, physics, sound, keyboard
and gamepads. The editor stays native. How it works is in
[ARCHITECTURE.md](ARCHITECTURE.md), the sections starting at "The engine
builds for the web".

## Build

Once per machine:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.126 --locked
```

The CLI's version must equal the `wasm-bindgen` crate's in `Cargo.lock`.
`cargo xtask dist --web` checks before it builds and names the version to
install when they differ.

Then, for the build `build.ron` describes:

```sh
cargo xtask dist --web                                    # → dist/<name>-web/
cargo xtask dist --web --manifest platformer.build.ron    # a game with its own code
```

The folder holds four files and needs nothing else:

| File | What it is |
| --- | --- |
| `index.html` | The page: a full-window canvas, a status line, the loader |
| `game.js` | wasm-bindgen's loader |
| `game_bg.wasm` | The player, or the game's own binary |
| `game.pak` | `build.ron` and every asset, fetched once before the game starts |

A scene build ships the player, which starts the scene the manifest names.
A game with its own binary must start through
`voltra_core::when_assets_ready` — the platformer example's `main` shows
how — so it runs once the pack has arrived.

## Try it locally

Any static server, from the build folder:

```sh
cd "dist/Voltra Sandbox-web"
python3 -m http.server 8000      # then open http://localhost:8000
```

Opening `index.html` straight from the disk does not work: a page loaded
from `file://` may not fetch the wasm or the pack.

## Publish

Upload the folder's four files anywhere that serves static files — GitHub
Pages, itch.io (as an HTML game, `index.html` at the zip's root), Netlify,
a bucket behind a CDN. What the server should do:

- **Serve `.wasm` as `application/wasm`.** Every mainstream host does. With
  another type the loader still works, a little slower, and says so in the
  console.
- **Compress.** The wasm is about 13 MB and about 4 MB gzipped; hosts that
  gzip or brotli on the fly need nothing more.
- **HTTPS**, or `localhost`: WebGPU is only offered to secure pages. Over
  plain HTTP elsewhere the game falls back to WebGL2.

No special headers are needed: the web build uses no threads, so it does not
need the cross-origin isolation (`COOP`/`COEP`) threaded wasm would.

## What differs from a native build

- **The page sizes the game.** The canvas fills the window; the manifest's
  window size is a desktop setting.
- **Sound starts on the first key, click or touch.** Browsers keep audio off
  until the player interacts with the page.
- **The keyboard** goes to the canvas from the start; clicking elsewhere on
  the page takes it away, as on any web page.
- **Saves** go to the browser's `localStorage` for that site, one entry per
  slot. Clearing the site's data clears them.
- **The log** goes to the browser's console (F12), and a game that cannot
  start says why on the page.
- **No hot reload**, and no files outside the pack.

## Checking a web build

`crates/voltra-player/web/test/` holds the tests CI runs (Node and
Playwright with a Chromium):

```sh
node crates/voltra-player/web/test/first-frame.mjs   # the sandbox draws, on WebGPU and WebGL2
node crates/voltra-player/web/test/play.mjs          # the platformer walks, takes a coin, sounds
```

Each builds through `cargo xtask dist --web` into `target/`, serves it and
drives it headless; `--skip-build` reuses the last build.
