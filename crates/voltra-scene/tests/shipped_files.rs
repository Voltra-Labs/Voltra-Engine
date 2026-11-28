//! Every scene, prefab and clip the repository ships loads with nothing
//! lost: each component the file holds arrives under a name something
//! registers, and a save writes all of them back — the check that a
//! format migration renamed everything it had to, keys and the names
//! held as values alike.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use voltra_ecs::World;
use voltra_scene::format::{self, from_scene_file, to_scene_file};
use voltra_scene::{AnimationClip, ComponentRegistry, PrefabPart};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root exists")
}

/// The asset folders: the engine's samples and each game's.
fn asset_roots() -> Vec<PathBuf> {
    let root = workspace();
    let mut roots = vec![root.join("assets")];
    for game in std::fs::read_dir(root.join("games"))
        .expect("games/")
        .flatten()
    {
        let assets = game.path().join("assets");
        if assets.is_dir() {
            roots.push(assets);
        }
    }
    roots
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a folder").flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, out);
        } else if path.extension().is_some_and(|e| e == "ron") {
            out.push(path);
        }
    }
}

/// The names the sample games register their own components under, read
/// from their `NAME` consts: this crate cannot link the games.
fn game_names() -> BTreeSet<String> {
    let mut sources = Vec::new();
    fn rust(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    for game in std::fs::read_dir(workspace().join("games"))
        .expect("games/")
        .flatten()
    {
        rust(&game.path().join("src"), &mut sources);
    }
    let mut names = BTreeSet::new();
    for source in sources {
        let text = std::fs::read_to_string(source).expect("reads");
        for line in text.lines() {
            if let Some(rest) = line
                .trim()
                .strip_prefix("pub const NAME: &'static str = \"")
            {
                if let Some(name) = rest.strip_suffix("\";") {
                    names.insert(name.to_owned());
                }
            }
        }
    }
    names
}

/// Whether `text` is shaped as a scene file — a version and entities —
/// rather than another RON asset a scene's name allows (a theme, a
/// material); whether it reads is the test's question.
fn is_scene(text: &str) -> bool {
    #[derive(serde::Deserialize)]
    #[allow(dead_code)] // Read to see that it is there.
    struct Probe {
        version: u32,
        entities: serde::de::IgnoredAny,
    }
    ron::from_str::<Probe>(text).is_ok()
}

#[test]
fn every_shipped_scene_and_prefab_loads_with_nothing_lost() {
    let registry = ComponentRegistry::with_defaults();
    let mut known: BTreeSet<String> = registry.names().map(str::to_owned).collect();
    let games = game_names();
    assert!(games.len() > 10, "found the games' names: {games:?}");
    known.extend(games);

    let mut checked = 0;
    let mut paths = Vec::new();
    for root in asset_roots() {
        files(&root, &mut paths);
    }
    for path in &paths {
        let shown = path
            .strip_prefix(workspace())
            .unwrap_or(path)
            .display()
            .to_string();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = std::fs::read_to_string(path).expect("reads");
        if !voltra_assets::watch::is_scene(&name) || !is_scene(&text) {
            continue;
        }
        // Current on disk, not only once read: `cargo xtask migrate` has
        // been run over everything shipped (CI's `--check` says the same).
        let (file, written) =
            format::parse_versioned(&text).unwrap_or_else(|e| panic!("{shown}: {e}"));
        assert_eq!(
            written,
            format::VERSION,
            "{shown}: run `cargo xtask migrate`"
        );
        checked += 1;

        for record in &file.entities {
            for component in record.components.keys() {
                assert!(
                    known.contains(component),
                    "{shown}: `{component}` is a name nothing registers"
                );
            }
            if let Some(raw) = record.components.get(PrefabPart::NAME) {
                let part: PrefabPart = raw.into_rust().expect("a part");
                for component in &part.overridden {
                    assert!(
                        known.contains(component),
                        "{shown}: an override of `{component}`, which nothing registers"
                    );
                }
            }
        }

        // Loaded and saved, every component comes back: a registered one
        // as its type writes it, any other exactly as the file had it.
        let mut world = World::new();
        from_scene_file(&file, &registry, &mut world).unwrap_or_else(|e| panic!("{shown}: {e}"));
        let saved = to_scene_file(&world, &registry).expect("saves");
        assert_eq!(saved.entities.len(), file.entities.len(), "{shown}");
        for record in &file.entities {
            let back = saved
                .entities
                .iter()
                .find(|r| r.id == record.id)
                .unwrap_or_else(|| panic!("{shown}: entity {} lost", record.id.0));
            let names: Vec<&String> = record.components.keys().collect();
            let back_names: Vec<&String> = back.components.keys().collect();
            assert_eq!(names, back_names, "{shown}: entity {}", record.id.0);
            for (name, value) in &record.components {
                let expected = match registry.normalize(name, value) {
                    Some(normal) => normal.expect("a registered component reads"),
                    None => value.clone(),
                };
                assert_eq!(back.components[name], expected, "{shown}: {name}");
            }
        }
    }
    assert!(
        checked >= 15,
        "found the shipped scenes and prefabs ({checked})"
    );
}

#[test]
fn every_shipped_clip_reads_every_track() {
    let mut checked = 0;
    let mut paths = Vec::new();
    for root in asset_roots() {
        files(&root, &mut paths);
    }
    for path in &paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !voltra_assets::watch::is_animation(&name) {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("reads");
        let clip: AnimationClip =
            ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let keys: usize = clip.tracks.iter().map(|t| t.keys.len()).sum();
        // Every track named a property this build knows, and the check a
        // load runs drops no key of it.
        assert_eq!(
            clip.tracks.len(),
            text.matches("property:").count(),
            "{}",
            path.display()
        );
        let kept: usize = clip
            .clone()
            .checked("shipped")
            .tracks
            .iter()
            .map(|t| t.keys.len())
            .sum();
        assert_eq!(kept, keys, "{}", path.display());
        // Under the current names, as the clip store writes it — compared
        // as LF: a checkout may have made it CRLF, which is the same file.
        let written = voltra_scene::ron_store::to_text(&clip).expect("writes");
        let lf = text.replace("\r\n", "\n");
        assert!(
            lf.ends_with(&written),
            "{}: run `cargo xtask migrate`",
            path.display()
        );
        checked += 1;
    }
    assert!(checked >= 9, "found the shipped clips ({checked})");
}
