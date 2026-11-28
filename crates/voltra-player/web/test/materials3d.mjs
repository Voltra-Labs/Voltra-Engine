// Standard surface materials in the browser: `cargo xtask dist --web` of
// materials3d.build.ron, opened on WebGPU and on WebGL2 — spheres lit by
// Filament's BRDF under one directional light, an unlit sphere, a floor
// bent by a normal map, a grate cut out by its alpha and a glass pane
// blended over a box. The colours below are what the native headless
// frame draws (crates/voltra-core/tests/materials3d.rs's scene at this
// size), so both backends must light exactly as lavapipe does: a normal
// map decoded from sRGB on WebGL2's second upload path would tilt the
// whole floor off its colour, and a base colour decoded twice would darken
// the unlit sphere.
//
//   node crates/voltra-player/web/test/materials3d.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/web-materials3d). See harness.mjs for
// what it needs installed.

import { build, onEveryBackend, options, serve, waitFor } from "./harness.mjs";

const { skipBuild, out } = options("web-materials3d");
const site = build({ out, manifest: "materials3d.build.ron", skipBuild });

const server = await serve(site);
const colours = {
  // The floor where its normal map is flat, (128, 128, 255) read as data.
  floor: [153, 155, 158],
  // The unlit sphere: its linear (0.2, 0.9, 0.4) through AgX.
  unlit: [140, 200, 166],
  // The glass pane over the background, and over the red box.
  glassOverSky: [127, 135, 142],
  glassOverBox: [191, 143, 141],
  // A texture that would not load.
  magenta: [255, 0, 255],
};
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "materials3d",
    check: async (page) => {
      const { ok, found } = await waitFor(
        page,
        colours,
        (f) => f.floor.count > 100_000 && f.unlit.count > 8_000
          && f.glassOverSky.count > 12_000 && f.glassOverBox.count > 1_200
          // Where the scene puts them: the unlit sphere right of the
          // middle, the glass pane right, both above the floor's mean.
          && f.unlit.x > 640 && f.glassOverBox.x > 560 && f.glassOverBox.y < f.floor.y,
      );
      console.log(`on screen: ${JSON.stringify(found)}`);
      if (!ok) return "the floor, the unlit sphere or the glass did not draw as natively";
      if (found.magenta.count > 0) return `${found.magenta.count} pixels of a missing texture`;
      return null;
    },
  });
} finally {
  server.close();
}
