// Touch in the browser: the platformer built for the web, opened on a
// touch screen on WebGPU and on WebGL2, its on-screen stick — the
// bottom-left corner, `Touch(BottomLeft, Right)` in default.input.ron —
// held and slid right by a finger, and the walker walks.
//
//   node crates/voltra-player/web/test/touch.mjs [--skip-build] [--out DIR]
//
// The finger is Chromium's own touch input (DevTools' Input.dispatchTouchEvent),
// which reaches the canvas as pointer events of type "touch", as a phone's
// does. Builds into DIR (default: target/web-touch).

import { build, encode, onEveryBackend, options, serve, waitFor, measure } from "./harness.mjs";

const { skipBuild, out } = options("web-touch");
const site = build({ out, manifest: "platformer.build.ron", skipBuild });

// The crates left of the start: walking right moves them left on screen,
// as the camera follows the walker.
const colours = { crates: encode([0.85, 0.55, 0.3]) };

const server = await serve(site);
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "touch",
    pageOptions: { hasTouch: true, isMobile: false },
    check: async (page) => {
      const first = await waitFor(page, colours, (f) => f.crates.count > 300);
      if (!first.ok) return `the level was not drawn: ${JSON.stringify(first.found)}`;
      await page.waitForTimeout(1500);
      const before = await measure(page, colours);

      // A thumb down in the stick's corner, slid right past the stick's
      // radius (540 / 12 = 45 pixels), held, then lifted.
      const cdp = await page.context().newCDPSession(page);
      const finger = (type, x, y) =>
        cdp.send("Input.dispatchTouchEvent", {
          type,
          touchPoints: type === "touchEnd" ? [] : [{ x, y, id: 1 }],
        });
      await finger("touchStart", 150, 440);
      for (let x = 150; x <= 230; x += 10) {
        await finger("touchMove", x, 440);
        await page.waitForTimeout(20);
      }
      // Moves keep the finger alive while the walker walks.
      for (let i = 0; i < 12; i++) {
        await page.waitForTimeout(100);
        await finger("touchMove", 230, 440 + (i % 2));
      }
      await finger("touchEnd", 230, 440);
      await page.waitForTimeout(800);

      const after = await measure(page, colours);
      console.log(`crates x: ${before.crates.x} → ${after.crates.x}`);
      if (after.crates.count > 0 && !(after.crates.x < before.crates.x - 40)) {
        return "the walker did not walk right under the finger";
      }
      return null;
    },
  });
} finally {
  server.close();
}
