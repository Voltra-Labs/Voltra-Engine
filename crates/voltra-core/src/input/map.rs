//! Named actions and the controls bound to each: Godot's `InputMap`, as a
//! file.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use super::binding::Binding;

/// The format version this build reads and writes.
pub const VERSION: u32 = 1;

/// What a game names its actions with, and the controls behind each.
///
/// A `BTreeMap` so a saved map lists its actions in the same order every
/// time, and a rebinding shows up in a diff as the one line it changed.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActionMap {
    pub version: u32,
    #[serde(default)]
    pub actions: BTreeMap<String, Action>,
}

/// How far an analog control must move before an action counts as held,
/// when a map does not say: past the drift of a worn stick, short of where a
/// deliberate push starts.
pub const DEFAULT_DEADZONE: f32 = 0.25;

/// The largest dead zone accepted. At `1` nothing could ever exceed it.
pub const MAX_DEADZONE: f32 = 0.95;

/// One action's bindings, any of which drives it, and its dead zone.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Action {
    #[serde(default)]
    pub bindings: Vec<Binding>,
    /// Analog travel, `0` to `1`, below which the action is not held. Keys
    /// are all-or-nothing and always clear it.
    #[serde(default = "default_deadzone")]
    pub deadzone: f32,
}

impl Default for Action {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            deadzone: DEFAULT_DEADZONE,
        }
    }
}

fn default_deadzone() -> f32 {
    DEFAULT_DEADZONE
}

/// Why a map could not be read or written.
#[derive(Debug)]
pub enum MapError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: Box<ron::error::SpannedError>,
    },
    Serialize(ron::Error),
    UnsupportedVersion {
        path: PathBuf,
        found: u32,
    },
    EmptyActionName {
        path: PathBuf,
    },
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "could not access {}: {source}", path.display()),
            Self::Parse { path, source } => {
                write!(f, "could not parse {}: {source}", path.display())
            }
            Self::Serialize(source) => write!(f, "could not write an input map: {source}"),
            Self::UnsupportedVersion { path, found } => write!(
                f,
                "{} is input map version {found}; this build reads {VERSION}",
                path.display()
            ),
            Self::EmptyActionName { path } => {
                write!(f, "{} names an action with an empty name", path.display())
            }
        }
    }
}

impl std::error::Error for MapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Serialize(source) => Some(source),
            Self::UnsupportedVersion { .. } | Self::EmptyActionName { .. } => None,
        }
    }
}

impl ActionMap {
    pub fn new() -> Self {
        Self {
            version: VERSION,
            actions: BTreeMap::new(),
        }
    }

    /// Parses a map from RON. `path` only names the source in errors.
    pub fn from_ron(text: &str, path: &Path) -> Result<Self, MapError> {
        let map: Self = ron::from_str(text).map_err(|source| MapError::Parse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        if map.version != VERSION {
            return Err(MapError::UnsupportedVersion {
                path: path.to_path_buf(),
                found: map.version,
            });
        }
        if map.actions.keys().any(|name| name.trim().is_empty()) {
            return Err(MapError::EmptyActionName {
                path: path.to_path_buf(),
            });
        }
        Ok(map)
    }

    /// Reads a map from `path`.
    pub fn load(path: &Path) -> Result<Self, MapError> {
        let text = std::fs::read_to_string(path).map_err(|source| MapError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_ron(&text, path)
    }

    /// The map as RON, pretty-printed for a diff.
    pub fn to_ron(&self) -> Result<String, MapError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(MapError::Serialize)
    }

    /// Writes the map to `path` — wherever the game keeps a player's
    /// settings. Through a temporary file and a rename, so a crash mid-write
    /// leaves the previous file rather than half of this one.
    pub fn save(&self, path: &Path) -> Result<(), MapError> {
        let text = self.to_ron()?;
        let io = |source| MapError::Io {
            path: path.to_path_buf(),
            source,
        };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".tmp");
        let temporary = PathBuf::from(temporary);
        std::fs::write(&temporary, text).map_err(io)?;
        std::fs::rename(&temporary, path).map_err(io)
    }

    /// The bindings of `action`, empty for an action the map does not have.
    pub fn bindings(&self, action: &str) -> &[Binding] {
        self.actions
            .get(action)
            .map_or(&[], |action| action.bindings.as_slice())
    }

    /// `action`'s dead zone, kept within `0..=MAX_DEADZONE` whatever the
    /// file says.
    pub fn deadzone(&self, action: &str) -> f32 {
        let deadzone = self
            .actions
            .get(action)
            .map_or(DEFAULT_DEADZONE, |action| action.deadzone);
        if deadzone.is_finite() {
            deadzone.clamp(0.0, MAX_DEADZONE)
        } else {
            DEFAULT_DEADZONE
        }
    }

    /// Sets `action`'s dead zone, creating the action.
    pub fn set_deadzone(&mut self, action: &str, deadzone: f32) {
        self.actions.entry(action.to_owned()).or_default().deadzone = deadzone;
    }

    /// Whether the map names `action` at all.
    pub fn contains(&self, action: &str) -> bool {
        self.actions.contains_key(action)
    }

    /// Adds `binding` to `action`, creating the action. Binding a control an
    /// action already has does nothing.
    pub fn bind(&mut self, action: &str, binding: Binding) {
        let bindings = &mut self.actions.entry(action.to_owned()).or_default().bindings;
        if !bindings.contains(&binding) {
            bindings.push(binding);
        }
    }

    /// Removes `binding` from `action`. Returns whether it was there.
    pub fn unbind(&mut self, action: &str, binding: Binding) -> bool {
        let Some(action) = self.actions.get_mut(action) else {
            return false;
        };
        let before = action.bindings.len();
        action.bindings.retain(|b| *b != binding);
        action.bindings.len() != before
    }

    /// "Press a key for jump": `action`'s binding at `slot` becomes
    /// `binding`, appended if the slot is past the end — and `binding` is
    /// taken off every other action, so one key never drives two of them by
    /// accident.
    pub fn rebind(&mut self, action: &str, slot: usize, binding: Binding) {
        for (name, other) in &mut self.actions {
            if name != action {
                other.bindings.retain(|b| *b != binding);
            }
        }
        let bindings = &mut self.actions.entry(action.to_owned()).or_default().bindings;
        bindings.retain(|b| *b != binding);
        if slot < bindings.len() {
            bindings[slot] = binding;
        } else {
            bindings.push(binding);
        }
    }

    /// Every action name, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.actions.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use winit::event::MouseButton;
    use winit::keyboard::KeyCode;

    use super::*;

    const FILE: &str = r#"(
        version: 1,
        actions: {
            "jump": (bindings: [Key(Space), Key(ArrowUp)]),
            "fire": (bindings: [Mouse(Left)]),
        },
    )"#;

    fn parsed() -> ActionMap {
        ActionMap::from_ron(FILE, Path::new("test.input.ron")).expect("parses")
    }

    #[test]
    fn a_file_names_its_actions_and_their_bindings() {
        let map = parsed();
        assert_eq!(
            map.bindings("jump"),
            &[Binding::Key(KeyCode::Space), Binding::Key(KeyCode::ArrowUp)]
        );
        assert_eq!(map.bindings("fire"), &[Binding::Mouse(MouseButton::Left)]);
        assert!(map.bindings("nothing").is_empty());
    }

    #[test]
    fn a_map_round_trips_through_ron() {
        let map = parsed();
        let text = map.to_ron().expect("writes");
        assert_eq!(
            ActionMap::from_ron(&text, Path::new("again")).expect("reads"),
            map
        );
    }

    #[test]
    fn pad_bindings_and_a_dead_zone_read_from_a_file() {
        use crate::input::binding::{Direction, PadAxis, PadButton};
        let text = r#"(version: 1, actions: {
            "jump": (bindings: [Pad(South)]),
            "left": (bindings: [PadAxis(LeftStickX, Negative)], deadzone: 0.4),
        })"#;
        let map = ActionMap::from_ron(text, Path::new("x")).expect("parses");
        assert_eq!(map.bindings("jump"), &[Binding::Pad(PadButton::South)]);
        assert_eq!(
            map.bindings("left"),
            &[Binding::PadAxis(PadAxis::LeftStickX, Direction::Negative)]
        );
        assert_eq!(map.deadzone("left"), 0.4);
        assert_eq!(map.deadzone("jump"), DEFAULT_DEADZONE);
    }

    #[test]
    fn a_dead_zone_out_of_range_is_clamped() {
        let mut map = ActionMap::new();
        map.set_deadzone("a", 3.0);
        map.set_deadzone("b", f32::NAN);
        assert_eq!(map.deadzone("a"), MAX_DEADZONE);
        assert_eq!(map.deadzone("b"), DEFAULT_DEADZONE);
    }

    #[test]
    fn another_version_is_refused() {
        let text = FILE.replace("version: 1", "version: 9");
        assert!(matches!(
            ActionMap::from_ron(&text, Path::new("x")),
            Err(MapError::UnsupportedVersion { found: 9, .. })
        ));
    }

    #[test]
    fn an_empty_action_name_is_refused() {
        let text = r#"(version: 1, actions: { " ": (bindings: []) })"#;
        assert!(matches!(
            ActionMap::from_ron(text, Path::new("x")),
            Err(MapError::EmptyActionName { .. })
        ));
    }

    #[test]
    fn rebinding_takes_the_control_off_every_other_action() {
        let mut map = parsed();
        map.rebind("fire", 0, Binding::Key(KeyCode::Space));

        assert_eq!(map.bindings("fire"), &[Binding::Key(KeyCode::Space)]);
        assert_eq!(map.bindings("jump"), &[Binding::Key(KeyCode::ArrowUp)]);
    }

    #[test]
    fn rebinding_past_the_last_slot_appends() {
        let mut map = parsed();
        map.rebind("jump", 5, Binding::Key(KeyCode::KeyW));
        assert_eq!(map.bindings("jump").len(), 3);
    }

    #[test]
    fn bind_does_not_duplicate_and_unbind_reports() {
        let mut map = ActionMap::new();
        map.bind("jump", Binding::Key(KeyCode::Space));
        map.bind("jump", Binding::Key(KeyCode::Space));
        assert_eq!(map.bindings("jump").len(), 1);
        assert!(map.unbind("jump", Binding::Key(KeyCode::Space)));
        assert!(!map.unbind("jump", Binding::Key(KeyCode::Space)));
    }

    #[test]
    fn save_writes_a_file_load_reads_back() {
        let dir = voltra_testkit::scratch_root();
        let path = dir.join("settings").join("player.input.ron");
        let map = parsed();
        map.save(&path).expect("saves");
        assert_eq!(ActionMap::load(&path).expect("loads"), map);
        assert!(
            !path.with_extension("ron.tmp").exists(),
            "no temporary left"
        );
    }
}
