// The first 3D scene in the browser: `cargo xtask dist --web` of
// shapes3d.build.ron, opened on WebGPU and on WebGL2 — the floor and the
// four built-in meshes drawn through the depth pass, and two walls 1 cm
// apart at 50 m where the far one never shows through. On WebGL2 the GL
// depth remap costs reversed-Z its precision (ARCHITECTURE "The 3D
// pass"); 1 cm at 50 m is what it still resolves with the 0.1 m near plane.
//
//   node crates/voltra-player/web/test/shapes3d.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/web-shapes3d). See harness.mjs for what
// it needs installed.

import { build, encode, onEveryBackend, options, serve, waitFor } from "./harness.mjs";

const { skipBuild, out } = options("web-shapes3d");
const site = build({ out, manifest: "shapes3d.build.ron", skipBuild });

const server = await serve(site);
// The colours scenes/shapes3d.ron gives each mesh, linear.
const colours = {
  floor: encode([0.35, 0.37, 0.4]),
  cube: encode([0.9, 0.15, 0.1]),
  sphere: encode([0.1, 0.8, 0.2]),
  capsule: encode([0.15, 0.3, 0.95]),
  cylinder: encode([0.95, 0.8, 0.1]),
  wallFront: encode([0.9, 0.9, 0.9]),
  wallBehind: encode([0.0, 0.9, 0.9]),
};
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "shapes3d",
    check: async (page) => {
      const { ok, found } = await waitFor(
        page,
        colours,
        (f) => f.floor.count > 80_000 && f.cube.count > 2_000 && f.sphere.count > 2_000
          && f.capsule.count > 2_000 && f.cylinder.count > 2_000 && f.wallFront.count > 10_000
          // Left to right as the scene places them.
          && f.cube.x < f.sphere.x && f.sphere.x < f.capsule.x && f.capsule.x < f.cylinder.x,
      );
      console.log(`on screen: ${JSON.stringify(found)}`);
      if (!ok) return "the floor, a mesh or the near wall was not drawn where it stands";
      if (found.wallBehind.count > 0) {
        return `the wall 1 cm behind showed through in ${found.wallBehind.count} pixels`;
      }
      return null;
    },
  });
} finally {
  server.close();
}
