//! The project's render settings, `default.render.ron`: read with the
//! first 3D frame, and again when the file changes.

use std::path::Path;

/// The file a project's render settings come from, relative to the asset
/// root:
///
/// ```ron
/// (version: 1, msaa: 4)
/// ```
///
/// A convention beside `default.input.ron` and `default.layers.ron`, edited
/// in the editor's Project Settings. Missing means the defaults. Read only
/// once a frame draws a 3D camera: a 2D game never opens it.
pub const DEFAULT_RENDER_SETTINGS: &str = "default.render.ron";

/// The format version this build reads.
const VERSION: u32 = 1;

/// What `default.render.ron` sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSettings {
    /// Samples a pixel of a 3D shot: 1 (off) or 4, the default.
    pub msaa: u32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            msaa: voltra_render::render3d::Hdr3D::DEFAULT_SAMPLES,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderFile {
    version: u32,
    #[serde(default = "default_msaa")]
    msaa: u32,
}

fn default_msaa() -> u32 {
    RenderSettings::default().msaa
}

/// The settings a render file's `text` holds, or why it holds none: it
/// does not parse, is of another version, or asks for an MSAA other than 1
/// or 4. What Project Settings checks an edit with before saving it.
pub fn parse_render_settings(text: &str) -> Result<RenderSettings, String> {
    let file: RenderFile =
        ron::from_str(text).map_err(|e| format!("not a render settings file: {e}"))?;
    if file.version != VERSION {
        return Err(format!(
            "render settings version {}; this build reads {VERSION}",
            file.version
        ));
    }
    if !matches!(file.msaa, 1 | 4) {
        return Err(format!(
            "msaa {}: 1 (off) or 4 — what every device draws",
            file.msaa
        ));
    }
    Ok(RenderSettings { msaa: file.msaa })
}

/// The settings under `root`: the defaults when there is no file, and when
/// it will not read or parse — with the reason logged.
pub(in crate::app) fn read(root: &Path) -> RenderSettings {
    let path = root.join(DEFAULT_RENDER_SETTINGS);
    if !voltra_assets::fs::exists(&path) {
        return RenderSettings::default();
    }
    let parsed = voltra_assets::fs::read_to_string(&path)
        .map_err(|e| format!("could not read it: {e}"))
        .and_then(|text| parse_render_settings(&text));
    match parsed {
        Ok(settings) => {
            log::info!("render settings from {DEFAULT_RENDER_SETTINGS}: {settings:?}");
            settings
        }
        Err(e) => {
            log::warn!("{}: {e}; the defaults stand", path.display());
            RenderSettings::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_sets_msaa_and_a_missing_field_or_file_is_the_default() {
        assert_eq!(
            parse_render_settings("(version: 1, msaa: 1)"),
            Ok(RenderSettings { msaa: 1 })
        );
        assert_eq!(
            parse_render_settings("(version: 1)"),
            Ok(RenderSettings::default())
        );
        assert_eq!(RenderSettings::default().msaa, 4);
        let root = voltra_testkit::scratch_root();
        assert_eq!(read(&root), RenderSettings::default());
        std::fs::write(root.join(DEFAULT_RENDER_SETTINGS), "(version: 1, msaa: 1)")
            .expect("writes");
        assert_eq!(read(&root).msaa, 1);
    }

    #[test]
    fn a_broken_file_is_refused_with_why() {
        for (text, why) in [
            ("(version: 2, msaa: 4)", "version"),
            ("(version: 1, msaa: 8)", "msaa 8"),
            ("(version: 1, msaa: 0)", "msaa 0"),
            ("(version: 1, mssa: 4)", "not a render settings file"),
        ] {
            let e = parse_render_settings(text).expect_err(text);
            assert!(e.contains(why), "{text}: {e}");
        }
        let root = voltra_testkit::scratch_root();
        std::fs::write(root.join(DEFAULT_RENDER_SETTINGS), "(version: 1, msaa: 3)")
            .expect("writes");
        assert_eq!(read(&root), RenderSettings::default());
    }

    #[test]
    fn the_shipped_file_reads() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let text =
            std::fs::read_to_string(path.join(DEFAULT_RENDER_SETTINGS)).expect("it is shipped");
        assert_eq!(parse_render_settings(&text), Ok(RenderSettings::default()));
    }
}
