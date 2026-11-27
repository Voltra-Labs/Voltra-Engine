// The arena's web build joining a native arena server over WebSocket: the
// page's `?join=ws://…` names the server (a page has no LAN search: it
// joins by address), `?bot` readies it up in the server's lobby and — the
// lobby's first player, its host — starts the match; the server logs the
// browser joining and spawns its player, and the page — once replication
// has sent that player — predicts it. On WebGPU and WebGL2, a peer each:
// the second drops into the match the first started.
//
//   node crates/voltra-player/web/test/arena.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/web-arena). See harness.mjs for what it
// needs installed.

import { execFileSync, spawn } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";

import { build, onEveryBackend, options, serve, workspace } from "./harness.mjs";

const { skipBuild, out } = options("web-arena");
const site = build({ out, manifest: "games/arena/build.ron", skipBuild });
const target = process.env.CARGO_TARGET_DIR ?? join(workspace, "target");
if (!skipBuild) {
  console.log("$ cargo build -p arena --bin arena-server");
  execFileSync("cargo", ["build", "-p", "arena", "--bin", "arena-server"], { cwd: workspace, stdio: "inherit" });
}

const portFile = join(out, "arena-ports");
rmSync(portFile, { force: true });
const exe = process.platform === "win32" ? "arena-server.exe" : "arena-server";
const serverLog = [];
const server = spawn(
  join(target, "debug", exe),
  ["--asset-root", join(workspace, "games/arena/assets"), "--udp", "off", "--ws", "0",
    "--discovery", "off", "--min-players", "1", "--seconds", "240", "--port-file", portFile],
  { env: { ...process.env, RUST_LOG: "info" }, stdio: ["ignore", "ignore", "pipe"] },
);
// `onEveryBackend` exits the process when it is done: the server goes too.
process.on("exit", () => server.kill());
server.stderr.on("data", (chunk) => serverLog.push(...chunk.toString().split("\n").filter(Boolean)));

const deadline = Date.now() + 30_000;
while (!existsSync(portFile) && Date.now() < deadline) {
  await new Promise((wait) => setTimeout(wait, 50));
}
if (!existsSync(portFile)) {
  server.kill();
  console.error(`FAIL\n  the arena server never wrote its port:\n  ${serverLog.join("\n  ")}`);
  process.exit(1);
}
const port = readFileSync(portFile, "utf8").match(/^ws (\d+)$/m)?.[1];

// The page's log, kept where a check can read it.
const keepLog = () => {
  window.__lines = [];
  for (const level of ["log", "info", "warn", "error", "debug"]) {
    const original = console[level].bind(console);
    console[level] = (...args) => {
      window.__lines.push(args.map(String).join(" "));
      original(...args);
    };
  }
};

async function until(test, timeout = 60_000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (await test()) return true;
    await new Promise((wait) => setTimeout(wait, 200));
  }
  return false;
}

const web = await serve(site);
let joined = 0;
try {
  await onEveryBackend({
    site,
    url: `${web.url}?join=ws://127.0.0.1:${port}/&bot`,
    name: "arena",
    initScript: keepLog,
    check: async (page) => {
      const before = joined;
      const plays = () => serverLog.filter((line) => /arena: peer \d+ plays/.test(line)).length;
      if (!(await until(async () => plays() > before))) {
        return `the server never spawned a player for the page:\n  ${serverLog.join("\n  ")}`;
      }
      joined = plays();
      const predicted = await until(() =>
        page.evaluate(() => window.__lines.some((line) => line.includes("arena: playing as"))),
      );
      if (!predicted) return "the page never received its player to predict";
      return null;
    },
  });
} finally {
  web.close();
  server.kill();
}
