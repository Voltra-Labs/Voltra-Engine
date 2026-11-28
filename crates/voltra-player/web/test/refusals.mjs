// What a GPU refuses, and what is drawn instead, in the browser: `cargo
// xtask dist --web` of fixtures/refusals.build.ron — a web-test fixture,
// not a sample — on WebGPU and on WebGL2. A sphere and a sprite bar whose
// shaders sample a texture inside a branch on a per-pixel value: naga
// (native) and WebGL2's GLSL run them; WebGPU's Tint refuses the modules,
// and the engine must then draw the sphere's standard block (flat red)
// and the bar plainly (its orange tint) — never a black frame or a lost
// sprite pass — while the plain sphere and bar beside them draw as ever.
// Colours are the native frame's (crates/voltra-core/tests/refusals.rs).
//
//   node crates/voltra-player/web/test/refusals.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/web-refusals). See harness.mjs for what
// it needs installed.

import { build, onEveryBackend, options, serve, waitFor } from "./harness.mjs";

const { skipBuild, out } = options("web-refusals");
const site = build({
  out,
  manifest: "crates/voltra-player/web/test/fixtures/refusals.build.ron",
  skipBuild,
});

const server = await serve(site);
const colours = {
  // The branchy sphere: its own yellow, or its standard block's red.
  sphereOwn: [202, 194, 119],
  sphereStandard: [204, 100, 91],
  // The plain sphere, unlit green through AgX.
  spherePlain: [114, 198, 138],
  // The branchy bar: its own blue, or plainly its orange tint.
  barOwn: [89, 149, 255],
  barPlain: [255, 188, 89],
  // The plain bar, cyan.
  barCyan: [89, 243, 255],
};
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "refusals",
    check: async (page, backend) => {
      const refused = backend.name === "WebGPU";
      const [sphere, sphereNot] = refused
        ? ["sphereStandard", "sphereOwn"]
        : ["sphereOwn", "sphereStandard"];
      const [bar, barNot] = refused ? ["barPlain", "barOwn"] : ["barOwn", "barPlain"];
      const { ok, found } = await waitFor(
        page,
        colours,
        (f) => f[sphere].count > 4_000 && f[sphereNot].count < 50
          && f[bar].count > 4_000 && f[barNot].count < 50
          && f.spherePlain.count > 4_000 && f.barCyan.count > 4_000
          // Where the fixture puts them: the branchy ones left, the plain
          // ones right, the bars below the spheres.
          && f[sphere].x < 480 && f.spherePlain.x > 480
          && f[bar].x < 480 && f.barCyan.x > 480 && f[bar].y > f[sphere].y,
      );
      console.log(`on screen: ${JSON.stringify(found)}`);
      return ok ? null : `the ${sphere} sphere, the ${bar} bar or the plain ones did not draw`;
    },
  });
} finally {
  server.close();
}
