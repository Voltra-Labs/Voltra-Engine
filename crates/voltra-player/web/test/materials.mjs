// Sprite materials in the browser: `cargo xtask dist --web` of
// materials.build.ron, opened on WebGPU and on WebGL2 — the hit-flashed
// coin drawn white and the outlined one ringed in green, through each
// backend's own compile of the material's WGSL.
//
//   node crates/voltra-player/web/test/materials.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/web-materials). See harness.mjs for what
// it needs installed.

import { build, encode, onEveryBackend, options, serve, waitFor } from "./harness.mjs";

const { skipBuild, out } = options("web-materials");
const site = build({ out, manifest: "materials.build.ron", skipBuild });

const server = await serve(site);
// The flash's colour, and the outline's (outline.material.ron).
const colours = { white: encode([1.0, 1.0, 1.0]), green: encode([0.1, 1.0, 0.2]) };
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "materials",
    check: async (page) => {
      const { ok, found } = await waitFor(
        page,
        colours,
        // The flashed coin is in the middle, the outlined one right of it.
        (f) => f.white.count > 1000 && f.white.x > 400 && f.white.x < 560
          && f.green.count > 100 && f.green.x > 560,
      );
      console.log(`on screen: ${JSON.stringify(found)}`);
      return ok ? null : "the flashed coin or the outline was not drawn";
    },
  });
} finally {
  server.close();
}
