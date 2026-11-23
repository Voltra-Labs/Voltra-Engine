//! `cargo xtask pack`: a game as one file, for the web player.
//!
//! The same build `dist` lays out as a folder — the shipped manifest and the
//! asset tree — written as a [`voltra_assets::pack`] instead, which a page
//! fetches whole before the game starts.

use std::path::Path;

use voltra_player::MANIFEST_FILE;

use crate::dist::{self, Plan, SHIPPED_ASSETS};
use crate::files::tree_files;

/// The pack's files for `plan`: `build.ron`, then `assets/…`, `/`-separated.
pub fn files(plan: &Plan) -> Result<Vec<(String, Vec<u8>)>, String> {
    if plan.manifest.game.is_some() {
        return Err(format!(
            "{} names a game binary; the web player runs scenes, and a game with \
             its own code has no web entry yet",
            plan.manifest.name
        ));
    }
    let manifest = dist::shipped(&plan.manifest)
        .to_ron()
        .map_err(|e| e.to_string())?;
    let mut files = vec![(MANIFEST_FILE.to_owned(), manifest.into_bytes())];
    let tree = tree_files(&plan.assets)
        .map_err(|e| format!("could not list {}: {e}", plan.assets.display()))?;
    for relative in tree {
        let parts: Vec<_> = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        let key = format!("{SHIPPED_ASSETS}/{}", parts.join("/"));
        let path = plan.assets.join(&relative);
        let bytes =
            std::fs::read(&path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
        files.push((key, bytes));
    }
    Ok(files)
}

/// Writes the pack for `plan` to `out`.
pub fn run(plan: &Plan, out: &Path) -> Result<(), String> {
    let files = files(plan)?;
    let bytes = voltra_assets::pack::encode(
        files
            .iter()
            .map(|(path, bytes)| (path.as_str(), bytes.as_slice())),
    )
    .map_err(|e| format!("could not pack {}: {e}", plan.manifest.name))?;
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    std::fs::write(out, &bytes).map_err(|e| format!("could not write {}: {e}", out.display()))?;
    println!(
        "{} → {} ({} files, {} KiB)",
        plan.manifest.name,
        out.display(),
        files.len(),
        bytes.len().div_ceil(1024)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn the_sandbox_packs_with_its_manifest_first_and_reads_back() {
        let plan = dist::plan(&workspace().join(MANIFEST_FILE), Path::new("out"))
            .expect("the workspace manifest plans");
        let files = files(&plan).expect("packs");
        assert_eq!(files[0].0, MANIFEST_FILE);
        assert!(
            files
                .iter()
                .any(|(path, _)| path == "assets/scenes/sandbox.ron"),
            "the start scene is in it, under the shipped asset folder"
        );

        let bytes =
            voltra_assets::pack::encode(files.iter().map(|(p, b)| (p.as_str(), b.as_slice())))
                .expect("encodes");
        let back = voltra_assets::pack::decode(&bytes).expect("decodes");
        assert_eq!(back, files);

        let shipped = voltra_player::Manifest::from_ron(
            std::str::from_utf8(&back[0].1).expect("text"),
            Path::new(MANIFEST_FILE),
        )
        .expect("the packed manifest parses");
        assert_eq!(shipped.assets, PathBuf::from(SHIPPED_ASSETS));
    }

    #[test]
    fn a_game_with_its_own_binary_is_not_packed() {
        let plan =
            dist::plan(&workspace().join("platformer.build.ron"), Path::new("out")).expect("plans");
        assert!(files(&plan).is_err());
    }
}
