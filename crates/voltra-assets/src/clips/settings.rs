//! A clip's settings file: which sound, streamed or not, and where it loops.

use crate::path::AssetPath;
use crate::watch::is_clip_settings;

/// The format version this build reads.
pub const VERSION: u32 = 1;

/// What a `*.clip.ron` says:
///
/// ```ron
/// (version: 1, file: "audio/theme.ogg", stream: true, loop_start: 4.0, loop_end: Some(62.5))
/// ```
///
/// The engine has no import sidecars, so the settings Unity keeps in an
/// `AudioClip`'s import settings (Load Type: Streaming) and Godot in an
/// `.import` file (`loop`, `loop_offset`) are an asset of their own, named
/// wherever a sound is: a scene's `AudioSource` naming `music/theme.clip.ron`
/// plays the file it points at, with these settings. A plain `.ogg` named
/// directly is resident and loops whole, as before.
///
/// `file` is a plain string, the way an atlas names its texture, rather than
/// the `Path("…")` a scene writes: this is a file people write by hand.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipSettings {
    /// The sound, relative to the asset root.
    pub file: AssetPath,
    /// Decode while playing rather than whole at load: for music and long
    /// ambiences, where the samples resident would be megabytes.
    pub stream: bool,
    /// Where a looping voice goes back to, in seconds from the start.
    pub loop_start: f64,
    /// Where a looping voice turns back, in seconds; `None` is the end.
    pub loop_end: Option<f64>,
}

/// The file as written.
#[derive(serde::Deserialize)]
struct SettingsFile {
    version: u32,
    file: String,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    loop_start: f64,
    #[serde(default)]
    loop_end: Option<f64>,
}

impl ClipSettings {
    /// The settings a file's `text` holds, or why it holds none.
    pub fn parse(text: &str) -> Result<Self, String> {
        let raw: SettingsFile =
            ron::from_str(text).map_err(|e| format!("not clip settings: {e}"))?;
        if raw.version != VERSION {
            return Err(format!(
                "clip settings version {}; this build reads {VERSION}",
                raw.version
            ));
        }
        let file =
            AssetPath::new(&raw.file).map_err(|e| format!("its file is not an asset path: {e}"))?;
        let settings = Self {
            file,
            stream: raw.stream,
            loop_start: raw.loop_start,
            loop_end: raw.loop_end,
        };
        if is_clip_settings(settings.file.as_str()) {
            return Err("its file is another clip's settings, not a sound".to_owned());
        }
        if !settings.loop_start.is_finite() || settings.loop_start < 0.0 {
            return Err(format!(
                "loop_start {} is not a time in the sound",
                settings.loop_start
            ));
        }
        if let Some(end) = settings.loop_end {
            if !end.is_finite() || end <= settings.loop_start {
                return Err(format!(
                    "loop_end {end} is not after loop_start {}",
                    settings.loop_start
                ));
            }
        }
        Ok(settings)
    }

    /// The loop's start and end in frames at `rate`.
    pub fn loop_frames(&self, rate: u32) -> (u64, Option<u64>) {
        let frames = |seconds: f64| (seconds * f64::from(rate)).round() as u64;
        (frames(self.loop_start), self.loop_end.map(frames))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_settings_file_reads() {
        let settings = ClipSettings::parse(
            r#"(version: 1, file: "audio/theme.ogg", stream: true, loop_start: 4.0, loop_end: Some(8.5))"#,
        )
        .expect("it reads");
        assert!(settings.stream);
        assert_eq!(settings.file.as_str(), "audio/theme.ogg");
        assert_eq!(settings.loop_frames(1_000), (4_000, Some(8_500)));
    }

    #[test]
    fn everything_but_the_file_may_be_left_out() {
        let settings = ClipSettings::parse(r#"(version: 1, file: "a.wav")"#).expect("it reads");
        assert!(!settings.stream);
        assert_eq!(settings.loop_frames(8), (0, None));
    }

    #[test]
    fn a_file_that_would_not_play_is_refused_with_why() {
        let refused = |text: &str| ClipSettings::parse(text).expect_err(text);
        assert!(refused(r#"(version: 2, file: "a.wav")"#).contains("version"));
        assert!(refused(r#"(version: 1, file: "b.clip.ron")"#).contains("settings"));
        assert!(refused(r#"(version: 1, file: "a.wav", loop_start: -1.0)"#).contains("loop_start"));
        assert!(
            refused(r#"(version: 1, file: "a.wav", loop_start: 2.0, loop_end: Some(1.0))"#)
                .contains("loop_end")
        );
        assert!(refused(r#"(version: 1, file: "../a.wav")"#).contains("asset path"));
    }
}
