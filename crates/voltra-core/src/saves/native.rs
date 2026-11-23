//! Save slots on a desktop: files in the per-user folder.

use std::io;
use std::path::{Path, PathBuf};

/// Writes `text` to `path` through a temporary and a rename, creating the
/// folder first.
pub(super) fn write(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)
}

/// How many bytes `path` holds, or `None` when there is nothing there.
pub(super) fn size(path: &Path) -> io::Result<Option<u64>> {
    match std::fs::metadata(path) {
        Ok(meta) => Ok(Some(meta.len())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

pub(super) fn read(path: &Path) -> io::Result<String> {
    std::fs::read_to_string(path)
}

/// Removes `path`; whether there was anything to remove.
pub(super) fn remove(path: &Path) -> io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

/// The names of the files directly in `dir`; none when it cannot be read.
pub(super) fn file_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}
