//! The repository's build tasks, run as `cargo xtask <task>`.
//!
//! The `xtask` convention: tasks are a binary in the workspace rather than a
//! shell script, so they are written in the language the project already
//! builds and run identically on every platform it does. `.cargo/config.toml`
//! holds the alias.

mod dist;
mod files;
mod guide;
mod new_game;
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
        fetches (default: build.ron, dist/<name>-web/game.pak)
  new-game NAME [--title TITLE]
        Make games/NAME: a game crate the editor loads and plays, with its
        build.ron, a first scene and a test that loads it as the editor
        does
  guide
        Run the user guide's checked commands (docs/guide), in order, as
        someone following it from a clean checkout would";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("dist") => dist_command(args),
        Some("pack") => pack_command(args),
        Some("new-game") => new_game_command(args),
        Some("guide") => guide::run(&workspace_root()),
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

fn new_game_command(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut name = None;
    let mut title = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--title" => title = Some(args.next().ok_or("`--title` needs a value")?),
            other if other.starts_with("--") => {
                return Err(format!("unknown option `{other}`\n\n{USAGE}"))
            }
            other if name.is_none() => name = Some(other.to_owned()),
            other => return Err(format!("one name only, not `{other}` too")),
        }
    }
    let name = name.ok_or(format!("new-game needs a name\n\n{USAGE}"))?;
    let names = new_game::Names::new(&name, title.as_deref())?;
    let dir = new_game::create(&workspace_root(), &names)?;
    // Best effort: a template filled in with a long name may want rewrapping.
    let _ = std::process::Command::new("cargo")
        .args(["fmt", "--package", &names.package])
        .current_dir(&dir)
        .status();
    println!("{}", new_game::next_steps(&names));
    Ok(())
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
