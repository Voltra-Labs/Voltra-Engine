//! Which sorting layers the project draws with, and keeping them current.

use std::path::Path;

use voltra_assets::AssetPath;
use voltra_scene::SortingLayers;

use super::App;

/// The file a project's sorting layers come from, relative to the asset root:
/// `(version: 1, layers: ["Background", "Default", "Foreground"])`.
///
/// A convention, as [`DEFAULT_INPUT_MAP`](super::DEFAULT_INPUT_MAP) is: the
/// editor and the game both read it, so the layers the inspector offers are
/// the ones the game sorts by. `*.layers.ron` keeps it out of the scene list.
pub const DEFAULT_SORTING_LAYERS: &str = "default.layers.ron";

/// The format version this build reads.
const VERSION: u32 = 1;

#[derive(serde::Deserialize)]
struct LayersFile {
    version: u32,
    layers: Vec<String>,
}

/// The sorting layers a layers file's `text` holds, or why it holds none:
/// it does not parse, is of another version, or names an empty layer. What
/// the editor's project settings check an edit with before saving it.
pub fn parse_sorting_layers(text: &str) -> Result<SortingLayers, String> {
    let file: LayersFile =
        ron::from_str(text).map_err(|e| format!("not a sorting layers file: {e}"))?;
    if file.version != VERSION {
        return Err(format!(
            "sorting layers version {}; this build reads {VERSION}",
            file.version
        ));
    }
    if file.layers.iter().any(|name| name.trim().is_empty()) {
        return Err("an empty sorting layer name".to_owned());
    }
    Ok(SortingLayers::new(file.layers))
}

/// Reads a layers file. `None`, with the reason logged, for one that will
/// not read or does not parse.
fn read(path: &Path) -> Option<SortingLayers> {
    let text = match voltra_assets::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            log::warn!("could not read {}: {e}", path.display());
            return None;
        }
    };
    match parse_sorting_layers(&text) {
        Ok(layers) => Some(layers),
        Err(e) => {
            log::warn!("{}: {e}", path.display());
            None
        }
    }
}

impl App {
    /// Reads the project's layers at startup, unless code already set them.
    /// No file is normal — the default list stands.
    pub(super) fn load_sorting_layers(&mut self, root: &Path) {
        if self.sorting_layers_from_code {
            return;
        }
        let full = root.join(DEFAULT_SORTING_LAYERS);
        if !voltra_assets::fs::exists(&full) {
            return;
        }
        if let Some(layers) = read(&full) {
            log::info!(
                "sorting layers from {DEFAULT_SORTING_LAYERS}: {}",
                layers.names().join(", ")
            );
            self.sorting_layers = layers;
        }
    }

    /// Re-reads the layers if `path` is their file. Returns whether it was,
    /// whether or not it read — a broken save keeps the layers in use.
    pub(super) fn reload_sorting_layers(&mut self, path: &AssetPath) -> bool {
        if path.as_str() != DEFAULT_SORTING_LAYERS {
            return false;
        }
        if self.sorting_layers_from_code {
            return true;
        }
        let root = self
            .asset_root
            .clone()
            .unwrap_or_else(voltra_assets::default_root);
        if let Some(layers) = read(&root.join(DEFAULT_SORTING_LAYERS)) {
            self.sorting_layers = layers;
            log::info!("reloaded {DEFAULT_SORTING_LAYERS}");
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_with(text: &str) -> std::path::PathBuf {
        let root = voltra_testkit::scratch_root();
        std::fs::write(root.join(DEFAULT_SORTING_LAYERS), text).expect("writes");
        root
    }

    #[test]
    fn the_conventional_file_sets_the_layers() {
        let root = root_with(r#"(version: 1, layers: ["Sky", "Default", "Fx"])"#);
        let mut app = App::default();
        app.load_sorting_layers(&root);
        assert_eq!(app.sorting_layers.names(), &["Sky", "Default", "Fx"]);
    }

    #[test]
    fn no_file_keeps_the_default_list() {
        let mut app = App::default();
        app.load_sorting_layers(&voltra_testkit::scratch_root());
        assert_eq!(app.sorting_layers.names().len(), 3);
    }

    #[test]
    fn a_broken_file_keeps_what_was_there() {
        let root = root_with(r#"(version: 1, layers: [""])"#);
        let mut app = App::default();
        app.load_sorting_layers(&root);
        assert_eq!(app.sorting_layers.names().len(), 3);
    }

    #[test]
    fn layers_set_in_code_win_over_the_file() {
        let root = root_with(r#"(version: 1, layers: ["Sky"])"#);
        let mut app = App::default().with_sorting_layers(["Ground", "Sky"]);
        app.load_sorting_layers(&root);
        assert_eq!(app.sorting_layers.rank("Sky"), 2);
    }

    #[test]
    fn a_reload_of_the_file_replaces_the_layers() {
        let root = root_with(r#"(version: 1, layers: ["A"])"#);
        let mut app = App::default().with_asset_root(&root);
        app.load_sorting_layers(&root);
        std::fs::write(
            root.join(DEFAULT_SORTING_LAYERS),
            r#"(version: 1, layers: ["A", "B"])"#,
        )
        .expect("writes");
        let path = AssetPath::new(DEFAULT_SORTING_LAYERS).expect("valid");
        assert!(app.reload_sorting_layers(&path));
        assert_eq!(app.sorting_layers.rank("B"), 2);
    }

    #[test]
    fn the_shipped_layers_file_reads() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(DEFAULT_SORTING_LAYERS);
        let layers = read(&path).expect("the shipped file reads");
        assert_eq!(layers.names(), &["Background", "Default", "Foreground"]);
    }
}
