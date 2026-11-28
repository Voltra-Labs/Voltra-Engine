//! `cargo xtask migrate`: every scene, prefab and clip under a folder read
//! through the engine's own loader and written through its own writer, so
//! a file comes out exactly as an editor save of it would — in the current
//! format version, with every renamed component under its new name.
//!
//! `--check` writes nothing: it lists what a migration would change and
//! fails if anything would, which CI runs over the shipped assets. "Out of
//! date" is any difference from that save, so a version 2 file written by
//! hand — fields left to their defaults, keys out of order — is listed too,
//! not only an older version.
//!
//! A rewrite keeps no comment but the leading block: one inside the
//! entities is dropped, as an editor save drops it.
//!
//! Two differences from an editor save, both kept on purpose: a file's
//! leading comment block is kept (a save would drop it; the shipped samples
//! explain themselves there), and a component the engine does not register
//! — a game's own — is written back exactly as the file had it, since this
//! task does not link the game that registers it.

use std::path::{Path, PathBuf};

use serde::de::IgnoredAny;
use serde::Deserialize;
use voltra_ecs::World;
use voltra_player::{Manifest, MANIFEST_FILE};
use voltra_scene::format::{from_scene_file, parse, to_scene_file, to_text};
use voltra_scene::{AnimationClip, ComponentRegistry};

/// What a file is, as far as migrating goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A scene or a prefab: a scene file.
    Scene,
    /// An animation clip (`.anim.ron`).
    Clip,
}

/// One file a migration would change.
#[derive(Debug)]
pub struct Outdated {
    /// Where it is.
    pub path: PathBuf,
    /// What it is.
    pub kind: Kind,
    /// What a migration writes in its place.
    pub text: String,
}

/// Runs the task: `migrate [FOLDER]... [--check]`.
pub fn run(args: impl Iterator<Item = String>, workspace: &Path) -> Result<(), String> {
    let mut folders = Vec::new();
    let mut check = false;
    for arg in args {
        match arg.as_str() {
            "--check" => check = true,
            other if other.starts_with("--") => {
                return Err(format!(
                    "unknown option `{other}` (migrate [FOLDER]... [--check])"
                ))
            }
            other => folders.push(PathBuf::from(other)),
        }
    }
    if folders.is_empty() {
        folders = asset_roots(workspace)?;
    }
    let registry = ComponentRegistry::with_defaults();
    let mut outdated = Vec::new();
    let mut seen = 0;
    for folder in &folders {
        if !folder.is_dir() {
            return Err(format!("{} is not a folder", folder.display()));
        }
        let (found, files) = survey(folder, &registry)?;
        seen += found;
        outdated.extend(files);
    }
    let shown = |path: &Path| {
        path.strip_prefix(workspace)
            .unwrap_or(path)
            .display()
            .to_string()
    };
    if check {
        for file in &outdated {
            println!("out of date: {} ({:?})", shown(&file.path), file.kind);
        }
        if outdated.is_empty() {
            println!("{seen} scene, prefab and clip files are up to date");
            return Ok(());
        }
        return Err(format!(
            "{} of {seen} files are out of date — an older version, or not as an editor \
             save writes them; `cargo xtask migrate` rewrites them (keeping each file's \
             leading comments, dropping any other)",
            outdated.len()
        ));
    }
    for file in &outdated {
        voltra_scene::format::atomic::replace(&file.path, file.text.as_bytes())
            .map_err(|e| format!("{}: {e}", file.path.display()))?;
        println!("migrated {}", shown(&file.path));
    }
    println!(
        "{} of {seen} scene, prefab and clip files rewritten",
        outdated.len()
    );
    Ok(())
}

/// The asset root of every build manifest in the workspace: `build.ron`
/// and `*.build.ron` at the root, and each game's `build.ron`.
fn asset_roots(workspace: &Path) -> Result<Vec<PathBuf>, String> {
    let mut manifests = Vec::new();
    let mut manifests_in = |dir: &Path| {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == MANIFEST_FILE || name.ends_with(&format!(".{MANIFEST_FILE}")) {
                manifests.push(entry.path());
            }
        }
    };
    manifests_in(workspace);
    if let Ok(games) = std::fs::read_dir(workspace.join("games")) {
        for game in games.flatten() {
            manifests_in(&game.path());
        }
    }
    manifests.sort();
    let mut roots: Vec<PathBuf> = Vec::new();
    for manifest in manifests {
        let read = Manifest::load(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
        let dir = manifest.parent().unwrap_or(workspace);
        let root = read.asset_root(dir);
        let root = root.canonicalize().unwrap_or(root);
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    if roots.is_empty() {
        return Err("no build.ron in the workspace names an asset root; name a folder".into());
    }
    Ok(roots)
}

/// How many scene, prefab and clip files are under `folder`, and those a
/// migration would change, in path order.
pub fn survey(
    folder: &Path,
    registry: &ComponentRegistry,
) -> Result<(usize, Vec<Outdated>), String> {
    let mut files = Vec::new();
    ron_files(folder, &mut files);
    files.sort();
    let mut seen = 0;
    let mut outdated = Vec::new();
    for path in files {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let migrated = if voltra_assets::watch::is_animation(&name) {
            Some((Kind::Clip, clip_text(&text)))
        } else if voltra_assets::watch::is_scene(&name) && is_scene_file(&text) {
            Some((Kind::Scene, scene_text(&text, registry)))
        } else {
            None
        };
        let Some((kind, migrated)) = migrated else {
            continue;
        };
        seen += 1;
        let migrated = migrated.map_err(|e| format!("{}: {e}", path.display()))?;
        // Compared as LF, the engine's writer's ending; written back in the
        // file's own, all through — a checkout that turned the file to CRLF
        // is not out of date for it, and a mixed file comes out one way.
        let ending = Ending::of(&text);
        let lf = text.replace("\r\n", "\n");
        let migrated = format!("{}{migrated}", header(&lf));
        if migrated != lf || matches!(ending, Ending::Mixed(_)) {
            outdated.push(Outdated {
                path,
                kind,
                text: ending.apply(&migrated),
            });
        }
    }
    Ok((seen, outdated))
}

/// A file's line endings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// `\n` only, as the engine writes.
    Lf,
    /// `\r\n` only: a checkout that converted it (Git for Windows'
    /// `core.autocrlf`).
    Crlf,
    /// Both: written back in the one most lines had — CRLF when `true`.
    Mixed(bool),
}

impl Ending {
    fn of(text: &str) -> Self {
        let crlf = text.matches("\r\n").count();
        let lf = text.matches('\n').count() - crlf;
        match (crlf, lf) {
            (0, _) => Self::Lf,
            (_, 0) => Self::Crlf,
            _ => Self::Mixed(crlf > lf),
        }
    }

    /// LF `text` in this ending; a mixed file takes the one most of its
    /// lines had, so a rewrite is consistent.
    fn apply(self, text: &str) -> String {
        match self {
            Self::Lf | Self::Mixed(false) => text.to_owned(),
            Self::Crlf | Self::Mixed(true) => text.replace('\n', "\r\n"),
        }
    }
}

/// Every `.ron` under `dir`, leaving out build output and hidden folders.
fn ron_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !name.starts_with('.') && name != "target" && name != "dist" {
                ron_files(&path, out);
            }
        } else if name.to_ascii_lowercase().ends_with(".ron") {
            out.push(path);
        }
    }
}

/// Whether `text` is shaped as a scene file — a version and a list of
/// entities — rather than another RON asset a scene's name allows (a
/// theme, a material, a build manifest). Whether it then reads is another
/// question, answered with its error.
fn is_scene_file(text: &str) -> bool {
    #[derive(Deserialize)]
    #[allow(dead_code)] // Read to see that it is there.
    struct Probe {
        version: u32,
        entities: IgnoredAny,
    }
    ron::from_str::<Probe>(text).is_ok()
}

/// The comment block `text` starts with: [`voltra_assets::leading_comments`],
/// which the editor's material writer keeps the same way.
fn header(text: &str) -> &str {
    voltra_assets::leading_comments(text)
}

/// `text`, a scene file of any version, as an editor save of it writes it:
/// loaded into a world through the registry and saved back.
fn scene_text(text: &str, registry: &ComponentRegistry) -> Result<String, String> {
    let file = parse(text).map_err(|e| e.to_string())?;
    let mut world = World::new();
    from_scene_file(&file, registry, &mut world).map_err(|e| e.to_string())?;
    let saved = to_scene_file(&world, registry).map_err(|e| e.to_string())?;
    to_text(&saved).map_err(|e| e.to_string())
}

/// `text`, a clip, as the clip store writes it — the timeline's save. Read
/// as written: keys a load would drop as unusable are kept for the author.
fn clip_text(text: &str) -> Result<String, String> {
    let clip: AnimationClip = ron::from_str(text).map_err(|e| e.to_string())?;
    voltra_scene::ron_store::to_text(&clip).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
