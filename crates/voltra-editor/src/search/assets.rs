//! A search over the asset root: every file and folder under it, whatever
//! folder the browser is in, matched by name and by kind.
//!
//! The whole tree rather than the open folder, as Unity's Project search and
//! Unreal's Content Browser search: "where did I put the coin sound" is the
//! question a search box is for, and a folder already showing what is in it
//! needs no filter.

use std::path::Path;

use voltra_assets::browse::{self, Entry, EntryKind};
use voltra_assets::AssetPath;

use super::Query;
use crate::prefab::is_prefab_name;

/// What a `t:` term asks of an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindTerm {
    /// One kind of entry.
    Kind(EntryKind),
    /// A scene named as a prefab ([`PREFAB_SUFFIX`](crate::prefab::PREFAB_SUFFIX)).
    Prefab,
}

impl KindTerm {
    /// The term a word names: the kind's own name, or the one an author
    /// would reach for first ("sound" for a clip, "folder" for a directory).
    fn named(word: &str) -> Option<Self> {
        let kind = match word {
            "texture" | "image" | "sprite" | "png" => EntryKind::Texture,
            "atlas" => EntryKind::Atlas,
            "sound" | "audio" | "clip" | "music" => EntryKind::Clip,
            "font" => EntryKind::Font,
            "input" => EntryKind::InputMap,
            "layers" | "sorting" => EntryKind::SortingLayers,
            "buses" | "bus" => EntryKind::BusLayout,
            "render" => EntryKind::RenderSettings,
            "animation" | "anim" => EntryKind::Animation,
            "animator" => EntryKind::Animator,
            "material" => EntryKind::Material,
            "scene" => EntryKind::Scene,
            "prefab" => return Some(Self::Prefab),
            "folder" | "directory" | "dir" => EntryKind::Directory,
            "other" => EntryKind::Other,
            _ => return None,
        };
        Some(Self::Kind(kind))
    }

    fn matches(self, entry: &Entry) -> bool {
        match self {
            Self::Kind(kind) => entry.kind == kind,
            Self::Prefab => entry.kind == EntryKind::Scene && is_prefab_name(&entry.name),
        }
    }
}

/// The kinds a query's `t:` terms name, and the terms that name none.
fn kind_terms(query: &Query) -> (Vec<KindTerm>, Vec<String>) {
    let mut terms = Vec::new();
    let mut unknown = Vec::new();
    for typed in query.types() {
        match KindTerm::named(typed) {
            Some(term) => terms.push(term),
            None => unknown.push(typed.clone()),
        }
    }
    (terms, unknown)
}

/// What a search found under the root.
#[derive(Debug, Default)]
pub(crate) struct AssetMatches {
    /// The matching entries, in the walk's order.
    pub(crate) entries: Vec<Entry>,
    /// `t:` terms naming no kind, for the panel to say so.
    pub(crate) unknown: Vec<String>,
}

/// The entries of `everything` that `query` matches: the file name holds
/// every word, and the entry is of any of the `t:` kinds — any, not all,
/// because a file is one kind, so `t:texture t:atlas` can only mean either
/// (Unity's rule too). A term that names no kind matches nothing and is
/// reported.
pub(crate) fn matching(everything: &[Entry], query: &Query) -> AssetMatches {
    let (terms, unknown) = kind_terms(query);
    if !unknown.is_empty() {
        return AssetMatches {
            entries: Vec::new(),
            unknown,
        };
    }
    let entries = everything
        .iter()
        .filter(|entry| terms.is_empty() || terms.iter().any(|term| term.matches(entry)))
        .filter(|entry| query.name_matches(&entry.name))
        .cloned()
        .collect();
    AssetMatches {
        entries,
        unknown: Vec::new(),
    }
}

/// Every file and folder under `root`, depth first in listing order. A
/// folder that cannot be read is logged and skipped, as
/// [`browse::find`] does, so one bad folder hides nothing else.
pub(crate) fn walk(root: &Path) -> Vec<Entry> {
    let mut found = Vec::new();
    walk_into(root, None, &mut found);
    found
}

fn walk_into(root: &Path, dir: Option<&AssetPath>, found: &mut Vec<Entry>) {
    let entries = match browse::list(root, dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("skipping a folder while searching the assets: {e}");
            return;
        }
    };
    for entry in entries {
        let below = entry.is_dir().then(|| entry.path.clone());
        found.push(entry);
        if let Some(below) = below {
            walk_into(root, Some(&below), found);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> voltra_testkit::Scratch {
        let root = voltra_testkit::Scratch::new("voltra-asset-search");
        for path in [
            "sprites/coin.png",
            "sprites/coin.atlas.ron",
            "audio/coin.wav",
            "prefabs/coin.prefab.ron",
            "scenes/level.ron",
            "materials/flash.material.ron",
        ] {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().expect("a parent")).expect("a folder");
            std::fs::write(file, "").expect("written");
        }
        root
    }

    fn paths(found: &AssetMatches) -> Vec<&str> {
        found.entries.iter().map(|e| e.path.as_str()).collect()
    }

    #[test]
    fn a_name_is_found_in_every_folder() {
        let root = project();
        let everything = walk(&root);

        let found = matching(&everything, &Query::parse("COIN"));

        assert_eq!(
            paths(&found),
            [
                "audio/coin.wav",
                "prefabs/coin.prefab.ron",
                "sprites/coin.atlas.ron",
                "sprites/coin.png"
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_kind_narrows_and_two_kinds_are_either() {
        let root = project();
        let everything = walk(&root);
        let found = |text: &str| paths(&matching(&everything, &Query::parse(text))).join(" ");

        assert_eq!(found("t:sound"), "audio/coin.wav");
        assert_eq!(
            found("coin t:texture t:atlas"),
            "sprites/coin.atlas.ron sprites/coin.png"
        );
        assert_eq!(found("t:material"), "materials/flash.material.ron");
        assert_eq!(found("t:prefab"), "prefabs/coin.prefab.ron");
        assert_eq!(
            found("t:scene"),
            "prefabs/coin.prefab.ron scenes/level.ron",
            "a prefab is a scene"
        );
        assert_eq!(found("t:folder sc"), "scenes");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_kind_nothing_is_called_is_reported() {
        let found = matching(&[], &Query::parse("t:texure"));
        assert_eq!(found.unknown, ["texure"]);
    }
}
