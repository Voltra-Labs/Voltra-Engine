//! Where every asset's bytes come from: the disk, or a pack mounted in
//! memory.
//!
//! Every store, scene, prefab and settings file reads through here rather
//! than `std::fs`, so the one place that knows how a file is found is this
//! one. A browser has no disk: a web build fetches a pack of the game's
//! assets before it starts and [`mount`]s it, and every read below answers
//! from it. Natively nothing is mounted and reads go to the disk — unless a
//! test mounts a pack, which is how the web path is tested without a
//! browser.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock};

/// A pack's files by their path relative to its root, `/`-separated.
type Files = HashMap<String, Vec<u8>>;

/// The mounted packs, each under its root.
static MOUNTED: RwLock<Vec<(PathBuf, Files)>> = RwLock::new(Vec::new());

/// Serves `files` — relative paths, `/`-separated, and their bytes — as if
/// they were on disk under `root`. A later mount of the same root replaces
/// the earlier one.
pub fn mount(root: impl Into<PathBuf>, files: impl IntoIterator<Item = (String, Vec<u8>)>) {
    let root = root.into();
    let files: Files = files.into_iter().collect();
    let mut mounted = MOUNTED.write().unwrap_or_else(PoisonError::into_inner);
    mounted.retain(|(existing, _)| *existing != root);
    mounted.push((root, files));
}

/// Takes the pack at `root` away again.
pub fn unmount(root: &Path) {
    let mut mounted = MOUNTED.write().unwrap_or_else(PoisonError::into_inner);
    mounted.retain(|(existing, _)| existing != root);
}

/// The bytes of the file at `path`: from a pack mounted over it, else from
/// the disk (natively; the web has none).
pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    if let Some(bytes) = from_pack(path) {
        return bytes;
    }
    from_disk(path)
}

/// [`read`], as UTF-8 text.
pub fn read_to_string(path: &Path) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// The pack's answer for `path`, when a mounted pack covers it: its bytes,
/// or not found — a path inside a pack's root that the pack lacks is
/// missing, not looked for on disk, so a web build and a test behave alike.
fn from_pack(path: &Path) -> Option<io::Result<Vec<u8>>> {
    let mounted = MOUNTED.read().unwrap_or_else(PoisonError::into_inner);
    let (root, files) = mounted.iter().find(|(root, _)| path.starts_with(root))?;
    let relative = path.strip_prefix(root).ok()?;
    let key = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    Some(files.get(&key).cloned().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is not in the asset pack", root.join(relative).display()),
        )
    }))
}

#[cfg(not(target_arch = "wasm32"))]
fn from_disk(path: &Path) -> io::Result<Vec<u8>> {
    std::fs::read(path)
}

#[cfg(target_arch = "wasm32")]
fn from_disk(path: &Path) -> io::Result<Vec<u8>> {
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "{}: the web has no disk, and no asset pack covers it",
            path.display()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mounted_pack_answers_for_its_root_and_the_disk_for_the_rest() {
        let root = std::env::temp_dir().join(format!("voltra-pack-{}", std::process::id()));
        mount(
            &root,
            [("scenes/a.ron".to_owned(), b"(version: 1)".to_vec())],
        );
        assert_eq!(
            read_to_string(&root.join("scenes").join("a.ron")).expect("in the pack"),
            "(version: 1)"
        );
        let missing = read(&root.join("scenes/b.ron")).expect_err("not in the pack");
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);

        // Outside every pack: the disk.
        let outside = std::env::temp_dir().join(format!("voltra-disk-{}.txt", std::process::id()));
        std::fs::write(&outside, "disk").expect("written");
        assert_eq!(read_to_string(&outside).expect("on disk"), "disk");
        let _ = std::fs::remove_file(&outside);

        unmount(&root);
        assert!(
            read(&root.join("scenes/a.ron")).is_err(),
            "gone with its pack"
        );
    }
}
