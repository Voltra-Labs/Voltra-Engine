// One frame of both worlds in the browser: `cargo xtask dist --web` of
// mixed3d.build.ron, opened on WebGPU and on WebGL2 — the 3D world drawn
// into its HDR target (Rgba16Float where the browser can draw it, the
// frame's sRGB format where WebGL2 cannot), multisampled, through the AgX
// curve; the 2D camera's banner composited over it untouched by the curve;
// the HUD over both.
//
//   node crates/voltra-player/web/test/mixed3d.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/web-mixed3d). See harness.mjs for what
// it needs installed.

import { build, encode, onEveryBackend, options, serve, waitFor } from "./harness.mjs";

const { skipBuild, out } = options("web-mixed3d");
const site = build({ out, manifest: "mixed3d.build.ron", skipBuild });

const server = await serve(site);
const colours = {
  // Through AgX: the bytes crates/voltra-core/tests/scene3d.rs pins.
  cube: [206, 125, 110],
  sphere: [114, 198, 138],
  // The scene's linear colours, encoded: 2D is never tone mapped.
  banner: encode([1.0, 0.6, 0.1]),
  hud: encode([1.0, 0.0, 1.0]),
  // Skipping the curve, the cube would be this.
  rawCube: encode([0.9, 0.15, 0.1]),
};
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "mixed3d",
    check: async (page) => {
      const { ok, found } = await waitFor(
        page,
        colours,
        (f) => f.cube.count > 3_000 && f.sphere.count > 3_000 && f.hud.count > 7_000
          // 6 × 1 units through a size-5 camera on 540 pixels: 324 × 54.
          && f.banner.count > 15_000
          && f.cube.x < f.sphere.x && f.banner.y > f.cube.y && f.hud.y < f.cube.y,
      );
      console.log(`on screen: ${JSON.stringify(found)}`);
      if (!ok) return "the 3D meshes, the 2D banner or the HUD were not drawn where they stand";
      if (found.rawCube.count > 0) return `${found.rawCube.count} cube pixels skipped the tone curve`;
      return null;
    },
  });
} finally {
  server.close();
}
