//! The project a server works in, and the paths it may touch.

use std::fmt;
use std::path::{Path, PathBuf};

use voltra_assets::AssetPath;

/// A project: its root, and the asset root every path is inside.
#[derive(Debug, Clone)]
pub struct Project {
    root: PathBuf,
    assets: PathBuf,
}

/// Why a path was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// Not an asset path: absolute, empty, or climbing out with `..`.
    Invalid(String),
    /// It resolves outside the asset root — through a link, say.
    Outside(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(e) => write!(f, "not an asset path: {e}"),
            Self::Outside(p) => write!(f, "{p} resolves outside the asset root"),
        }
    }
}

impl std::error::Error for PathError {}

impl Project {
    /// The project at `root`, its assets in `root/assets` unless `assets`
    /// says otherwise (relative to `root`).
    pub fn new(root: impl Into<PathBuf>, assets: Option<PathBuf>) -> Self {
        let root = root.into();
        let assets = match assets {
            Some(path) if path.is_absolute() => path,
            Some(path) => root.join(path),
            None => root.join("assets"),
        };
        Self { root, assets }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn assets(&self) -> &Path {
        &self.assets
    }

    /// `raw` as an asset path, and where it is on disk — refused if it is
    /// not an asset path, or if the file (or, for one not made yet, its
    /// nearest existing parent) resolves outside the asset root.
    pub fn resolve(&self, raw: &str) -> Result<(AssetPath, PathBuf), PathError> {
        let path = AssetPath::new(raw).map_err(|e| PathError::Invalid(e.to_string()))?;
        let full = self.assets.join(path.as_str());
        let root = self
            .assets
            .canonicalize()
            .map_err(|_| PathError::Outside(raw.to_owned()))?;
        let mut probe = full.as_path();
        let existing = loop {
            if probe.exists() {
                break probe;
            }
            match probe.parent() {
                Some(parent) => probe = parent,
                None => return Err(PathError::Outside(raw.to_owned())),
            }
        };
        let real = existing
            .canonicalize()
            .map_err(|_| PathError::Outside(raw.to_owned()))?;
        if !real.starts_with(&root) {
            return Err(PathError::Outside(raw.to_owned()));
        }
        Ok((path, full))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_inside_resolves_and_one_that_leaves_is_refused() {
        let root = voltra_testkit::scratch_root();
        std::fs::create_dir_all(root.join("assets/scenes")).expect("dirs");
        let project = Project::new(&root, None);
        let (path, full) = project.resolve("scenes/new.ron").expect("inside");
        assert_eq!(path.as_str(), "scenes/new.ron");
        assert_eq!(full, root.join("assets/scenes/new.ron"));
        assert!(matches!(
            project.resolve("../secrets.txt"),
            Err(PathError::Invalid(_))
        ));
        assert!(matches!(
            project.resolve("/etc/passwd"),
            Err(PathError::Invalid(_))
        ));
        #[cfg(unix)]
        {
            let outside = voltra_testkit::scratch_root();
            std::os::unix::fs::symlink(&outside, root.join("assets/link")).expect("link");
            assert!(matches!(
                project.resolve("link/x.ron"),
                Err(PathError::Outside(_))
            ));
        }
    }
}
