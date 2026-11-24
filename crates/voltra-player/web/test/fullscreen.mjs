// Fullscreen from a tick, in the browser: the platformer built for the
// web, its HUD's Fullscreen button clicked, and the canvas is the page's
// fullscreen element; clicked again, and it is not. On WebGPU and WebGL2.
//
//   node crates/voltra-player/web/test/fullscreen.mjs [--skip-build] [--out DIR]
//
// What it pins (ARCHITECTURE.md, "A game's window requests are carried out
// between frames"): `tick.window.set_fullscreen` reaches the Fullscreen API
// through winit, a frame after the click — within the transient activation
// the click gave the page, which is what the browser asks before it grants
// fullscreen.
//
// Builds into DIR (default: target/web-fullscreen). See harness.mjs for what
// it needs installed.

import { build, encode, onEveryBackend, options, serve, waitFor } from "./harness.mjs";

const { skipBuild, out } = options("web-fullscreen");
const site = build({ out, manifest: "platformer.build.ron", skipBuild });

const colours = { crates: encode([0.85, 0.55, 0.3]) };
// The middle of the HUD's Fullscreen button (`assets/scenes/platformer.ron`:
// 134..264 across, 68..102 down, in the canvas's pixels).
const BUTTON = { x: 199, y: 85 };

const fullscreenElement = (page) =>
  page.evaluate(() => document.fullscreenElement?.tagName ?? null);

async function until(page, wanted, timeout = 5000) {
  const deadline = Date.now() + timeout;
  let now = await fullscreenElement(page);
  while (now !== wanted && Date.now() < deadline) {
    await page.waitForTimeout(100);
    now = await fullscreenElement(page);
  }
  return now;
}

const server = await serve(site);
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "fullscreen",
    check: async (page) => {
      const first = await waitFor(page, colours, (f) => f.crates.count > 300);
      if (!first.ok) return `the level was not drawn: ${JSON.stringify(first.found)}`;
      await page.waitForTimeout(500);
      if ((await fullscreenElement(page)) !== null) return "fullscreen before any click";

      await page.mouse.click(BUTTON.x, BUTTON.y);
      const entered = await until(page, "CANVAS");
      console.log(`after the click, the fullscreen element is ${entered}`);
      if (entered !== "CANVAS") return "the canvas did not go fullscreen after the click";

      // Fullscreen, the canvas fills the viewport and the HUD stays at its
      // corner: the same button, clicked again, leaves.
      await page.waitForTimeout(500);
      await page.mouse.click(BUTTON.x, BUTTON.y);
      const left = await until(page, null);
      console.log(`after the second click, the fullscreen element is ${left}`);
      if (left !== null) return "the canvas did not leave fullscreen after the second click";
      return null;
    },
  });
} finally {
  server.close();
}
