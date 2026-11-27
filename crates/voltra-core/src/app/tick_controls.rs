//! What a game's tick may do with the player's controls beyond rebinding
//! them: keep them for the next run, and go back to the game's own.

use super::Tick;
use crate::input::{ActionMap, MapError};

impl Tick<'_> {
    /// Keeps the controls as the player has rebound them, for every run
    /// after this one: what differs from the game's own map, in the
    /// player's folder — a file natively, `localStorage` in a browser. The
    /// loop lays them over the map when the game next starts.
    pub fn save_controls(&mut self) -> Result<(), MapError> {
        self.actions.save_player(self.default_actions, self.saves)
    }

    /// Puts every action back to the game's own bindings, from the next
    /// frame on. Not kept until [`save_controls`](Self::save_controls).
    pub fn reset_controls(&mut self) {
        self.actions.clone_from(self.default_actions);
    }

    /// The game's own bindings, before the player rebound anything: what an
    /// options screen shows as the default.
    pub fn default_controls(&self) -> &ActionMap {
        self.default_actions
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use crate::input::{ActionMap, Binding};
    use crate::{App, Headless, KeyCode};

    const FRAME: f32 = 1.0 / 60.0;

    fn map() -> ActionMap {
        let mut map = ActionMap::new();
        map.bind("jump", Binding::Key(KeyCode::Space));
        map.bind("fire", Binding::Key(KeyCode::KeyF));
        map
    }

    /// A game that rebinds jump to the first key pressed after F1, saves
    /// when asked, and says whether jump is held.
    fn game(dir: &std::path::Path, held: Rc<Cell<bool>>) -> Headless {
        let mut rebinding = false;
        let app = App::default()
            .with_actions(map())
            .with_save_dir(dir)
            .with_update(move |tick| {
                if rebinding {
                    if let Some(binding) = tick.input.pressed_binding() {
                        tick.actions.rebind("jump", 0, binding);
                        if let Err(e) = tick.save_controls() {
                            panic!("the controls did not save: {e}");
                        }
                        rebinding = false;
                    }
                } else if tick.input.was_key_pressed(KeyCode::F1) {
                    rebinding = true;
                }
                held.set(tick.input.action("jump").is_held());
            });
        Headless::new(app)
    }

    fn tap(session: &mut Headless, key: KeyCode) {
        session.input_mut().on_key(key, true, false);
        session.step(1, FRAME);
        session.input_mut().on_key(key, false, false);
        session.step(1, FRAME);
    }

    #[test]
    fn a_rebound_key_drives_its_action_in_the_next_run() {
        let scratch = voltra_testkit::scratch_root();
        let dir = scratch.join("user");
        let held = Rc::new(Cell::new(false));
        let mut first = game(&dir, held.clone());
        tap(&mut first, KeyCode::F1);
        tap(&mut first, KeyCode::KeyJ);
        assert_eq!(
            first.into_app().actions().bindings("jump"),
            &[Binding::Key(KeyCode::KeyJ)]
        );

        // A new session: the game's own map, and the player's over it.
        let mut second = game(&dir, held.clone());
        second.input_mut().on_key(KeyCode::KeyJ, true, false);
        second.step(1, FRAME);
        assert!(held.get(), "J is jump in the next run");
        second.input_mut().on_key(KeyCode::KeyJ, false, false);
        second.input_mut().on_key(KeyCode::Space, true, false);
        second.step(1, FRAME);
        assert!(!held.get(), "and Space no longer is");
        let app = second.into_app();
        assert_eq!(app.default_actions(), &map(), "the defaults are the game's");
        assert_eq!(
            app.actions().bindings("fire"),
            &[Binding::Key(KeyCode::KeyF)]
        );
    }

    #[test]
    fn a_reset_saved_leaves_no_controls_behind() {
        let scratch = voltra_testkit::scratch_root();
        let dir = scratch.join("user");
        let app = App::default()
            .with_actions(map())
            .with_save_dir(&dir)
            .with_update(|tick| {
                tick.actions.rebind("jump", 0, Binding::Key(KeyCode::KeyJ));
                tick.reset_controls();
                assert_eq!(tick.actions, tick.default_controls());
                if let Err(e) = tick.save_controls() {
                    panic!("the controls did not save: {e}");
                }
            });
        let mut session = Headless::new(app);
        session.step(1, FRAME);
        let saves = crate::Saves::in_dir(&dir);
        assert_eq!(ActionMap::load_player(&saves).expect("reads"), None);
    }
}
