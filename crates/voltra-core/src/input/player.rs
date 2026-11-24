//! The controls a player rebound, kept in their own folder: only what
//! differs from the game's map, laid over it when the game starts.
//!
//! Unity's Input System saves binding *overrides*
//! (`SaveBindingOverridesAsJson`), not the whole asset, and this does the
//! same for the same reason: a game that ships a new action, or changes a
//! default the player never touched, must reach that player. A whole map
//! saved would freeze every default at the version the player first
//! rebound in. The file is an input map all the same (version, actions),
//! so a person can read it.

use super::map::{Action, ActionMap, MapError};
use crate::saves::Saves;

/// The player's file beside the save slots: on the web, the key
/// `/<game>/saves/controls.input.ron` in the page's `localStorage`.
pub const PLAYER_CONTROLS: &str = "controls.input.ron";

impl ActionMap {
    /// The actions of this map that differ from `defaults` — bindings or
    /// dead zone — each whole. An action the defaults have and this map
    /// lacks is there with no bindings: the player unbound it.
    pub fn overrides(&self, defaults: &ActionMap) -> ActionMap {
        let mut overrides = ActionMap::new();
        for (name, action) in &self.actions {
            if defaults.actions.get(name) != Some(action) {
                overrides.actions.insert(name.clone(), action.clone());
            }
        }
        for name in defaults.actions.keys() {
            if !self.actions.contains_key(name) {
                overrides.actions.insert(name.clone(), Action::default());
            }
        }
        overrides
    }

    /// Lays `overrides` over this map: each action both name takes the
    /// overrides' bindings and dead zone. An action this map does not have
    /// is left out — the game no longer has it — and logged. Answers how
    /// many actions changed.
    pub fn apply_overrides(&mut self, overrides: &ActionMap) -> usize {
        let mut applied = 0;
        for (name, action) in &overrides.actions {
            match self.actions.get_mut(name) {
                Some(mine) => {
                    if mine != action {
                        *mine = action.clone();
                        applied += 1;
                    }
                }
                None => log::info!("the player's controls name {name:?}, which this game has not"),
            }
        }
        applied
    }

    /// Keeps what this map changed from `defaults` as the player's controls,
    /// in `saves`' folder — a file natively, `localStorage` on the web. With
    /// nothing changed the file is removed, so a reset is a reset.
    pub fn save_player(&self, defaults: &ActionMap, saves: &Saves) -> Result<(), MapError> {
        let overrides = self.overrides(defaults);
        if overrides.actions.is_empty() {
            saves
                .remove_file(PLAYER_CONTROLS)
                .map_err(MapError::Storage)?;
            return Ok(());
        }
        saves
            .write_text(PLAYER_CONTROLS, &overrides.to_ron()?)
            .map_err(MapError::Storage)
    }

    /// The player's saved controls, as overrides for
    /// [`apply_overrides`](Self::apply_overrides), or `None` when they
    /// never rebound anything. A file that is not an input map of this
    /// version is an error, said rather than silently dropped.
    pub fn load_player(saves: &Saves) -> Result<Option<ActionMap>, MapError> {
        let Some(text) = saves
            .read_text(PLAYER_CONTROLS)
            .map_err(MapError::Storage)?
        else {
            return Ok(None);
        };
        let path = saves
            .path_of_file(PLAYER_CONTROLS)
            .map_err(MapError::Storage)?;
        Self::from_ron(&text, &path).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use winit::keyboard::KeyCode;

    use super::*;
    use crate::input::Binding;

    fn defaults() -> ActionMap {
        let mut map = ActionMap::new();
        map.bind("jump", Binding::Key(KeyCode::Space));
        map.bind("fire", Binding::Key(KeyCode::KeyF));
        map
    }

    #[test]
    fn only_what_changed_is_an_override() {
        let mut mine = defaults();
        mine.rebind("jump", 0, Binding::Key(KeyCode::KeyJ));
        let overrides = mine.overrides(&defaults());
        assert_eq!(overrides.names().collect::<Vec<_>>(), vec!["jump"]);
        assert!(defaults().overrides(&defaults()).actions.is_empty());
    }

    #[test]
    fn overrides_reach_only_the_actions_the_game_still_has() {
        let mut overrides = ActionMap::new();
        overrides.bind("jump", Binding::Key(KeyCode::KeyJ));
        overrides.bind("gone", Binding::Key(KeyCode::KeyG));
        let mut map = defaults();
        assert_eq!(map.apply_overrides(&overrides), 1);
        assert_eq!(map.bindings("jump"), &[Binding::Key(KeyCode::KeyJ)]);
        assert_eq!(map.bindings("fire"), &[Binding::Key(KeyCode::KeyF)]);
        assert!(!map.contains("gone"));
    }

    #[test]
    fn a_player_s_controls_round_trip_through_a_user_folder() {
        let saves = Saves::in_dir(voltra_testkit::scratch_root().join("user"));
        assert_eq!(ActionMap::load_player(&saves).expect("reads"), None);
        let mut mine = defaults();
        mine.rebind("jump", 0, Binding::Key(KeyCode::KeyJ));
        mine.save_player(&defaults(), &saves).expect("saves");

        let mut next_run = defaults();
        let saved = ActionMap::load_player(&saves)
            .expect("reads")
            .expect("there is one");
        next_run.apply_overrides(&saved);
        assert_eq!(next_run, mine);

        // Back to the defaults: nothing left to keep.
        defaults().save_player(&defaults(), &saves).expect("saves");
        assert_eq!(ActionMap::load_player(&saves).expect("reads"), None);
    }

    #[test]
    fn a_broken_file_is_an_error() {
        let saves = Saves::in_dir(voltra_testkit::scratch_root());
        saves
            .write_text(PLAYER_CONTROLS, "(version: 9)")
            .expect("writes");
        assert!(matches!(
            ActionMap::load_player(&saves),
            Err(MapError::UnsupportedVersion { found: 9, .. })
        ));
    }
}
