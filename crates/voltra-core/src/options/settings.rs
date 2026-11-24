//! What an options screen keeps besides the controls: the display and the
//! volumes, one file in the player's folder, applied when the game starts.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::app::Tick;
use crate::saves::{SaveError, Saves};
use crate::window::Fullscreen;

/// The file the settings are kept in, beside the save slots — on the web,
/// the key `/<game>/saves/options.settings.ron`.
pub const OPTIONS_FILE: &str = "options.settings.ron";

/// The format version this build reads and writes.
pub const VERSION: u32 = 1;

/// The display and the volumes as the player chose them.
///
/// Every field but the version has a default, so a file written before a
/// field existed still reads.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    /// The format version; must equal [`VERSION`].
    pub version: u32,
    /// How the window fills the screen.
    #[serde(default)]
    pub fullscreen: Fullscreen,
    /// The windowed size chosen, in physical pixels; `None` keeps the size
    /// the window opens at.
    #[serde(default)]
    pub windowed_size: Option<(u32, u32)>,
    /// Whether frames wait for the display's refresh.
    #[serde(default = "on")]
    pub vsync: bool,
    /// Each bus's volume, by name, `0` to `1`.
    #[serde(default)]
    pub volumes: BTreeMap<String, f32>,
}

fn on() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: VERSION,
            fullscreen: Fullscreen::Windowed,
            windowed_size: None,
            vsync: true,
            volumes: BTreeMap::new(),
        }
    }
}

/// Why the settings could not be read.
#[derive(Debug)]
pub enum SettingsError {
    /// The file could not be reached or did not parse.
    Storage(SaveError),
    /// The file is another version.
    UnsupportedVersion(u32),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(e) => write!(f, "the options: {e}"),
            Self::UnsupportedVersion(found) => write!(
                f,
                "the options file is version {found}; this build reads {VERSION}"
            ),
        }
    }
}

impl std::error::Error for SettingsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(e) => Some(e),
            Self::UnsupportedVersion(_) => None,
        }
    }
}

impl Settings {
    /// The settings `saves`' folder holds, or `None` before any were kept.
    pub fn load(saves: &Saves) -> Result<Option<Self>, SettingsError> {
        let Some(settings) = saves
            .load_file::<Self>(OPTIONS_FILE)
            .map_err(SettingsError::Storage)?
        else {
            return Ok(None);
        };
        if settings.version != VERSION {
            return Err(SettingsError::UnsupportedVersion(settings.version));
        }
        Ok(Some(settings))
    }

    /// Keeps these settings in `saves`' folder.
    pub fn save(&self, saves: &Saves) -> Result<(), SaveError> {
        saves.save_file(OPTIONS_FILE, self)
    }

    /// The settings the game runs with now: the window as it is, and every
    /// bus's volume. What a screen starts from when nothing was kept.
    pub fn current(tick: &Tick<'_>) -> Self {
        let buses = tick.audio.buses();
        let volumes = buses
            .names()
            .filter_map(|name| {
                let bus = buses.find(name)?;
                Some((name.to_owned(), tick.audio.bus_volume(bus)))
            })
            .collect();
        Self {
            version: VERSION,
            fullscreen: tick.window.fullscreen(),
            windowed_size: None,
            vsync: tick.window.vsync(),
            volumes,
        }
    }

    /// The volume kept for `bus`, `1` when none was.
    pub fn volume(&self, bus: &str) -> f32 {
        self.volumes.get(bus).copied().unwrap_or(1.0)
    }

    /// Asks the window for the display and sets every bus kept — from the
    /// next frame on, as `tick.window` does. A volume out of `0..=1`, or not
    /// a number, is kept in range.
    pub fn apply(&self, tick: &mut Tick<'_>) {
        if let Some((width, height)) = self.windowed_size {
            tick.window.set_size(width, height);
        }
        tick.window.set_fullscreen(self.fullscreen);
        tick.window.set_vsync(self.vsync);
        for (name, volume) in &self.volumes {
            let volume = if volume.is_finite() {
                volume.clamp(0.0, 1.0)
            } else {
                1.0
            };
            let bus = tick.audio.bus(name);
            tick.audio.set_bus_volume(bus, volume, Duration::ZERO);
        }
    }

    /// Reads the kept settings and applies them: what a game calls when it
    /// starts. Nothing kept is nothing to do; a file that will not read is
    /// a warning, and the game runs as it opens. Answers what was applied.
    pub fn apply_saved(tick: &mut Tick<'_>) -> Option<Self> {
        // No folder (a test's `App::default`): nothing kept, nothing said.
        tick.saves.dir()?;
        match Self::load(tick.saves) {
            Ok(Some(settings)) => {
                settings.apply(tick);
                log::info!("applied the player's options");
                Some(settings)
            }
            Ok(None) => None,
            Err(e) => {
                log::warn!("{e}; running with the defaults");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_another_version_is_refused() {
        let saves = Saves::in_dir(voltra_testkit::scratch_root());
        assert!(Settings::load(&saves).expect("reads").is_none());
        let mut settings = Settings {
            fullscreen: Fullscreen::Borderless,
            windowed_size: Some((1600, 900)),
            vsync: false,
            ..Settings::default()
        };
        settings.volumes.insert("music".to_owned(), 0.4);
        settings.save(&saves).expect("saves");
        assert_eq!(
            Settings::load(&saves).expect("reads"),
            Some(settings.clone())
        );

        settings.version = 7;
        settings.save(&saves).expect("saves");
        assert!(matches!(
            Settings::load(&saves),
            Err(SettingsError::UnsupportedVersion(7))
        ));
    }

    #[test]
    fn a_file_with_fields_missing_reads_with_their_defaults() {
        let saves = Saves::in_dir(voltra_testkit::scratch_root());
        saves
            .write_text(OPTIONS_FILE, "(version: 1)")
            .expect("writes");
        let settings = Settings::load(&saves).expect("reads").expect("one");
        assert!(settings.vsync);
        assert_eq!(settings.volume("music"), 1.0);
    }
}
