// A game with its own code, played in the browser: the platformer example
// built for the web, its walker walked right with the keyboard into a coin,
// and the coin's sound heard — on WebGPU and on WebGL2.
//
//   node crates/voltra-player/web/test/play.mjs [--skip-build] [--out DIR]
//
// What it pins (the web's sound and input; ARCHITECTURE.md, "In the
// browser, sound waits for a gesture"):
// - the canvas has the keyboard without being clicked (`app/web.rs`);
// - an action read by a tick moves the game (`move_right` on ArrowRight);
// - sound waits for that first key (the autoplay policy is on), then starts
//   on it, and a clip reaches the speakers: the
//   samples cpal hands WebAudio are watched, and the coin's are not silent.
//
// Builds into DIR (default: target/web-play). See harness.mjs for what it
// needs installed.

import { build, encode, onEveryBackend, options, serve, waitFor, measure } from "./harness.mjs";

const { skipBuild, out } = options("web-play");
const site = build({ out, manifest: "platformer.build.ron", skipBuild });

// Every buffer cpal fills is copied into an `AudioBuffer`; keep the loudest
// sample seen, and every context's state, where the test can ask for them.
//
// And hold the page to a browser's autoplay rule, which headless Chromium
// does not apply — flag or not, and it reports user activation from the
// start: a context made before the first trusted key, pointer or touch
// starts suspended, and `resume` before one waits, as Chrome's does.
function watchAudio() {
  let gestured = false;
  for (const kind of ["keydown", "pointerdown", "touchstart"]) {
    // Capturing, so it is seen before the game's own handler runs.
    window.addEventListener(kind, (event) => { if (event.isTrusted) gestured = true; }, true);
  }
  window.voltraAudio = { loudest: 0, contexts: [] };
  const copy = AudioBuffer.prototype.copyToChannel;
  AudioBuffer.prototype.copyToChannel = function (source, ...rest) {
    for (const sample of source) {
      const level = Math.abs(sample);
      if (level > window.voltraAudio.loudest) window.voltraAudio.loudest = level;
    }
    return copy.call(this, source, ...rest);
  };
  const Context = window.AudioContext;
  window.AudioContext = class extends Context {
    constructor(...args) {
      super(...args);
      window.voltraAudio.contexts.push(this);
      if (!gestured) super.suspend();
    }
    resume() {
      if (!gestured) return new Promise(() => {});
      return super.resume();
    }
  };
}

const audio = (page) =>
  page.evaluate(() => ({
    loudest: window.voltraAudio.loudest,
    states: window.voltraAudio.contexts.map((context) => context.state),
  }));

// The two crates left of the start, as the frame stores them. The camera
// follows the walker, so walking right moves them left on screen.
const colours = { crates: encode([0.85, 0.55, 0.3]) };

const server = await serve(site);
try {
  await onEveryBackend({
    site,
    url: server.url,
    name: "play",
    initScript: watchAudio,
    check: async (page) => {
      const first = await waitFor(page, colours, (f) => f.crates.count > 300);
      if (!first.ok) return `the level was not drawn: ${JSON.stringify(first.found)}`;
      // Settled: the walker has landed and the camera caught up.
      await page.waitForTimeout(1500);
      const before = await measure(page, colours);
      const quiet = await audio(page);
      console.log(`before the key: ${JSON.stringify(quiet)}`);
      if (quiet.loudest > 0.001) return `sound before any coin: ${JSON.stringify(quiet)}`;
      // The page is held to a browser's rule: no sound before a gesture.
      if (quiet.states.includes("running")) return "audio ran before any gesture";

      // No click first: the key must reach the canvas on its own.
      await page.keyboard.down("ArrowRight");
      await page.waitForTimeout(1200);
      await page.keyboard.up("ArrowRight");
      await page.waitForTimeout(800);

      const after = await measure(page, colours);
      const heard = await audio(page);
      console.log(`crates x: ${before.crates.x} → ${after.crates.x}; audio: ${JSON.stringify(heard)}`);
      if (after.crates.count > 0 && !(after.crates.x < before.crates.x - 40)) {
        return "the walker did not move right";
      }
      if (!heard.states.includes("running")) return "no running AudioContext after the key";
      if (heard.loudest < 0.01) return "the coin's clip was not heard";
      return null;
    },
  });
} finally {
  server.close();
}
