//! `cargo xtask dist`: a folder someone without Rust can run.
//!
//! Godot's export, reduced to what this engine has: the player built in
//! release, renamed after the game, beside its loose assets and the manifest
//! that tells it what to start. Loose files rather than an archive — the
//! asset root already resolves beside the executable — and every asset
//! copied, since pruning the unused ones needs a dependency graph that does
//! not exist yet.

use std::path::{Path, PathBuf};
use std::process::Command;

use voltra_player::{Manifest, MANIFEST_FILE};

use crate::files::{copy_tree, prepare_build_dir};

/// Where the build lands and what is in it, worked out before anything is
/// written — so a bad manifest fails before a long release build.
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub manifest: Manifest,
    /// The source asset root.
    pub assets: PathBuf,
    /// `<out>/<name>`.
    pub folder: PathBuf,
    /// `<folder>/<name><exe suffix>`.
    pub executable: PathBuf,
}

/// Reads `manifest_path` and lays the build out under `out`.
pub fn plan(manifest_path: &Path, out: &Path) -> Result<Plan, String> {
    let manifest = Manifest::load(manifest_path).map_err(|e| e.to_string())?;
    let dir = manifest_path.parent().unwrap_or(Path::new(""));
    let assets = manifest.asset_root(dir);
    if !assets.is_dir() {
        return Err(format!("no asset folder at {}", assets.display()));
    }
    let scene = manifest.scene_path(dir);
    if !scene.is_file() {
        return Err(format!("the start scene {} is not there", scene.display()));
    }
    let name = manifest.file_name();
    let folder = out.join(&name);
    let executable = folder.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    Ok(Plan {
        manifest,
        assets,
        folder,
        executable,
    })
}

/// Builds the player and writes the build `plan` describes.
pub fn run(plan: &Plan, workspace: &Path) -> Result<(), String> {
    println!("building the player (release, windowed)…");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(workspace)
        .args([
            "build",
            "--release",
            "--package",
            "voltra-player",
            "--features",
            "windowed",
        ])
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("the release build failed ({status})"));
    }

    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let built = target
        .join("release")
        .join(format!("voltra-player{}", std::env::consts::EXE_SUFFIX));

    prepare_build_dir(&plan.folder, MANIFEST_FILE).map_err(|e| e.to_string())?;
    std::fs::copy(&built, &plan.executable)
        .map_err(|e| format!("could not copy {}: {e}", built.display()))?;
    let copied = copy_tree(&plan.assets, &plan.folder.join("assets"))
        .map_err(|e| format!("could not copy the assets: {e}"))?;

    // The shipped manifest: the same build, with its assets where the copy
    // put them.
    let shipped = Manifest {
        assets: PathBuf::from("assets"),
        ..plan.manifest.clone()
    };
    let text = shipped.to_ron().map_err(|e| e.to_string())?;
    std::fs::write(plan.folder.join(MANIFEST_FILE), text)
        .map_err(|e| format!("could not write the manifest: {e}"))?;

    println!(
        "{} → {} ({copied} asset files)",
        plan.manifest.name,
        plan.folder.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after 1970")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "voltra-dist-{nanos}-{:?}",
            std::thread::current().id()
        ));
        std::fs::create_dir_all(dir.join("assets/scenes")).expect("scratch dir");
        std::fs::write(dir.join("assets/scenes/start.ron"), "(version: 1)").expect("scene");
        dir
    }

    fn write_manifest(dir: &Path, name: &str, scene: &str) -> PathBuf {
        let path = dir.join(MANIFEST_FILE);
        std::fs::write(
            &path,
            format!(r#"(version: 1, name: "{name}", scene: "{scene}")"#),
        )
        .expect("manifest");
        path
    }

    #[test]
    fn the_build_is_named_after_the_game() {
        let dir = scratch();
        let manifest = write_manifest(&dir, "Sky: Rift", "scenes/start.ron");
        let plan = plan(&manifest, &dir.join("dist")).expect("plans");
        assert_eq!(plan.folder, dir.join("dist").join("Sky Rift"));
        assert_eq!(
            plan.executable,
            plan.folder
                .join(format!("Sky Rift{}", std::env::consts::EXE_SUFFIX))
        );
        assert_eq!(plan.assets, dir.join("assets"));
    }

    #[test]
    fn a_missing_start_scene_fails_before_anything_is_built() {
        let dir = scratch();
        let manifest = write_manifest(&dir, "Game", "scenes/nowhere.ron");
        let error = plan(&manifest, &dir.join("dist")).expect_err("refused");
        assert!(error.contains("nowhere.ron"), "{error}");
    }

    #[test]
    fn the_shipped_workspace_manifest_plans() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let plan = plan(&workspace.join(MANIFEST_FILE), &workspace.join("dist"))
            .expect("the repository's build.ron describes a real build");
        assert!(!plan.manifest.name.is_empty());
    }
}
