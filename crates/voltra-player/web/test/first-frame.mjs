// The web player's end-to-end check: build the page, open it in Chromium,
// wait for the first frame, and find the sandbox's sprites on screen.
//
//   node crates/voltra-player/web/test/first-frame.mjs [--skip-build] [--out DIR]
//
// Needs `wasm-bindgen` (the CLI, the same version as the crate in
// Cargo.lock), Node and Playwright with a Chromium. Builds into DIR
// (default: target/web-test), serves it on a free local port, opens it on
// WebGPU and on WebGL2, and exits non-zero — printing the page's console —
// when either does not show the sprites.

import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { copyFileSync, mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const here = dirname(fileURLToPath(import.meta.url));
const workspace = resolve(here, "../../../..");
const args = process.argv.slice(2);
const skipBuild = args.includes("--skip-build");
const outAt = args.indexOf("--out");
const site = resolve(outAt >= 0 ? args[outAt + 1] : join(workspace, "target/web-test"));

function run(command, commandArgs) {
  console.log(`$ ${command} ${commandArgs.join(" ")}`);
  execFileSync(command, commandArgs, { cwd: workspace, stdio: "inherit" });
}

if (!skipBuild) {
  mkdirSync(site, { recursive: true });
  run("cargo", ["build", "--release", "-p", "voltra-player", "--target", "wasm32-unknown-unknown"]);
  const targetDir = process.env.CARGO_TARGET_DIR ?? join(workspace, "target");
  run("wasm-bindgen", [
    "--target", "web", "--no-typescript", "--out-dir", site,
    join(targetDir, "wasm32-unknown-unknown/release/voltra-player.wasm"),
  ]);
  run("cargo", ["xtask", "pack", "--out", join(site, "game.pak")]);
  copyFileSync(join(workspace, "crates/voltra-player/web/index.html"), join(site, "index.html"));
}
if (!existsSync(join(site, "index.html"))) {
  console.error(`no page at ${site}; run without --skip-build`);
  process.exit(1);
}

// The smallest static server that serves a wasm file with the type
// `WebAssembly.instantiateStreaming` insists on.
const types = {
  ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm",
  ".pak": "application/octet-stream",
};
const server = createServer((request, response) => {
  const path = decodeURIComponent(new URL(request.url, "http://x").pathname);
  if (path === "/probe") {
    response.writeHead(200, { "content-type": "text/html" });
    response.end("<!doctype html><canvas width=64 height=64></canvas>");
    return;
  }
  const file = normalize(join(site, path === "/" ? "index.html" : path));
  if (!file.startsWith(site + sep) || !existsSync(file)) {
    response.writeHead(404).end();
    return;
  }
  response.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream" });
  response.end(readFileSync(file));
});
await new Promise((ready) => server.listen(0, "127.0.0.1", ready));
const url = `http://127.0.0.1:${server.address().port}/`;

// Playwright from wherever it is installed: the project has no package.json.
const require = createRequire(import.meta.url);
let playwright;
try {
  playwright = require("playwright");
} catch {
  playwright = require(execFileSync("npm", ["root", "-g"]).toString().trim() + "/playwright");
}

// Both of the player's GPU paths, on SwiftShader so no GPU is needed.
// WebGPU presents through Vulkan: on ANGLE's GL path headless Chromium
// cannot back a WebGPU canvas and loses the device on its first frame.
// WebGL2 is what a browser without WebGPU gets, so it runs with WebGPU off.
const backends = [
  {
    name: "WebGPU",
    expect: "backend: BrowserWebGpu",
    // Whether this Chromium can show a WebGPU canvas at all, asked in plain
    // JavaScript before the player is: some builds headless cannot (the
    // swap chain's shared image is refused and the device lost), which is
    // the machine's limit, not the engine's.
    probe: true,
    args: [
      "--enable-unsafe-webgpu", "--enable-features=Vulkan", "--use-vulkan=swiftshader",
      "--use-angle=vulkan", "--enable-unsafe-swiftshader",
    ],
  },
  {
    name: "WebGL2",
    expect: "backend: Gl",
    args: ["--enable-unsafe-swiftshader", "--use-angle=swiftshader", "--disable-features=WebGPU"],
  },
];

const failures = [];
for (const backend of backends) {
  if (backend.probe && !(await canPresentWebGpu(backend))) {
    console.log(`\n== ${backend.name}: SKIPPED — this Chromium cannot present a WebGPU canvas headless`);
    continue;
  }
  const failure = await check(backend);
  if (failure) failures.push(`${backend.name}: ${failure}`);
}
server.close();
if (failures.length > 0) {
  console.error(`FAIL\n  ${failures.join("\n  ")}`);
  process.exit(1);
}
console.log("PASS");

// Clears a WebGPU canvas a few frames running; false when there is no
// adapter or the device is lost doing it.
async function canPresentWebGpu(backend) {
  const browser = await playwright.chromium.launch({ args: backend.args });
  try {
    const page = await browser.newPage();
    await page.goto(`${url}probe`);
    return await page.evaluate(async () => {
      const adapter = await navigator.gpu?.requestAdapter();
      if (!adapter) return false;
      const device = await adapter.requestDevice();
      let lost = false;
      device.lost.then(() => { lost = true; });
      const context = document.querySelector("canvas").getContext("webgpu");
      context.configure({ device, format: navigator.gpu.getPreferredCanvasFormat() });
      for (let i = 0; i < 5; i++) {
        const encoder = device.createCommandEncoder();
        encoder.beginRenderPass({
          colorAttachments: [{
            view: context.getCurrentTexture().createView(),
            loadOp: "clear", storeOp: "store", clearValue: { r: 1, g: 0, b: 0, a: 1 },
          }],
        }).end();
        device.queue.submit([encoder.finish()]);
        await new Promise((frame) => requestAnimationFrame(frame));
      }
      await new Promise((settle) => setTimeout(settle, 300));
      return !lost;
    });
  } catch {
    return false;
  } finally {
    await browser.close();
  }
}

// Opens the page on one backend; what went wrong, or null.
async function check(backend) {
  console.log(`\n== ${backend.name}`);
  const browser = await playwright.chromium.launch({ args: backend.args });
  const page = await browser.newPage({ viewport: { width: 960, height: 540 } });
  const consoleLines = [];
  page.on("console", (message) => consoleLines.push(`[${message.type()}] ${message.text()}`));
  page.on("pageerror", (error) => consoleLines.push(`[pageerror] ${error.message}`));

  let failure = null;
  try {
    await page.goto(url);
    await page.waitForSelector("canvas", { timeout: 60_000 });
    const found = await waitForSprites(page, 60_000);
    if (found.ok) {
      console.log(`sprites on screen: ${JSON.stringify(found.counts)}`);
    } else {
      failure = `the sandbox's sprites were not on screen: ${JSON.stringify(found.counts)}`;
    }
  } catch (error) {
    failure = String(error);
  }

  const shot = join(site, `first-frame-${backend.name.toLowerCase()}.png`);
  writeFileSync(shot, await page.screenshot());
  await browser.close();
  console.log(`page console:\n  ${consoleLines.join("\n  ")}`);
  console.log(`screenshot: ${shot}`);

  const errors = consoleLines.filter((line) => line.startsWith("[error]") || line.startsWith("[pageerror]"));
  if (!failure && errors.length > 0) {
    failure = `the page logged ${errors.length} error(s)`;
  }
  if (!failure && !consoleLines.some((line) => line.includes(backend.expect))) {
    failure = `the adapter is not on ${backend.name} (no "${backend.expect}" in the log)`;
  }
  return failure;
}

// Screenshots the page until it shows the sandbox: the orange crate and the
// grey floor, each as a block of pixels rather than a stray few. A scene's
// colours are linear and the frame is sRGB, so each is looked for encoded —
// what a native window shows too — within a small tolerance.
async function waitForSprites(page, timeout) {
  const deadline = Date.now() + timeout;
  let counts = {};
  while (Date.now() < deadline) {
    const png = await page.screenshot();
    counts = await countColours(page, png.toString("base64"), {
      // `sandbox.ron`'s second crate and its floor.
      orange: encode([1.0, 0.55, 0.3]),
      floor: encode([0.25, 0.27, 0.32]),
    });
    if (counts.orange > 400 && counts.floor > 2000) {
      return { ok: true, counts };
    }
    await page.waitForTimeout(500);
  }
  return { ok: false, counts };
}

// Linear colour to the 8-bit sRGB a frame stores.
function encode(linear) {
  return linear.map((c) => {
    const s = c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055;
    return Math.round(s * 255);
  });
}

// Decodes the screenshot in the page itself, which already has a PNG decoder.
function countColours(page, base64, colours) {
  return page.evaluate(async ({ data, colours }) => {
    const image = new Image();
    image.src = `data:image/png;base64,${data}`;
    await image.decode();
    const canvas = new OffscreenCanvas(image.width, image.height);
    const context = canvas.getContext("2d");
    context.drawImage(image, 0, 0);
    const pixels = context.getImageData(0, 0, image.width, image.height).data;
    const counts = Object.fromEntries(Object.keys(colours).map((name) => [name, 0]));
    for (let i = 0; i < pixels.length; i += 4) {
      for (const [name, [r, g, b]] of Object.entries(colours)) {
        const near = Math.abs(pixels[i] - r) <= 6 && Math.abs(pixels[i + 1] - g) <= 6
          && Math.abs(pixels[i + 2] - b) <= 6;
        if (near) counts[name] += 1;
      }
    }
    return { ...counts, width: image.width, height: image.height };
  }, { data: base64, colours });
}
