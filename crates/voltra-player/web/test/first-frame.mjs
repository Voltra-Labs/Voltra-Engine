// The web player's first frame: `cargo xtask dist --web` of build.ron,
// open the page on WebGPU and on WebGL2, and find the sandbox's sprites.
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
      return ok ? null : "the sandbox's sprites were not on screen";
    },
  });
} finally {
  server.close();
}
