// What the web tests share: building a page, serving it, opening it in
// Chromium on each GPU backend, and reading colours off a screenshot.
//
// Needs `wasm-bindgen` (the CLI, the same version as the crate in
// Cargo.lock), Node and Playwright with a Chromium.

import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { readdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const here = dirname(fileURLToPath(import.meta.url));
export const workspace = resolve(here, "../../../..");

/// `--skip-build` and `--out DIR` from the command line, with `name` the
/// default folder under `target/`: where `cargo xtask dist --web` writes.
export function options(name) {
  const args = process.argv.slice(2);
  const outAt = args.indexOf("--out");
  return {
    skipBuild: args.includes("--skip-build"),
    out: resolve(outAt >= 0 ? args[outAt + 1] : join(workspace, "target", name)),
  };
}

function run(command, commandArgs) {
  console.log(`$ ${command} ${commandArgs.join(" ")}`);
  execFileSync(command, commandArgs, { cwd: workspace, stdio: "inherit" });
}

/// The web build of `manifest` under `out`, made with `cargo xtask dist
/// --web` — the command a person ships with — unless `skipBuild`; the
/// folder it is in either way.
export function build({ out, manifest, skipBuild }) {
  if (!skipBuild) {
    run("cargo", ["xtask", "dist", "--web", "--manifest", join(workspace, manifest), "--out", out]);
  }
  const folders = existsSync(out) ? readdirSync(out).filter((name) => name.endsWith("-web")) : [];
  if (folders.length !== 1) {
    throw new Error(`expected one web build in ${out}, found ${folders.length}; run without --skip-build`);
  }
  return join(out, folders[0]);
}

/// Serves `site` on a free local port, plus `/probe`, a bare canvas.
/// The smallest static server that gives a wasm file the type
/// `WebAssembly.instantiateStreaming` insists on.
export async function serve(site) {
  if (!existsSync(join(site, "index.html"))) {
    throw new Error(`no page at ${site}; run without --skip-build`);
  }
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
  return { url: `http://127.0.0.1:${server.address().port}/`, close: () => server.close() };
}

/// Playwright from wherever it is installed: the project has no package.json.
export function playwright() {
  const require = createRequire(import.meta.url);
  try {
    return require("playwright");
  } catch {
    return require(execFileSync("npm", ["root", "-g"]).toString().trim() + "/playwright");
  }
}

// Both GPU paths, on SwiftShader so no GPU is needed. WebGPU presents
// through Vulkan: on ANGLE's GL path headless Chromium cannot back a WebGPU
// canvas and loses the device on its first frame. WebGL2 is what a browser
// without WebGPU gets, so it runs with WebGPU off.
export const BACKENDS = [
  {
    name: "WebGPU",
    expect: "backend: BrowserWebGpu",
    // Whether this Chromium can show a WebGPU canvas at all, asked in plain
    // JavaScript before the game is: some builds headless cannot (the swap
    // chain's shared image is refused and the device lost), which is the
    // machine's limit, not the engine's.
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

/// Runs `check(page, backend)` on every backend the machine can show and
/// exits: non-zero, with each page's console, when one failed. `check`
/// returns what went wrong, or null. A page that logs an error, or whose
/// adapter is not the backend's, fails too.
export async function onEveryBackend({ site, url, name, initScript, check, pageOptions = {} }) {
  const pw = playwright();
  const failures = [];
  for (const backend of BACKENDS) {
    if (backend.probe && !(await canPresentWebGpu(pw, backend, url))) {
      console.log(`\n== ${backend.name}: SKIPPED — this Chromium cannot present a WebGPU canvas headless`);
      continue;
    }
    console.log(`\n== ${backend.name}`);
    const browser = await pw.chromium.launch({ args: backend.args });
    const page = await browser.newPage({ viewport: { width: 960, height: 540 }, ...pageOptions });
    if (initScript) await page.addInitScript(initScript);
    const consoleLines = [];
    page.on("console", (message) => consoleLines.push(`[${message.type()}] ${message.text()}`));
    page.on("pageerror", (error) => consoleLines.push(`[pageerror] ${error.message}`));

    let failure = null;
    try {
      await page.goto(url);
      await page.waitForSelector("canvas", { timeout: 60_000 });
      failure = await check(page, backend);
    } catch (error) {
      failure = String(error);
    }
    const shot = join(site, `${name}-${backend.name.toLowerCase()}.png`);
    writeFileSync(shot, await page.screenshot());
    await browser.close();
    console.log(`page console:\n  ${consoleLines.join("\n  ")}`);
    console.log(`screenshot: ${shot}`);

    const errors = consoleLines.filter((l) => l.startsWith("[error]") || l.startsWith("[pageerror]"));
    if (!failure && errors.length > 0) failure = `the page logged ${errors.length} error(s)`;
    if (!failure && !consoleLines.some((line) => line.includes(backend.expect))) {
      failure = `the adapter is not on ${backend.name} (no "${backend.expect}" in the log)`;
    }
    if (failure) failures.push(`${backend.name}: ${failure}`);
  }
  if (failures.length > 0) {
    console.error(`FAIL\n  ${failures.join("\n  ")}`);
    process.exit(1);
  }
  console.log("PASS");
}

// Clears a WebGPU canvas a few frames running; false when there is no
// adapter or the device is lost doing it.
async function canPresentWebGpu(pw, backend, url) {
  const browser = await pw.chromium.launch({ args: backend.args });
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

/// Linear colour to the 8-bit sRGB a frame stores: a scene's colours are
/// linear and the frame is sRGB, as in a native window.
export function encode(linear) {
  return linear.map((c) => {
    const s = c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055;
    return Math.round(s * 255);
  });
}

/// For each of `colours` (name → sRGB triple), how many pixels of a fresh
/// screenshot are within 6 of it, and the mean x of those pixels. Decoded in
/// the page itself, which already has a PNG decoder.
export async function measure(page, colours) {
  const png = await page.screenshot();
  return page.evaluate(async ({ data, colours }) => {
    const image = new Image();
    image.src = `data:image/png;base64,${data}`;
    await image.decode();
    const canvas = new OffscreenCanvas(image.width, image.height);
    const context = canvas.getContext("2d");
    context.drawImage(image, 0, 0);
    const pixels = context.getImageData(0, 0, image.width, image.height).data;
    const found = Object.fromEntries(Object.keys(colours).map((name) => [name, { count: 0, x: 0 }]));
    for (let i = 0; i < pixels.length; i += 4) {
      for (const [name, [r, g, b]] of Object.entries(colours)) {
        const near = Math.abs(pixels[i] - r) <= 6 && Math.abs(pixels[i + 1] - g) <= 6
          && Math.abs(pixels[i + 2] - b) <= 6;
        if (near) {
          found[name].count += 1;
          found[name].x += (i / 4) % image.width;
        }
      }
    }
    for (const entry of Object.values(found)) {
      entry.x = entry.count > 0 ? Math.round(entry.x / entry.count) : null;
    }
    return found;
  }, { data: png.toString("base64"), colours });
}

/// Measures until `ready(found)` holds or `timeout` ms pass; the last
/// measurement either way, and whether it held.
export async function waitFor(page, colours, ready, timeout = 60_000) {
  const deadline = Date.now() + timeout;
  let found = {};
  while (Date.now() < deadline) {
    found = await measure(page, colours);
    if (ready(found)) return { ok: true, found };
    await page.waitForTimeout(500);
  }
  return { ok: false, found };
}
