// The web player's first frame: `cargo xtask dist --web` of build.ron,
// open the page on WebGPU and on WebGL2, find the sandbox's sprites, and
// find them again after the page is resized.
//
//   node crates/voltra-player/web/test/first-frame.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/web-test). See harness.mjs for what it
// needs installed.

import { build, encode, onEveryBackend, options, serve, waitFor } from "./harness.mjs";

const { skipBuild, out } = options("web-test");
const site = build({ out, manifest: "build.ron", skipBuild });

const server = await serve(site);
// `sandbox.ron`'s second crate and its floor, as the frame stores them.
const colours = { orange: encode([1.0, 0.55, 0.3]), floor: encode([0.25, 0.27, 0.32]) };
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "first-frame",
    check: async (page) => {
      const { ok, found } = await waitFor(
        page,
        colours,
        (f) => f.orange.count > 400 && f.floor.count > 2000,
      );
      console.log(`on screen: ${JSON.stringify(found)}`);
      if (!ok) return "the sandbox's sprites were not on screen";

      // The page resized, as a dragged window is: the canvas follows it,
      // the frame is rebuilt at each size, and the scene is still drawn at
      // the last one — the floor across the new width.
      for (const [width, height] of [[1280, 720], [640, 480], [1100, 600], [800, 450]]) {
        await page.setViewportSize({ width, height });
        await page.waitForTimeout(150);
      }
      const resized = await waitFor(
        page,
        colours,
        (f) => f.orange.count > 200 && f.floor.count > 1000 && f.floor.x > 350 && f.floor.x < 450,
        20_000,
      );
      console.log(`after resizing to 800x450: ${JSON.stringify(resized.found)}`);
      return resized.ok ? null : "the scene was not redrawn at the page's new size";
    },
  });
} finally {
  server.close();
}
