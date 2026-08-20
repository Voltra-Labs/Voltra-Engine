//! Reading the asset root as a list of things an editor can show.
//!
//! One concept, and deliberately not two: this knows about directories, names
//! and extensions, and nothing about textures, handles or the GPU. A browser
//! panel joins this to [`Textures`](crate::textures::Textures) the same way
//! `voltra_core::App` joins [`AssetWatcher`](crate::watch::AssetWatcher) to it.
//!
//! Every path handed out is an [`AssetPath`], so a name the invariant refuses —
//! a Windows alternate data stream, a byte sequence that is not UTF-8 — is
//! dropped here rather than becoming a string some caller later has to check.

use std::path::Path;

use crate::error::AssetError;
use crate::path::AssetPath;
use crate::watch::is_texture;

/// What one row of a listing is.
///
/// `Other` is listed rather than hidden: a browser that silently omits the
/// `.psd` an artist just copied in reads as a failed copy. It says "this is
/// here and this engine cannot draw it", which is Unity's answer too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    /// A file with an extension in
    /// [`TEXTURE_EXTENSIONS`](crate::watch::TEXTURE_EXTENSIONS).
    Texture,
    /// A file this engine has no loader for.
    Other,
}

/// One directory or file under the asset root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The last component, which is what a row says.
    pub name: String,
    /// Where it is, relative to the asset root.
    pub path: AssetPath,
    pub kind: EntryKind,
}

impl Entry {
    /// Whether this entry can be opened as a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == EntryKind::Directory
    }
}

/// Lists `dir` under `root`. `None` lists the root itself.
///
/// Sorted directories first and then by name, case-insensitively — the order
/// Unity, Unreal and Godot all show, and the one a human scanning a column
/// expects. The sort is here rather than in the caller because a listing that
/// arrives in `read_dir` order is different on every filesystem, and a browser
/// whose rows move between platforms is a bug reported as a flicker.
///
/// Dotfiles are skipped: `.gitkeep`, `.DS_Store` and `.import` sidecars are
/// bookkeeping, not assets.
pub fn list(root: &Path, dir: Option<&AssetPath>) -> Result<Vec<Entry>, AssetError> {
    let base = match dir {
        Some(dir) => root.join(dir.as_str()),
        None => root.to_path_buf(),
    };

    let reader = std::fs::read_dir(&base).map_err(|source| AssetError::Read {
        path: base.clone(),
        source,
    })?;

    let mut entries = Vec::new();
    for entry in reader {
        let entry = match entry {
            Ok(entry) => entry,
            // One unreadable entry must not lose the rest of the directory:
            // a file being written while the scan runs is normal, not an error.
            Err(e) => {
                log::warn!("skipping an entry of {}: {e}", base.display());
                continue;
            }
        };

        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            log::warn!("skipping a non-UTF-8 name in {}", base.display());
            continue;
        };
        if name.starts_with('.') {
            continue;
        }

        // Built from the parent's already-validated path plus one component, so
        // the only way this fails is a name the invariant refuses.
        let raw = match dir {
            Some(dir) => format!("{}/{}", dir.as_str(), name),
            None => name.clone(),
        };
        let path = match AssetPath::new(&raw) {
            Ok(path) => path,
            Err(e) => {
                log::warn!("skipping {raw:?}: {e}");
                continue;
            }
        };

        // `file_type` rather than `metadata`: it does not follow symlinks, so a
        // link pointing outside the root is listed as the file it is here and
        // never walked into.
        let is_dir = match entry.file_type() {
            Ok(kind) => kind.is_dir(),
            Err(e) => {
                log::warn!("skipping {raw:?}: {e}");
                continue;
            }
        };

        let kind = if is_dir {
            EntryKind::Directory
        } else if is_texture(&name) {
            EntryKind::Texture
        } else {
            EntryKind::Other
        };

        entries.push(Entry { name, path, kind });
    }

    entries.sort_by(|a, b| {
        b.is_dir()
            .cmp(&a.is_dir())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// The directory holding `dir`. `None` is the asset root, which has no parent.
///
/// Here rather than in the panel because it is the inverse of the join `list`
/// performs, and the two have to agree about the separator.
pub fn parent(dir: &AssetPath) -> Option<AssetPath> {
    let (head, _) = dir.as_str().rsplit_once('/')?;
    // Cannot fail: `head` is a non-empty prefix of an already validated path.
    AssetPath::new(head).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch tree: two directories, a PNG, a file with no loader, a
    /// dotfile.
    fn tree() -> std::path::PathBuf {
        let root = voltra_testkit::scratch_root();
        std::fs::create_dir_all(root.join("sprites")).expect("sprites dir");
        std::fs::create_dir_all(root.join("Scenes")).expect("scenes dir");
        std::fs::write(root.join("sprites/checker.png"), b"not really a png").expect("png");
        std::fs::write(root.join("sprites/source.psd"), b"layers").expect("psd");
        std::fs::write(root.join("notes.txt"), b"hello").expect("txt");
        std::fs::write(root.join(".gitkeep"), b"").expect("dotfile");
        root
    }

    #[test]
    fn directories_come_first_then_names_case_insensitively() {
        let root = tree();
        let names: Vec<_> = list(&root, None)
            .expect("the root lists")
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        // `Scenes` before `sprites` despite the capital, and both before the
        // file.
        assert_eq!(names, ["Scenes", "sprites", "notes.txt"]);
    }

    #[test]
    fn a_dotfile_is_not_an_asset() {
        let root = tree();
        let entries = list(&root, None).expect("the root lists");
        assert!(entries.iter().all(|entry| entry.name != ".gitkeep"));
    }

    #[test]
    fn extension_decides_what_is_loadable() {
        let root = tree();
        let dir = AssetPath::new("sprites").expect("a valid path");
        let entries = list(&root, Some(&dir)).expect("the directory lists");

        let checker = &entries[0];
        assert_eq!(checker.kind, EntryKind::Texture);
        assert_eq!(checker.path.as_str(), "sprites/checker.png");

        let source = &entries[1];
        assert_eq!(source.kind, EntryKind::Other);
    }

    #[test]
    fn an_uppercase_extension_is_still_a_texture() {
        let root = voltra_testkit::scratch_root();
        std::fs::write(root.join("SHOUT.PNG"), b"").expect("png");
        let entries = list(&root, None).expect("the root lists");
        assert_eq!(entries[0].kind, EntryKind::Texture);
    }

    #[test]
    fn a_missing_directory_is_an_error_rather_than_an_empty_listing() {
        let root = voltra_testkit::scratch_root();
        let dir = AssetPath::new("nowhere").expect("a valid path");
        assert!(list(&root, Some(&dir)).is_err());
    }

    #[test]
    fn parent_walks_up_to_the_root_and_stops() {
        let deep = AssetPath::new("sprites/enemies/red.png").expect("a valid path");
        let up = parent(&deep).expect("a parent");
        assert_eq!(up.as_str(), "sprites/enemies");

        let up = parent(&up).expect("a parent");
        assert_eq!(up.as_str(), "sprites");

        assert!(parent(&up).is_none());
    }
}
