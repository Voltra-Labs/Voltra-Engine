//! The scene the editor has open: which file it came from, and whether it
//! has changed since it was opened or saved.

use voltra_assets::AssetPath;

use crate::undo::History;

/// What the window title, Save and the "save changes?" prompt know about
/// the open scene.
///
/// Unsaved changes are the history's business, not a flag every panel must
/// remember to set: the scene is clean while the history stands where it
/// stood at the last open or save. That is Godot's answer, and it makes
/// undoing back to the save clean again, which a flag cannot.
#[derive(Debug, Default)]
pub(crate) struct SceneDocument {
    /// `None` until the scene has a file: a new scene, or the demo the
    /// editor starts with.
    path: Option<AssetPath>,
    /// The history's position when the scene was last opened or saved.
    saved_at: u64,
}

impl SceneDocument {
    pub(crate) fn path(&self) -> Option<&AssetPath> {
        self.path.as_ref()
    }

    /// The file's name, or "Untitled".
    pub(crate) fn name(&self) -> &str {
        self.path.as_ref().map_or("Untitled", |path| {
            path.as_str().rsplit('/').next().unwrap_or(path.as_str())
        })
    }

    pub(crate) fn is_dirty(&self, history: &History) -> bool {
        history.position() != self.saved_at
    }

    /// The scene was just replaced — opened from `path`, or made new — and
    /// the history cleared with it.
    pub(crate) fn replaced(&mut self, path: Option<AssetPath>, history: &History) {
        self.path = path;
        self.saved_at = history.position();
    }

    /// The scene was just written to `path`.
    pub(crate) fn saved(&mut self, path: AssetPath, history: &History) {
        self.replaced(Some(path), history);
    }

    /// What the window is called: the scene, a `*` while it has unsaved
    /// changes, then the editor — the order every editor uses, so a row of
    /// windows reads by what is in them.
    pub(crate) fn title(&self, history: &History) -> String {
        let star = if self.is_dirty(history) { "*" } else { "" };
        format!("{}{star} — Voltra Editor", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_untitled_scene_is_clean_until_edited_and_named_after_its_file_once_saved() {
        let history = History::default();
        let mut document = SceneDocument::default();
        assert_eq!(document.title(&history), "Untitled — Voltra Editor");
        assert!(!document.is_dirty(&history));

        let path = AssetPath::new("scenes/level.ron").expect("a path");
        document.saved(path, &history);
        assert_eq!(document.name(), "level.ron");
        assert_eq!(
            document.path().map(AssetPath::as_str),
            Some("scenes/level.ron")
        );
    }
}
