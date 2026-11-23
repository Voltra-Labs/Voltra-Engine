//! The repository's build tasks, run as `cargo xtask <task>`.
//!
//! The `xtask` convention: tasks are a binary in the workspace rather than a
//! shell script, so they are written in the language the project already
//! builds and run identically on every platform it does. `.cargo/config.toml`
//! holds the alias.

mod dist;
mod files;
mod pack;
mod web;

use std::path::PathBuf;
use std::process::ExitCode;

use voltra_player::{MANIFEST_FILE, PACK_FILE};

const USAGE: &str = "\
Usage: cargo xtask <TASK>

Tasks:
  dist [--web] [--manifest FILE] [--out DIR]
        Build the player in release and package it with its assets and
        manifest into DIR/<name> (default: build.ron, dist/). With --web,
        build it for the browser into DIR/<name>-web: the page, the wasm
        and the pack, for any static server
  pack [--manifest FILE] [--out FILE]
        Write the manifest and its assets as the one file a web build
        fetches (default: build.ron, dist/<name>-web/game.pak)";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("dist") => dist_command(args),
        Some("pack") => pack_command(args),
        Some("-h" | "--help") | None => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Some(other) => Err(format!("unknown task `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn dist_command(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let workspace = workspace_root();
    let mut manifest = workspace.join(MANIFEST_FILE);
    let mut out = workspace.join("dist");
    let mut for_web = false;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("`{arg}` needs a value"));
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(value()?),
            "--out" => out = PathBuf::from(value()?),
            "--web" => for_web = true,
            other => return Err(format!("unknown option `{other}`\n\n{USAGE}")),
        }
    }
    let plan = dist::plan(&manifest, &out)?;
    if for_web {
        let web = web::plan(&plan, &out);
        web::run(&plan, &web, &workspace)
    } else {
        dist::run(&plan, &workspace)
    }
}

fn pack_command(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let workspace = workspace_root();
    let mut manifest = workspace.join(MANIFEST_FILE);
    let mut out = None;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("`{arg}` needs a value"));
        match arg.as_str() {
            "--manifest" => manifest = PathBuf::from(value()?),
            "--out" => out = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown option `{other}`\n\n{USAGE}")),
        }
    }
    let dist = workspace.join("dist");
    let plan = dist::plan(&manifest, &dist)?;
    let out = out.unwrap_or_else(|| {
        dist.join(format!("{}-web", plan.manifest.file_name()))
            .join(PACK_FILE)
    });
    pack::run(&plan, &out)
}

/// The workspace root: two levels above this crate's manifest. Not the
/// working directory, which is wherever `cargo xtask` was typed.
fn workspace_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(|crates| crates.parent())
        .map(PathBuf::from)
        .unwrap_or(crate_dir)
}
