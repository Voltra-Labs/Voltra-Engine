//! Every asset of one kind under the root, however deep.

use std::path::Path;

use super::{list, EntryKind};
use crate::AssetPath;

/// Every file of `kind` under `root`, in listing order: each directory's
/// files and subdirectories as [`list`] sorts them, depth first. What an
/// "open scene" picker shows, or a tool listing a project's scenes.
///
/// Built on [`list`], so the same rules hold: dotfiles skipped, links never
/// walked into. A directory that cannot be read is logged and skipped —
/// one unreadable folder must not hide every other scene.
pub fn find(root: &Path, kind: EntryKind) -> Vec<AssetPath> {
    let mut found = Vec::new();
    walk(root, None, kind, &mut found);
    found
}

fn walk(root: &Path, dir: Option<&AssetPath>, kind: EntryKind, found: &mut Vec<AssetPath>) {
    let entries = match list(root, dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::warn!("skipping a directory while looking for assets: {e}");
            return;
        }
    };
    for entry in entries {
        if entry.is_dir() {
            walk(root, Some(&entry.path), kind, found);
        } else if entry.kind == kind {
            found.push(entry.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_are_found_in_every_folder_and_nothing_else_is() {
        let root = std::env::temp_dir().join(format!(
            "voltra-find-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in [
            ("a.ron", "()"),
            ("levels/deep/b.ron", "()"),
            ("levels/c.png", ""),
            ("levels/run.anim.ron", "()"),
            (".hidden/d.ron", "()"),
        ] {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().expect("a parent")).expect("a dir");
            std::fs::write(file, text).expect("written");
        }
        let found: Vec<String> = find(&root, EntryKind::Scene)
            .iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        assert_eq!(found, ["levels/deep/b.ron", "a.ron"]);
        assert!(find(&root.join("missing"), EntryKind::Scene).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
