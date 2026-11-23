//! `cargo xtask dist --web`: a folder any static server can host.
//!
//! The web half of `dist`: the same manifest, built for
//! `wasm32-unknown-unknown`, run through `wasm-bindgen`, beside the page that
//! loads it and the pack it reads (`pack.rs`). Godot's web export and Unity's
//! WebGL build ship the same three things — the page, the wasm with its
//! loader, the game's data — and nothing that needs a server of its own.
//!
//! ```text
//! dist/<name>-web/
//!   index.html      the canvas, a status line, the loader
//!   game.js         wasm-bindgen's loader
//!   game_bg.wasm    the player, or the game's own binary
//!   game.pak        build.ron and the assets
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use voltra_player::PACK_FILE;

use crate::dist::Plan;
use crate::files::prepare_build_dir;

/// The page every web build ships; its `<title>` becomes the game's name.
const PAGE: &str = include_str!("../../voltra-player/web/index.html");

/// The name wasm-bindgen gives its output (`--out-name`), and so what the
/// page imports, whichever binary it is.
pub const OUT_NAME: &str = "game";

/// The wasm target.
const TARGET: &str = "wasm32-unknown-unknown";

/// Where a web build lands and what it is built from.
#[derive(Debug, PartialEq)]
pub struct WebPlan {
    /// `<out>/<name>-web`.
    pub folder: PathBuf,
    /// What `cargo build --release --target wasm32-unknown-unknown` is given.
    pub cargo_args: Vec<String>,
    /// The built `.wasm`, under `target/wasm32-unknown-unknown/release`.
    pub wasm: PathBuf,
}

/// Lays the web build of `plan` out under `out`.
///
/// The player without `windowed` — a console window means nothing in a
/// browser — or the game's own target, as natively.
pub fn plan(plan: &Plan, out: &Path) -> WebPlan {
    let (cargo_args, wasm) = match &plan.manifest.game {
        Some(game) => {
            let file = format!("{}.wasm", game.target_name());
            let wasm = if game.is_example() {
                PathBuf::from("examples").join(file)
            } else {
                PathBuf::from(file)
            };
            (game.cargo_args(), wasm)
        }
        None => (
            ["--package", "voltra-player"].map(String::from).to_vec(),
            PathBuf::from("voltra-player.wasm"),
        ),
    };
    WebPlan {
        folder: out.join(format!("{}-web", plan.manifest.file_name())),
        cargo_args,
        wasm,
    }
}

/// Builds the wasm and writes the web build `web` describes.
pub fn run(plan: &Plan, web: &WebPlan, workspace: &Path) -> Result<(), String> {
    // Before the long build: a missing or mismatched CLI is the one failure
    // that is cheap to report first.
    check_wasm_bindgen(workspace)?;

    println!("building {} for the web (release)…", web.wasm.display());
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(workspace)
        .args(["build", "--release", "--target", TARGET])
        .args(&web.cargo_args)
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if !status.success() {
        return Err(format!(
            "the wasm build failed ({status}); is the target installed? \
             `rustup target add {TARGET}`"
        ));
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let built = target.join(TARGET).join("release").join(&web.wasm);

    // A web build's mark is its pack, as a native one's is its manifest.
    prepare_build_dir(&web.folder, PACK_FILE).map_err(|e| e.to_string())?;
    let status = Command::new("wasm-bindgen")
        .args(["--target", "web", "--no-typescript", "--out-name", OUT_NAME])
        .arg("--out-dir")
        .arg(&web.folder)
        .arg(&built)
        .status()
        .map_err(|e| format!("could not run wasm-bindgen: {e}"))?;
    if !status.success() {
        return Err(format!("wasm-bindgen failed ({status})"));
    }
    std::fs::write(web.folder.join("index.html"), page(&plan.manifest.name))
        .map_err(|e| format!("could not write the page: {e}"))?;
    crate::pack::run(plan, &web.folder.join(PACK_FILE))?;

    println!(
        "{} → {} (serve the folder; see docs/WEB.md)",
        plan.manifest.name,
        web.folder.display()
    );
    Ok(())
}

/// Refuses a `wasm-bindgen` CLI that is missing or is not the version of
/// the crate in `Cargo.lock`: the two must match exactly, or the CLI
/// refuses the wasm with a schema error far less clear than this.
fn check_wasm_bindgen(workspace: &Path) -> Result<(), String> {
    let lock = std::fs::read_to_string(workspace.join("Cargo.lock"))
        .map_err(|e| format!("could not read Cargo.lock: {e}"))?;
    let wanted = locked_version(&lock, "wasm-bindgen")
        .ok_or("Cargo.lock has no wasm-bindgen; nothing builds for the web")?;
    let install = format!("cargo install wasm-bindgen-cli --version {wanted} --locked");
    let output = Command::new("wasm-bindgen")
        .arg("--version")
        .output()
        .map_err(|_| format!("wasm-bindgen is not installed; install it with `{install}`"))?;
    let reported = String::from_utf8_lossy(&output.stdout);
    match cli_version(&reported) {
        Some(found) if found == wanted => Ok(()),
        Some(found) => Err(format!(
            "wasm-bindgen {found} is installed, and this workspace needs {wanted}; \
             install it with `{install}`"
        )),
        None => Err(format!(
            "could not read wasm-bindgen's version from `{}`; install it with `{install}`",
            reported.trim()
        )),
    }
}

/// The version of `package` in a `Cargo.lock`'s text.
fn locked_version(lock: &str, package: &str) -> Option<String> {
    let name = format!("name = \"{package}\"");
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line.trim() == name {
            let version = lines.next()?.trim();
            return version
                .strip_prefix("version = \"")?
                .strip_suffix('"')
                .map(str::to_owned);
        }
    }
    None
}

/// The version in `wasm-bindgen --version`'s answer (`wasm-bindgen 0.2.126`).
fn cli_version(output: &str) -> Option<String> {
    let version = output.split_whitespace().nth(1)?;
    version
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
        .then(|| version.to_owned())
}

/// The page, titled `name`.
fn page(name: &str) -> String {
    let escaped = name
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    PAGE.replacen(
        "<title>Voltra</title>",
        &format!("<title>{escaped}</title>"),
        1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dist;

    fn workspace() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn a_scene_build_ships_the_player_without_its_console_flag() {
        let native = dist::plan(&workspace().join("build.ron"), Path::new("out")).expect("plans");
        let web = plan(&native, Path::new("out"));
        assert_eq!(web.folder, Path::new("out").join("Voltra Sandbox-web"));
        assert_eq!(web.cargo_args, ["--package", "voltra-player"]);
        assert_eq!(web.wasm, PathBuf::from("voltra-player.wasm"));
    }

    #[test]
    fn a_game_build_ships_its_own_wasm() {
        let native =
            dist::plan(&workspace().join("platformer.build.ron"), Path::new("out")).expect("plans");
        let web = plan(&native, Path::new("out"));
        assert_eq!(
            web.cargo_args,
            ["--package", "voltra-core", "--example", "platformer"]
        );
        assert_eq!(web.wasm, Path::new("examples").join("platformer.wasm"));
    }

    #[test]
    fn the_lockfile_names_the_cli_to_install() {
        let lock = "[[package]]\nname = \"wasm-bindgen-futures\"\nversion = \"0.4.1\"\n\n\
                    [[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.126\"\n";
        assert_eq!(
            locked_version(lock, "wasm-bindgen").as_deref(),
            Some("0.2.126")
        );
        assert_eq!(locked_version(lock, "js-sys"), None);
        assert!(
            locked_version(
                &std::fs::read_to_string(workspace().join("Cargo.lock")).expect("lock"),
                "wasm-bindgen"
            )
            .is_some(),
            "the workspace builds for the web, so its lockfile has wasm-bindgen"
        );
    }

    #[test]
    fn the_cli_version_is_read_from_its_answer() {
        assert_eq!(
            cli_version("wasm-bindgen 0.2.126\n").as_deref(),
            Some("0.2.126")
        );
        assert_eq!(cli_version("wasm-bindgen"), None);
        assert_eq!(cli_version("error: unknown"), None);
    }

    #[test]
    fn the_page_is_titled_after_the_game_and_escaped() {
        let page = page("Rock & <Roll>");
        assert!(page.contains("<title>Rock &amp; &lt;Roll&gt;</title>"));
        assert!(
            page.contains(&format!("./{OUT_NAME}.js")),
            "it loads what wasm-bindgen writes"
        );
        assert!(
            page.contains("voltra-status"),
            "it has the status line games report to"
        );
    }
}
