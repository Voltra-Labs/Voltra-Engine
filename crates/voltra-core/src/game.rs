//! A game's own code, as one value the loop runs.
//!
//! A game registers its components and plays its turns through [`Game`]. Its
//! own binary hands one to [`App::with_game`](crate::App::with_game) and links
//! it like any other code; the editor loads the same crate as a dynamic
//! library ([`library`]) and runs a fresh one each time Play is pressed. One
//! entry point for both, as Fyrox's plugins have: what a game does in the
//! editor is what it does shipped.
//!
//! [`export_game!`](crate::export_game) is all a game crate adds to be
//! loadable; [`stamp`] is how a library built by another compiler, or with
//! other flags, is refused rather than trusted.

use voltra_scene::ComponentRegistry;

use crate::app::Tick;

#[cfg(not(target_arch = "wasm32"))]
pub mod builder;
#[cfg(not(target_arch = "wasm32"))]
pub mod library;
pub mod plugin;
pub mod stamp;
pub mod swap;
#[cfg(not(target_arch = "wasm32"))]
pub mod watch;

/// A game: its components, and what it does each turn while the world is
/// live.
///
/// Every method has an empty default, so a game writes only the turns it
/// has. The state a game keeps lives in the value — the editor makes a new
/// one for every play session, so nothing carries over from the last run,
/// and a shipped build makes one for its whole life.
///
/// A game finds what it acts on by asking the world — the entity carrying
/// its `Walker`, every `Coin` — rather than holding entities made in code:
/// in the editor the scene is whatever the author made, and Play runs the
/// game on it.
pub trait Game {
    /// Names the game's own components, so scene files and prefabs carry
    /// them and the inspector shows them. Called once, before any scene that
    /// may hold them is read.
    fn register(&self, registry: &mut ComponentRegistry) {
        let _ = registry;
    }

    /// The game's first turn, before its first [`update`](Self::update) or
    /// [`fixed_update`](Self::fixed_update): where it looks the scene over.
    fn start(&mut self, tick: &mut Tick<'_>) {
        let _ = tick;
    }

    /// Once a frame, before the frame's physics steps: where input edges
    /// belong (see [`App::with_update`](crate::App::with_update)).
    fn update(&mut self, tick: &mut Tick<'_>) {
        let _ = tick;
    }

    /// Before each physics step, with the fixed step as `delta`: where
    /// forces and velocities belong (see
    /// [`App::with_fixed_update`](crate::App::with_fixed_update)).
    fn fixed_update(&mut self, tick: &mut Tick<'_>) {
        let _ = tick;
    }
}

/// A boxed game is a game: what a game loaded from a library is handed
/// around as.
impl<G: Game + ?Sized> Game for Box<G> {
    fn register(&self, registry: &mut ComponentRegistry) {
        (**self).register(registry);
    }

    fn start(&mut self, tick: &mut Tick<'_>) {
        (**self).start(tick);
    }

    fn update(&mut self, tick: &mut Tick<'_>) {
        (**self).update(tick);
    }

    fn fixed_update(&mut self, tick: &mut Tick<'_>) {
        (**self).fixed_update(tick);
    }
}

/// Which of its two turns a game is taking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Turn {
    Frame,
    Fixed,
}

/// A game the loop is running, and whether it has had its first turn.
pub(crate) struct Running {
    game: Box<dyn Game>,
    started: bool,
}

impl Running {
    pub(crate) fn new(game: Box<dyn Game>) -> Self {
        Self {
            game,
            started: false,
        }
    }

    pub(crate) fn take_turn(&mut self, turn: Turn, tick: &mut Tick<'_>) {
        if !self.started {
            self.started = true;
            self.game.start(tick);
        }
        match turn {
            Turn::Frame => self.game.update(tick),
            Turn::Fixed => self.game.fixed_update(tick),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::App;

    const FRAME: f32 = 1.0 / 60.0;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Marker;

    /// A game writing down its turns.
    struct Diary(Rc<RefCell<Vec<&'static str>>>);

    impl Game for Diary {
        fn register(&self, registry: &mut ComponentRegistry) {
            registry.register::<Marker>("Marker");
        }
        fn start(&mut self, _: &mut Tick<'_>) {
            self.0.borrow_mut().push("start");
        }
        fn update(&mut self, _: &mut Tick<'_>) {
            self.0.borrow_mut().push("update");
        }
        fn fixed_update(&mut self, _: &mut Tick<'_>) {
            self.0.borrow_mut().push("fixed");
        }
    }

    #[test]
    fn a_game_registers_and_starts_before_its_first_turn() {
        let turns = Rc::new(RefCell::new(Vec::new()));
        let mut app = App::default()
            .with_simulation()
            .with_game(Diary(turns.clone()));

        assert!(app.registry.names().any(|name| name == "Marker"));
        app.advance(FRAME);
        app.advance(FRAME);

        assert_eq!(
            *turns.borrow(),
            ["start", "update", "fixed", "update", "fixed"]
        );
    }

    #[test]
    fn an_installed_hook_runs_before_the_game_in_the_same_turn() {
        let turns = Rc::new(RefCell::new(Vec::new()));
        let hook = turns.clone();
        let mut app = App::default()
            .with_simulation()
            .with_update(move |_| hook.borrow_mut().push("hook"))
            .with_game(Diary(turns.clone()));

        app.advance(FRAME);

        assert_eq!(*turns.borrow(), ["hook", "start", "update", "fixed"]);
    }

    #[test]
    fn a_game_while_editing_only_steps_when_asked() {
        let turns = Rc::new(RefCell::new(Vec::new()));
        let mut app = App::default().with_game(Diary(turns.clone()));

        app.advance(FRAME);
        assert!(turns.borrow().is_empty(), "not simulating: no turns");

        app.request_steps(1);
        app.advance(FRAME);
        assert_eq!(
            *turns.borrow(),
            ["start", "fixed"],
            "a Step steps the game too"
        );
    }
}
