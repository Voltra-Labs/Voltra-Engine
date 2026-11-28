//! The project's settings files: the input map, the sorting layers, the audio
//! buses, the render settings and the build manifest — read, checked and
//! saved as the files they already are.

use std::path::{Path, PathBuf};

use voltra_core::input::ActionMap;
use voltra_core::{
    parse_bus_layout, parse_render_settings, parse_sorting_layers, DEFAULT_BUS_LAYOUT,
    DEFAULT_INPUT_MAP, DEFAULT_RENDER_SETTINGS, DEFAULT_SORTING_LAYERS,
};

/// One of the files the Project Settings window edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsFile {
    /// `default.input.ron`, under the asset root.
    InputMap,
    /// `default.layers.ron`, under the asset root.
    SortingLayers,
    /// `default.buses.ron`, under the asset root: each bus's volume and
    /// effects.
    Buses,
    /// `default.render.ron`, under the asset root: 3D's MSAA.
    Render,
    /// `build.ron`, beside the asset root.
    Build,
}

impl SettingsFile {
    pub(crate) const ALL: [Self; 5] = [
        Self::InputMap,
        Self::SortingLayers,
        Self::Buses,
        Self::Render,
        Self::Build,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::InputMap => "Input map",
            Self::SortingLayers => "Sorting layers",
            Self::Buses => "Audio buses",
            Self::Render => "Render",
            Self::Build => "Build",
        }
    }

    /// The name a tool asks for it by.
    pub(crate) fn named(name: &str) -> Option<Self> {
        match name {
            "input" | "input_map" => Some(Self::InputMap),
            "layers" | "sorting_layers" => Some(Self::SortingLayers),
            "buses" | "audio" | "audio_buses" => Some(Self::Buses),
            "render" | "rendering" => Some(Self::Render),
            "build" => Some(Self::Build),
            _ => None,
        }
    }

    /// Where it is for the project whose asset root is `assets`.
    pub(crate) fn path(self, assets: &Path) -> PathBuf {
        match self {
            Self::InputMap => assets.join(DEFAULT_INPUT_MAP),
            Self::SortingLayers => assets.join(DEFAULT_SORTING_LAYERS),
            Self::Buses => assets.join(DEFAULT_BUS_LAYOUT),
            Self::Render => assets.join(DEFAULT_RENDER_SETTINGS),
            Self::Build => assets
                .parent()
                .filter(|dir| !dir.as_os_str().is_empty())
                .unwrap_or(Path::new("."))
                .join(voltra_player::MANIFEST_FILE),
        }
    }

    /// Whether `text` is a file of this kind the engine reads, and why not.
    pub(crate) fn check(self, text: &str, path: &Path) -> Result<(), String> {
        match self {
            Self::InputMap => ActionMap::from_ron(text, path)
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::SortingLayers => parse_sorting_layers(text).map(|_| ()),
            Self::Buses => parse_bus_layout(text).map(|_| ()),
            Self::Render => parse_render_settings(text).map(|_| ()),
            Self::Build => voltra_player::Manifest::from_ron(text, path)
                .map(|_| ())
                .map_err(|e| e.to_string()),
        }
    }
}

/// The file's text, or an empty one for a project that has none yet.
pub(crate) fn read(file: SettingsFile, assets: &Path) -> Result<String, String> {
    let path = file.path(assets);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Checks `text` and saves it as the file, through a temporary file and a
/// rename, so a crash mid-write leaves the old file. A file that would not
/// read is refused, with why, and nothing is written. The engine's watch
/// reads the saved file back: the bindings and layers apply at once.
pub(crate) fn write(file: SettingsFile, assets: &Path, text: &str) -> Result<PathBuf, String> {
    let path = file.path(assets);
    file.check(text, &path)?;
    let temporary = path.with_extension("ron.tmp");
    std::fs::write(&temporary, text)
        .and_then(|()| std::fs::rename(&temporary, &path))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    log::info!("saved {}", path.display());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The project's folder, removed when dropped, and its `assets`.
    fn project() -> (voltra_testkit::Scratch, PathBuf) {
        let dir = voltra_testkit::Scratch::new("voltra-settings");
        std::fs::create_dir_all(dir.join("assets")).expect("folders");
        let assets = dir.join("assets");
        (dir, assets)
    }

    #[test]
    fn layers_are_saved_and_read_back() {
        let (_project, assets) = project();
        let text = r#"(version: 1, layers: ["Sky", "Default", "Fx"])"#;
        let path = write(SettingsFile::SortingLayers, &assets, text).expect("saves");
        assert_eq!(path, assets.join(DEFAULT_SORTING_LAYERS));
        assert_eq!(
            read(SettingsFile::SortingLayers, &assets),
            Ok(text.to_owned())
        );
    }

    #[test]
    fn a_file_the_engine_would_not_read_is_refused_and_not_written() {
        let (_project, assets) = project();
        let error = write(
            SettingsFile::SortingLayers,
            &assets,
            r#"(version: 1, layers: [""])"#,
        )
        .expect_err("an empty name");
        assert!(error.contains("empty"), "{error}");
        assert_eq!(
            read(SettingsFile::SortingLayers, &assets),
            Ok(String::new())
        );
        assert!(write(SettingsFile::InputMap, &assets, "(version: 1, actions: {").is_err());
        assert!(write(SettingsFile::Build, &assets, "(nope)").is_err());
    }

    #[test]
    fn buses_are_saved_read_back_and_refused_when_they_would_not_mix() {
        let (_project, assets) = project();
        let text = r#"(version: 1, buses: [(name: "music", effects: [Reverb(mix: 0.2)])])"#;
        let path = write(SettingsFile::Buses, &assets, text).expect("saves");
        assert_eq!(path, assets.join(DEFAULT_BUS_LAYOUT));
        assert_eq!(read(SettingsFile::Buses, &assets), Ok(text.to_owned()));

        let ducked_by_itself =
            r#"(version: 1, buses: [(name: "music", effects: [Duck(by: "music")])])"#;
        let error = write(SettingsFile::Buses, &assets, ducked_by_itself).expect_err("refused");
        assert!(error.contains("itself"), "{error}");
        assert_eq!(read(SettingsFile::Buses, &assets), Ok(text.to_owned()));
        assert_eq!(SettingsFile::named("buses"), Some(SettingsFile::Buses));
    }

    #[test]
    fn render_settings_are_saved_and_an_msaa_no_device_draws_refused() {
        let (_project, assets) = project();
        let path = write(SettingsFile::Render, &assets, "(version: 1, msaa: 1)").expect("saves");
        assert_eq!(path, assets.join(DEFAULT_RENDER_SETTINGS));
        let error = write(SettingsFile::Render, &assets, "(version: 1, msaa: 3)").expect_err("3");
        assert!(error.contains("msaa 3"), "{error}");
        assert_eq!(
            read(SettingsFile::Render, &assets),
            Ok("(version: 1, msaa: 1)".to_owned())
        );
        assert_eq!(SettingsFile::named("render"), Some(SettingsFile::Render));
    }

    #[test]
    fn the_build_manifest_is_beside_the_assets() {
        let (_project, assets) = project();
        let text = r#"(version: 1, name: "Game", scene: "scenes/a.ron")"#;
        let path = write(SettingsFile::Build, &assets, text).expect("saves");
        assert_eq!(path, assets.parent().expect("a parent").join("build.ron"));
    }
}
