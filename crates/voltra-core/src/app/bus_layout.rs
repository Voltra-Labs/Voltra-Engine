//! The project's audio buses and their effects, and keeping them current.

use std::path::Path;

use voltra_assets::AssetPath;
use voltra_audio::{check_layout, BusSettings};

use super::App;

/// The file a project's buses come from, relative to the asset root:
///
/// ```ron
/// (version: 1, buses: [
///     (name: "music", volume: 0.8, effects: [
///         LowPass(cutoff: 18000.0),
///         Duck(by: "dialogue", depth: 0.7, attack: 0.05, release: 0.5),
///     ]),
///     (name: "sfx", effects: [Reverb(room_size: 0.6, mix: 0.25)]),
///     (name: "dialogue"),
/// ])
/// ```
///
/// Godot's `default_bus_layout.tres`, Unity's `AudioMixer` asset: the
/// editor's Project Settings edits it and the game reads it, so the mix a
/// designer hears is the one that ships. A convention, like
/// [`DEFAULT_SORTING_LAYERS`](super::DEFAULT_SORTING_LAYERS); `*.buses.ron`
/// keeps it out of the scene list.
pub const DEFAULT_BUS_LAYOUT: &str = "default.buses.ron";

/// The format version this build reads.
const VERSION: u32 = 1;

#[derive(serde::Deserialize)]
struct BusesFile {
    version: u32,
    buses: Vec<BusSettings>,
}

/// The buses a layout file's `text` holds, or why it holds none: it does not
/// parse, is of another version, names a bus twice or an effect with a
/// number that means nothing. What the editor's Project Settings checks an
/// edit with before saving it.
pub fn parse_bus_layout(text: &str) -> Result<Vec<BusSettings>, String> {
    let file: BusesFile = ron::from_str(text).map_err(|e| format!("not a bus layout file: {e}"))?;
    if file.version != VERSION {
        return Err(format!(
            "bus layout version {}; this build reads {VERSION}",
            file.version
        ));
    }
    check_layout(&file.buses)?;
    Ok(file.buses)
}

/// Reads a layout file. `None`, with the reason logged, for one that will
/// not read or does not parse.
fn read(path: &Path) -> Option<Vec<BusSettings>> {
    let text = match voltra_assets::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            log::warn!("could not read {}: {e}", path.display());
            return None;
        }
    };
    match parse_bus_layout(&text) {
        Ok(layout) => Some(layout),
        Err(e) => {
            log::warn!("{}: {e}", path.display());
            None
        }
    }
}

impl App {
    /// Lays the buses out from the project's file at startup. No file is
    /// normal — the default buses stand, with no effects.
    ///
    /// Applied on top of whatever table the game gave
    /// [`with_buses`](Self::with_buses): a layout adds and configures buses
    /// by name and never renumbers them.
    pub(super) fn load_bus_layout(&mut self, root: &Path) {
        let full = root.join(DEFAULT_BUS_LAYOUT);
        if !voltra_assets::fs::exists(&full) {
            return;
        }
        if let Some(layout) = read(&full) {
            log::info!(
                "audio buses from {DEFAULT_BUS_LAYOUT}: {}",
                layout
                    .iter()
                    .map(|bus| bus.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            self.audio.apply_layout(&layout);
        }
    }

    /// Re-reads the layout if `path` is its file. Returns whether it was,
    /// whether or not it read — a broken save keeps the mix in use.
    pub(super) fn reload_bus_layout(&mut self, path: &AssetPath) -> bool {
        if path.as_str() != DEFAULT_BUS_LAYOUT {
            return false;
        }
        let root = self
            .asset_root
            .clone()
            .unwrap_or_else(voltra_assets::default_root);
        if let Some(layout) = read(&root.join(DEFAULT_BUS_LAYOUT)) {
            self.audio.apply_layout(&layout);
            log::info!("reloaded {DEFAULT_BUS_LAYOUT}");
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_audio::EffectDesc;

    fn root_with(text: &str) -> std::path::PathBuf {
        let root = voltra_testkit::scratch_root();
        std::fs::write(root.join(DEFAULT_BUS_LAYOUT), text).expect("writes");
        root
    }

    #[test]
    fn the_conventional_file_lays_the_buses_out() {
        let root = root_with(
            r#"(version: 1, buses: [
                (name: "music", volume: 0.5, effects: [LowPass(cutoff: 900.0)]),
                (name: "dialogue"),
            ])"#,
        );
        let mut app = App::default();
        app.load_bus_layout(&root);

        let music = app.audio.buses().find("music").expect("there");
        assert_eq!(app.audio.bus_volume(music), 0.5);
        assert!(matches!(
            app.audio.bus_effects(music),
            [EffectDesc::LowPass { cutoff, .. }] if *cutoff == 900.0
        ));
        assert!(app.audio.buses().find("dialogue").is_some());
    }

    #[test]
    fn no_file_keeps_the_default_buses() {
        let mut app = App::default();
        app.load_bus_layout(&voltra_testkit::scratch_root());
        let music = app.audio.buses().find("music").expect("a default bus");
        assert!(app.audio.bus_effects(music).is_empty());
    }

    #[test]
    fn a_broken_file_is_refused_with_why_and_changes_nothing() {
        assert!(parse_bus_layout(r#"(version: 2, buses: [])"#)
            .expect_err("a version")
            .contains("version"));
        assert!(parse_bus_layout(r#"(version: 1, buses: [(name: "a"), (name: "a")])"#).is_err());
        assert!(
            parse_bus_layout(r#"(version: 1, buses: [(name: "a", effects: [Bogus])])"#).is_err()
        );

        let root = root_with(r#"(version: 1, buses: [(name: "music", volume: -1.0)])"#);
        let mut app = App::default();
        app.load_bus_layout(&root);
        let music = app.audio.buses().find("music").expect("there");
        assert_eq!(app.audio.bus_volume(music), 1.0);
    }

    #[test]
    fn a_reload_of_the_file_replaces_the_layout() {
        let root = root_with(r#"(version: 1, buses: [(name: "music", volume: 0.5)])"#);
        let mut app = App::default().with_asset_root(&root);
        app.load_bus_layout(&root);
        std::fs::write(
            root.join(DEFAULT_BUS_LAYOUT),
            r#"(version: 1, buses: [(name: "sfx", effects: [Delay(time: 0.2)])])"#,
        )
        .expect("writes");

        let path = AssetPath::new(DEFAULT_BUS_LAYOUT).expect("valid");
        assert!(app.reload_bus_layout(&path));

        let music = app.audio.buses().find("music").expect("there");
        let sfx = app.audio.buses().find("sfx").expect("there");
        assert_eq!(app.audio.bus_volume(music), 1.0, "dropped from the file");
        assert_eq!(app.audio.bus_effects(sfx).len(), 1);
    }

    #[test]
    fn the_shipped_layout_reads() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(DEFAULT_BUS_LAYOUT);
        let layout = read(&path).expect("the shipped file reads");
        assert!(layout.iter().any(|bus| bus.name == "music"));
    }
}
