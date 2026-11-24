// Rebound controls kept in the browser: the platformer built for the web,
// its jump rebound to J (F1, then the key), the page reloaded, and J jumps
// the walker while Space — the old jump — does not. On WebGPU and WebGL2.
//
//   node crates/voltra-player/web/test/rebind.mjs [--skip-build] [--out DIR]
//
// What it pins (ARCHITECTURE.md, "Rebound controls are saved through the
// saves storage"): `Tick::save_controls` reaches the page's localStorage,
// under the key the saves folder rules give it, and the loop lays the
// saved controls over the game's map when the page starts again.
//
// Builds into DIR (default: target/web-rebind). See harness.mjs for what it
// needs installed.

import { build, encode, onEveryBackend, options, serve, waitFor, measure } from "./harness.mjs";

const { skipBuild, out } = options("web-rebind");
const site = build({ out, manifest: "platformer.build.ron", skipBuild });

// The walker, as the frame stores its colour. The camera follows it
// sideways only, so a jump moves it up the screen.
const colours = { walker: encode([0.35, 0.75, 1.0]), crates: encode([0.85, 0.55, 0.3]) };
// `/<game>/saves/controls.input.ron`: the saves folder's rules on the web.
const KEY = "/Voltra Platformer/saves/controls.input.ron";

async function settled(page) {
  const first = await waitFor(page, colours, (f) => f.crates.count > 300 && f.walker.count > 200);
  if (!first.ok) return `the level was not drawn: ${JSON.stringify(first.found)}`;
  // The walker has landed.
  await page.waitForTimeout(1500);
  return null;
}

/// How far up the screen the walker went while `key` was held: the
/// highest it reached against where it stood, in pixels.
async function rise(page, key) {
  const before = (await measure(page, colours)).walker.y;
  await page.keyboard.down(key);
  let highest = before;
  for (let i = 0; i < 6; i++) {
    await page.waitForTimeout(80);
    const at = (await measure(page, colours)).walker.y;
    if (at !== null && at < highest) highest = at;
  }
  await page.keyboard.up(key);
  await page.waitForTimeout(1500);
  return before - highest;
}

const server = await serve(site);
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "rebind",
    check: async (page) => {
      const notDrawn = await settled(page);
      if (notDrawn) return notDrawn;
      if (await page.evaluate((key) => localStorage.getItem(key), KEY)) {
        return "controls were saved before any rebinding";
      }

      // F1, then J: jump is J from now on, and kept.
      await page.keyboard.press("F1");
      await page.waitForTimeout(300);
      await page.keyboard.press("KeyJ");
      await page.waitForTimeout(500);
      const saved = await page.evaluate((key) => localStorage.getItem(key), KEY);
      console.log(`localStorage[${KEY}] = ${saved}`);
      if (!saved || !saved.includes("KeyJ")) return "the rebound jump was not saved to localStorage";

      await page.reload();
      await page.waitForSelector("canvas", { timeout: 60_000 });
      const notRedrawn = await settled(page);
      if (notRedrawn) return `after the reload, ${notRedrawn}`;

      const bySpace = await rise(page, "Space");
      const byJ = await rise(page, "KeyJ");
      console.log(`after the reload the walker rose ${bySpace}px on Space and ${byJ}px on J`);
      if (byJ < 20) return "J did not jump after the reload";
      if (bySpace > 5) return "Space still jumped after the reload";
      return null;
    },
  });
} finally {
  server.close();
}
