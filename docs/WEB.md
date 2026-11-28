# Publishing a game on the web

A Voltra game runs in a browser tab: on WebGPU where the browser has it and
WebGL2 where it does not, with its sprites, 3D meshes, lights, physics,
sound, keyboard and gamepads. The editor stays native. How it works is in
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
`voltra_core::when_assets_ready` — the platformer's `games/platformer/src/main.rs` shows
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
- **The network is WebSockets only.** A page joins a server by its
  address (`ws://` or `wss://`); it cannot host, and it cannot look for
  games on the LAN — a browser has no UDP, let alone broadcast. The lobby
  screen greys Host and Find on LAN there, and `tick.net.discover` answers
  `DiscoveryError::Unsupported`; the lobby itself works as natively, over
  the WebSocket (docs/guide/08-networking.md).
- **3D depth is less precise on WebGL2.** The 3D pass uses reversed Z in a
  32-bit float depth buffer, which natively tells two surfaces 1 cm apart
  at 500 m. On WebGL2 (a browser without WebGPU) the GL backend remaps
  depth to GL's `[-1, 1]` range with no way to turn that off, which costs
  reversed Z its even spread of precision: the error grows with the square
  of the distance over the near plane, about 3 mm at 50 m with the default
  0.1 m `near`, about 30 cm at 500 m. Drawing is still correct — the
  nearer surface wins — only surfaces closer than that flicker against
  each other. A scene with far, close-together surfaces raises its
  `Camera3D`'s `near` (precision scales with it). WebGPU keeps the native
  precision. Pinned in the browser at 1 cm and 50 m by `shapes3d.mjs`.
- **3D's HDR target and MSAA degrade, never fail.** A 3D camera draws
  into `Rgba16Float`, 4× multisampled (`default.render.ron`'s `msaa`). On
  WebGL2 `Rgba16Float` is a render target only with the
  `EXT_color_buffer_float` (or `_half_float`) extension; without it the 3D
  shot draws into the frame's 8-bit sRGB format instead — light past 1
  clips before the tone curve, which still shapes what is under it — and
  where a format cannot be multisampled and resolved it draws one sample.
  Each is said once in the console. wgpu checks WebGL2 textures against
  what the backend reports rather than WebGPU's guarantees, and the
  engine asks the same (`TargetSupport`). Headless Chromium's SwiftShader
  has both: `mixed3d.mjs` draws the same picture on WebGPU and WebGL2.
- **Surface materials light the same on both.** The standard surface
  stays inside WebGL2's budgets, pinned by a test against
  `Limits::downlevel_webgl2_defaults`: 5 sampled textures of 16 (the plan
  keeps 12 for 0.5's materials, shadows and sky) through one sampler, two
  uniform blocks of 144 and 96 bytes, 5 of 15 inter-stage variables, the
  vertex attributes stage 135 pinned. A data texture (normal,
  metallic-roughness, occlusion) is read through a linear view; WebGL2
  cannot view an sRGB texture as linear, so there it is uploaded a second
  time as `Rgba8Unorm` (twice that image's memory) rather than read as
  colour. Alpha to coverage, which smooths a masked material's edge under
  MSAA, is core in WebGL2. `materials3d.mjs` requires the native frame's
  colours to the byte's neighbourhood on WebGPU and WebGL2 — a normal map
  read as sRGB would tilt the whole floor off its colour.
- **Custom surface materials compile for both** (stage 138): the
  composed module goes through naga to WGSL for WebGPU and to GLSL for
  WebGL2; group 1 binds 9 sampled textures (5 standard, 4 custom) and
  three uniform blocks a fragment stage. WebGL2 refuses a uniform binding
  whose type is not a multiple of 16 bytes
  (`DownlevelFlags::BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`, checked when the
  pipeline is made), so a custom surface's `struct Material` is aligned
  to 16 — a material of one `vec2` failed there and only there.
  A sprite material's struct is aligned the same way since. What naga
  cannot write as GLSL ES 300 (`textureNumLevels`, `pack4x8unorm`) is
  refused when the material loads, on every platform.
- **WebGPU holds WGSL's uniformity rule; naga does not.** A texture
  sampled inside a branch on a per-pixel value validates natively and
  runs on WebGL2, but Tint refuses the module. A custom surface's module
  and pipelines are made in an error scope — the `GPUDevice`'s own, since
  wgpu 30 panics converting a WebGPU error — and drawn only once it has
  answered; refused, the material draws its standard block and the
  console says why, once. Uncaptured GPU errors are logged, not a panic,
  through `onuncapturederror` on WebGPU.
- `custom3d.mjs` requires the native frame's colours for a data texture
  read unlit and a dissolve's rim on both, and a deliberately branchy
  material's standard block on WebGPU (its own colour on WebGL2).

## Checking a web build

`crates/voltra-player/web/test/` holds the tests CI runs (Node and
Playwright with a Chromium):

```sh
node crates/voltra-player/web/test/first-frame.mjs   # the sandbox draws, on WebGPU and WebGL2
node crates/voltra-player/web/test/play.mjs          # the platformer walks, takes a coin, sounds
node crates/voltra-player/web/test/shapes3d.mjs      # a 3D scene, and WebGL2's depth at 50 m
node crates/voltra-player/web/test/mixed3d.mjs       # 3D through AgX, a 2D overlay and the HUD over it
node crates/voltra-player/web/test/materials3d.mjs   # surface materials lit as natively, on both
node crates/voltra-player/web/test/custom3d.mjs      # custom surface materials, on both
```

Each builds through `cargo xtask dist --web` into `target/`, serves it and
drives it headless; `--skip-build` reuses the last build.
