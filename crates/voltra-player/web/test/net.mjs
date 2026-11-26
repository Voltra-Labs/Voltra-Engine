// The web build reaching a native server: `voltra-net`'s browser WebSocket,
// in a tiny page (crates/voltra-net/examples/web_probe.rs), connects to the
// native WebSocket listener (examples/echo_server.rs) and gets its datagram
// back. No GPU is involved, so it runs once, not on every backend.
//
//   node crates/voltra-player/web/test/net.mjs [--skip-build] [--out DIR]
//
// Builds into DIR (default: target/net-test). Needs what harness.mjs needs.

import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { options, playwright, serve, workspace } from "./harness.mjs";

const { skipBuild, out } = options("net-test");
const target = process.env.CARGO_TARGET_DIR ?? join(workspace, "target");
const site = join(out, "site");

function run(command, args) {
  console.log(`$ ${command} ${args.join(" ")}`);
  execFileSync(command, args, { cwd: workspace, stdio: "inherit" });
}

if (!skipBuild) {
  run("cargo", ["build", "-p", "voltra-net", "--example", "echo_server"]);
  run("cargo", ["build", "-p", "voltra-net", "--example", "web_probe", "--target", "wasm32-unknown-unknown"]);
  mkdirSync(site, { recursive: true });
  run("wasm-bindgen", [
    "--target", "web", "--no-typescript", "--out-dir", site, "--out-name", "probe",
    join(target, "wasm32-unknown-unknown", "debug", "examples", "web_probe.wasm"),
  ]);
  writeFileSync(join(site, "index.html"), `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>voltra-net probe</title><link rel="icon" href="data:,"></head>
<body><script type="module">
  import init from "./probe.js";
  init().catch((error) => console.error(String(error)));
</script></body></html>
`);
}

const exe = process.platform === "win32" ? "echo_server.exe" : "echo_server";
const portFile = join(out, "echo-port");
rmSync(portFile, { force: true });
const server = spawn(join(target, "debug", "examples", exe), ["--port-file", portFile, "--seconds", "90"], {
  stdio: "inherit",
});

let failure = null;
const page_console = [];
const web = await serve(site);
try {
  const deadline = Date.now() + 20_000;
  while (!existsSync(portFile) && Date.now() < deadline) {
    await new Promise((wait) => setTimeout(wait, 50));
  }
  if (!existsSync(portFile)) throw new Error("the echo server never wrote its port");
  const port = readFileSync(portFile, "utf8").trim();

  const browser = await playwright().chromium.launch();
  try {
    const page = await browser.newPage();
    page.on("console", (message) => page_console.push(`[${message.type()}] ${message.text()}`));
    page.on("pageerror", (error) => page_console.push(`[pageerror] ${error.message}`));
    await page.goto(`${web.url}?server=ws://127.0.0.1:${port}/`);
    await page.waitForFunction(() => document.title === "voltra-net: echoed", null, { timeout: 30_000 });
  } catch (error) {
    failure = String(error);
  } finally {
    await browser.close();
  }
} catch (error) {
  failure = String(error);
} finally {
  web.close();
  server.kill();
}

console.log(`page console:\n  ${page_console.join("\n  ")}`);
const errors = page_console.filter((l) => l.startsWith("[error]") || l.startsWith("[pageerror]"));
if (!failure && errors.length > 0) failure = `the page logged ${errors.length} error(s)`;
if (failure) {
  console.error(`FAIL\n  ${failure}`);
  process.exit(1);
}
console.log("PASS");
