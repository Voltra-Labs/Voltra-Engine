//! Which input map the game's actions come from, and keeping it current.

use std::path::{Path, PathBuf};

use voltra_assets::AssetPath;

use super::App;
use crate::input::ActionMap;

/// The map a game gets without naming one, relative to the asset root.
///
/// A convention rather than a settings file, which the engine does not have:
/// a game that ships `assets/default.input.ron` is bound with no code at
/// all. `*.input.ron` keeps it out of the scene list, as `*.atlas.ron` does.
pub const DEFAULT_INPUT_MAP: &str = "default.input.ron";

impl App {
    /// Reads the game's actions from `path` instead of
    /// [`DEFAULT_INPUT_MAP`]. Read when the loop starts, and hot reloaded
    /// with everything else.
    pub fn with_input_map(mut self, path: AssetPath) -> Self {
        self.input_map_path = Some(path);
        self
    }

    /// Uses `map` as the game's actions — one built in code, or a player's
    /// saved bindings — in place of whatever a file would have loaded. A
    /// file named by [`App::with_input_map`] still replaces it if it is
    /// edited while running.
    pub fn with_actions(mut self, map: ActionMap) -> Self {
        self.default_actions = map.clone();
        self.actions = map;
        self.actions_from_code = true;
        self
    }

    /// The game's actions, as the frame reads them.
    pub fn actions(&self) -> &ActionMap {
        &self.actions
    }

    /// The game's actions as it ships them, before the player's saved
    /// controls are laid over.
    pub fn default_actions(&self) -> &ActionMap {
        &self.default_actions
    }

    /// Sets the actions to the game's own, then lays the player's saved
    /// controls over them ([`ActionMap::load_player`]): what every start
    /// and every reload of the map ends with, so a rebound key survives
    /// both. No folder (a test's `App::default`) is no saved controls; a
    /// file that will not read is a warning, and the game's own bindings
    /// stand.
    fn use_actions(&mut self, map: ActionMap) {
        self.actions = map.clone();
        self.default_actions = map;
        if self.saves.dir().is_none() {
            return;
        }
        match ActionMap::load_player(&self.saves) {
            Ok(Some(overrides)) => {
                let changed = self.actions.apply_overrides(&overrides);
                log::info!("the player's controls: {changed} actions rebound");
            }
            Ok(None) => {}
            Err(e) => log::warn!("{e}; using the game's own bindings"),
        }
    }

    /// The path the map is read from: the one named, or the convention.
    fn input_map(&self) -> AssetPath {
        self.input_map_path.clone().unwrap_or_else(|| {
            AssetPath::new(DEFAULT_INPUT_MAP).expect("the default input map path is valid")
        })
    }

    fn asset_root(&self) -> PathBuf {
        self.asset_root
            .clone()
            .unwrap_or_else(voltra_assets::default_root)
    }

    /// Reads the map at startup.
    ///
    /// No file at the conventional path is normal — a game may bind in code
    /// or use raw keys — and costs one debug line. A file that is named but
    /// missing, or one that will not parse, is a warning, and the game runs
    /// with no actions rather than not at all.
    pub(super) fn load_input_map(&mut self, root: &Path) {
        if self.actions_from_code && self.input_map_path.is_none() {
            let map = self.default_actions.clone();
            self.use_actions(map);
            return;
        }
        let path = self.input_map();
        let full = root.join(path.as_str());
        if self.input_map_path.is_none() && !voltra_assets::fs::exists(&full) {
            log::debug!(
                "no {} under the asset root; no input actions",
                path.as_str()
            );
            self.use_actions(ActionMap::new());
            return;
        }
        match ActionMap::load(&full) {
            Ok(map) => {
                log::info!(
                    "input map {} ({} actions)",
                    path.as_str(),
                    map.actions.len()
                );
                self.use_actions(map);
            }
            Err(e) => log::warn!("{e}; running with no input actions"),
        }
    }

    /// Re-reads the map if `path` is it. Returns whether `path` was the map,
    /// whether or not it parsed — a broken save keeps the bindings already
    /// loaded.
    ///
    /// A reload replaces bindings rebound at runtime and not yet saved: the
    /// file is what the developer is editing, and it wins — over everything
    /// but the player's saved controls, which are laid over it again.
    pub(super) fn reload_input_map(&mut self, path: &AssetPath) -> bool {
        if *path != self.input_map() {
            return false;
        }
        match ActionMap::load(&self.asset_root().join(path.as_str())) {
            Ok(map) => {
                self.use_actions(map);
                log::info!("reloaded {}", path.as_str());
            }
            Err(e) => log::warn!("{e}; keeping the previous input map"),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use winit::keyboard::KeyCode;

    use super::*;
    use crate::input::Binding;

    const MAP: &str = r#"(version: 1, actions: { "jump": (bindings: [Key(Space)]) })"#;

    fn root_with(name: &str, text: &str) -> voltra_testkit::Scratch {
        let root = voltra_testkit::scratch_root();
        std::fs::write(root.join(name), text).expect("the fixture writes");
        root
    }

    #[test]
    fn the_conventional_map_is_loaded_with_no_code() {
        let root = root_with(DEFAULT_INPUT_MAP, MAP);
        let mut app = App::default();
        app.load_input_map(&root);
        assert_eq!(
            app.actions().bindings("jump"),
            &[Binding::Key(KeyCode::Space)]
        );
    }

    #[test]
    fn no_map_at_all_is_no_actions_and_no_error() {
        let mut app = App::default();
        let scratch = voltra_testkit::scratch_root();
        app.load_input_map(&scratch);
        assert!(app.actions().actions.is_empty());
    }

    #[test]
    fn a_named_map_is_read_instead() {
        let root = root_with("controls.input.ron", MAP);
        let mut app =
            App::default().with_input_map(AssetPath::new("controls.input.ron").expect("valid"));
        app.load_input_map(&root);
        assert!(app.actions().contains("jump"));
    }

    #[test]
    fn a_map_from_code_is_not_replaced_by_the_convention() {
        let root = root_with(DEFAULT_INPUT_MAP, MAP);
        let mut code = ActionMap::new();
        code.bind("fire", Binding::Key(KeyCode::KeyF));
        let mut app = App::default().with_actions(code.clone());
        app.load_input_map(&root);
        assert_eq!(app.actions(), &code);
    }

    #[test]
    fn a_reload_of_the_map_replaces_it_and_a_broken_one_keeps_it() {
        let root = root_with(DEFAULT_INPUT_MAP, MAP);
        let mut app = App::default().with_asset_root(&root);
        app.load_input_map(&root);
        let path = AssetPath::new(DEFAULT_INPUT_MAP).expect("valid");

        std::fs::write(
            root.join(DEFAULT_INPUT_MAP),
            r#"(version: 1, actions: { "jump": (bindings: [Key(KeyW)]) })"#,
        )
        .expect("writes");
        assert!(app.reload_input_map(&path));
        assert_eq!(
            app.actions().bindings("jump"),
            &[Binding::Key(KeyCode::KeyW)]
        );

        std::fs::write(root.join(DEFAULT_INPUT_MAP), "(half").expect("writes");
        assert!(app.reload_input_map(&path));
        assert_eq!(
            app.actions().bindings("jump"),
            &[Binding::Key(KeyCode::KeyW)]
        );
    }

    #[test]
    fn the_shipped_default_map_parses() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(DEFAULT_INPUT_MAP);
        let map = ActionMap::load(&path).expect("the shipped map parses");
        for action in [
            "jump",
            "move_left",
            "move_right",
            "ui_up",
            "ui_down",
            "ui_left",
            "ui_right",
            "ui_accept",
            "ui_cancel",
            "pause",
        ] {
            assert!(!map.bindings(action).is_empty(), "{action} is bound");
        }
    }

    #[test]
    fn another_file_is_not_the_map() {
        let mut app = App::default();
        assert!(!app.reload_input_map(&AssetPath::new("level.ron").expect("valid")));
    }
}
