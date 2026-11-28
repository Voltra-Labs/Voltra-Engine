// Lights and the environment in the browser: `cargo xtask dist --web` of
// lights3d.build.ron, opened on WebGPU and on WebGL2 — red, green and blue
// point lights on a wall, a spot light's cone, two suns, a sphere among
// ten small lights, pillars fading into fog toward a gradient sky. Each
// mesh is lit by the eight lights its instance attribute names (a
// `Uint32x3`, read flat), out of one uniform array of 128 point and spot
// lights — the 6 288-byte block must bind on WebGL2, whose blocks stop at
// 16 KiB, and its indexing must run in GLSL ES 300. The sky is drawn
// behind the geometry at the far plane, the fog tinted by it. The colours
// below are what the native headless frame draws
// (crates/voltra-core/tests/lights3d.rs's scene at this size).
//
//   node crates/voltra-player/web/test/lights3d.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/web-lights3d). See harness.mjs for what
// it needs installed.

import { build, onEveryBackend, options, serve, waitFor } from "./harness.mjs";

const { skipBuild, out } = options("web-lights3d");
const site = build({ out, manifest: "lights3d.build.ron", skipBuild });

const server = await serve(site);
const colours = {
  // Each lamp's glow on the back wall, through the fog and AgX.
  red: [193, 137, 124],
  green: [149, 196, 149],
  blue: [143, 159, 196],
  // The gradient sky over the top of the picture.
  sky: [173, 162, 161],
  // A texture that would not load.
  magenta: [255, 0, 255],
};
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "lights3d",
    check: async (page) => {
      const { ok, found } = await waitFor(
        page,
        colours,
        (f) => f.red.count > 400 && f.green.count > 280 && f.blue.count > 240
          && f.sky.count > 60_000
          // Where the scene puts them: the lamps left to right along the
          // wall, the sky above the floor.
          && f.red.x < 260 && f.green.x > 260 && f.green.x < 340
          && f.blue.x > 340 && f.blue.x < 420 && f.sky.y < 150,
      );
      console.log(`on screen: ${JSON.stringify(found)}`);
      if (!ok) return "the lamps' glows or the sky did not draw as natively";
      if (found.magenta.count > 0) return `${found.magenta.count} pixels of a missing texture`;
      return null;
    },
  });
} finally {
  server.close();
}
