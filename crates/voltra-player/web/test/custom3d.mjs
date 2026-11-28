// Custom surface materials in the browser: `cargo xtask dist --web` of
// custom3d.build.ron, opened on WebGPU and on WebGL2 — a floor tinted by
// world position, a sphere dissolving into a noise texture with a glowing
// cut, stripes sliding across another, and a sphere showing a data
// texture's bytes unlit. Each is a `surface()` of the game's own, composed
// around the engine's vertex stage and lighting, so both backends must
// compile the composed module (WebGL2 through naga's GLSL) and bind the
// custom uniform block and textures. The colours below are what the native
// headless frame draws (crates/voltra-core/tests/custom3d.rs's scene): the
// bands are the stripes' bytes read as data through AgX — read as sRGB on
// WebGL2's upload path they would be other colours entirely. The branchy
// sphere's shader breaks WGSL's uniformity rule, which naga lifts and Tint
// holds: on WebGPU it must show its standard block, not a black frame.
//
//   node crates/voltra-player/web/test/custom3d.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/web-custom3d). See harness.mjs for what
// it needs installed.

import { build, onEveryBackend, options, serve, waitFor } from "./harness.mjs";

const { skipBuild, out } = options("web-custom3d");
const site = build({ out, manifest: "custom3d.build.ron", skipBuild });

const server = await serve(site);
const colours = {
  // The bands sphere: stripes.png's first and last bands as data, unlit,
  // through AgX.
  bandRed: [202, 155, 143],
  bandBlue: [145, 169, 198],
  // The dissolve's glowing rim, where its noise is just above the cut.
  rim: [236, 204, 164],
  // The branchy sphere, whose shader samples a texture inside a branch:
  // its own yellow where it runs (native, WebGL2), its standard block's
  // flat red where the GPU refuses it (WebGPU's Tint) — never black.
  branchyOwn: [202, 194, 119],
  branchyStandard: [204, 100, 91],
  // A texture that would not load.
  magenta: [255, 0, 255],
};
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "custom3d",
    check: async (page, backend) => {
      // WebGPU refuses the branchy shader and draws its standard block;
      // WebGL2's GLSL has no such rule and runs it, as native does.
      const refused = backend.name === "WebGPU";
      const branchy = refused ? "branchyStandard" : "branchyOwn";
      const other = refused ? "branchyOwn" : "branchyStandard";
      const { ok, found } = await waitFor(
        page,
        colours,
        (f) => f.bandRed.count > 2_000 && f.bandBlue.count > 2_000 && f.rim.count > 30
          && f[branchy].count > 2_000 && f[other].count < 50
          // Where the scene puts them: the bands sphere right of the
          // middle, the dissolve left, the branchy one in the middle.
          && f.bandBlue.x > 640 && f.bandRed.x > 640 && f.rim.x < 320
          && Math.abs(f[branchy].x - 480) < 40,
      );
      console.log(`on screen: ${JSON.stringify(found)}`);
      if (!ok) return `the bands, the dissolve or the branchy sphere (${branchy}) did not draw as expected`;
      if (found.magenta.count > 0) return `${found.magenta.count} pixels of a missing texture`;
      return null;
    },
  });
} finally {
  server.close();
}
