//! How an `App` is put together: the `with_*` builders a game or the
//! editor calls before `run`.

use std::path::PathBuf;

use voltra_scene::ComponentRegistry;

use super::{App, Tick, UiFrame};
use crate::egui;

impl App {
    /// Installs the UI callback, run once per frame to lay the editor out.
    ///
    /// Without one the scene is drawn straight to the window and no UI exists,
    /// which is what a shipped game wants.
    pub fn with_ui(mut self, ui: impl FnMut(&mut egui::Ui, &mut UiFrame<'_>) + 'static) -> Self {
        self.ui = Some(Box::new(ui));
        self
    }

    /// Sets the directory every texture path resolves against.
    ///
    /// Without this, [`voltra_assets::default_root`] resolves one at startup.
    pub fn with_asset_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.asset_root = Some(root.into());
        self
    }

    /// Names the game, which names its per-user saves folder —
    /// `%APPDATA%/<name>/saves` on Windows; see
    /// [`saves::dir`](crate::saves::dir). Without this, the window title is
    /// the name.
    pub fn with_game_name(mut self, name: &str) -> Self {
        self.saves = crate::saves::Saves::for_game(name);
        self
    }

    /// Sorts drawables by `names`, back to front, instead of Background,
    /// Default, Foreground. `Default` is kept — every drawable naming no
    /// layer is on it.
    pub fn with_sorting_layers<S: Into<String>>(
        mut self,
        names: impl IntoIterator<Item = S>,
    ) -> Self {
        self.sorting_layers = voltra_scene::SortingLayers::new(names);
        self.sorting_layers_from_code = true;
        self
    }

    /// Keeps the save slots in `dir` instead of the per-user folder: a
    /// portable build, or a test.
    /// How every image is filtered when it does not land 1:1 on pixels:
    /// `Filter::Nearest` keeps pixel art crisp. Linear when not given.
    pub fn with_texture_filter(mut self, filter: voltra_render::Filter) -> Self {
        self.texture_filter = filter;
        self
    }

    /// The language the game starts in: the table `locale/<language>.ron`
    /// it reads keyed texts from. English (`en`) when not given.
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = language.to_owned();
        self
    }

    /// Keeps saves in `dir` instead of the per-game default.
    pub fn with_save_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.saves = crate::saves::Saves::in_dir(dir);
        self
    }

    /// The game's save slots, for setup code that reads one before the first
    /// tick.
    pub fn saves(&self) -> &crate::saves::Saves {
        &self.saves
    }

    /// Watches the asset root and reloads textures as their files change.
    ///
    /// The editor wants this; a shipped game does not, which is why it is not
    /// the default. A failure to start the watch is logged and otherwise
    /// ignored — the app runs, it just does not notice file changes.
    pub fn with_hot_reload(mut self) -> Self {
        self.hot_reload = true;
        self
    }

    /// Asks the UI before closing: a close the person asks for waits until
    /// the UI answers through [`UiFrame::close`] or
    /// [`UiFrame::cancel_close`] — the editor's "save changes?" prompt.
    /// Without a UI a close is still immediate, so nothing traps the window.
    pub fn with_close_guard(mut self) -> Self {
        self.window_requests.guard();
        self
    }

    /// Reads prefabs through `registry` instead of the default one.
    ///
    /// What a game with components of its own needs: a prefab carrying a
    /// `Coin` marker spawns it only if the registry knows the name, and keeps
    /// it unread otherwise. Start from [`ComponentRegistry::with_defaults`]
    /// and register on top, or the engine's own components stop loading.
    pub fn with_registry(mut self, registry: ComponentRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Runs `game`: registers its components on top of the registry so far,
    /// and gives it both turns, after any hook installed with
    /// [`with_update`](Self::with_update) or
    /// [`with_fixed_update`](Self::with_fixed_update).
    ///
    /// Call it after [`with_registry`](Self::with_registry), which would
    /// otherwise replace what the game registered.
    pub fn with_game(mut self, game: impl crate::Game + 'static) -> Self {
        game.register(&mut self.registry);
        self.game = Some(crate::game::Running::new(Box::new(game)));
        self
    }

    /// Mixes through `buses` instead of the default master, music, sfx and
    /// ui.
    ///
    /// A builder rather than a settings file, because there is no settings
    /// file: the table is code the game owns. Names a scene uses that are not
    /// in it still work — see [`Buses::id`](voltra_audio::Buses::id).
    pub fn with_buses(mut self, buses: voltra_audio::Buses) -> Self {
        self.audio = std::mem::take(&mut self.audio).with_buses(buses);
        self
    }

    /// Starts with the world live: physics steps, and the game's ticks run.
    ///
    /// Off by default: a scene with no `RigidBody2D` pays nothing either way, but
    /// an editor that started simulating the moment a body was added would move
    /// the thing being placed out from under the cursor, and one that ran game
    /// logic while authoring would edit the scene behind the author's back —
    /// which is why Unity runs scripts in play mode only.
    ///
    /// This is the *initial* value of a runtime switch, not a build-time
    /// choice. A game says it once and never thinks about it again; the editor
    /// says nothing and drives [`App::set_simulating`] from its play mode.
    ///
    /// Contacts are resolved: bodies rest on each other and stacks settle. Set
    /// [`App::gravity`] to change or disable the acceleration, and reach the
    /// solver's tuning through [`App::physics_mut`].
    pub fn with_simulation(mut self) -> Self {
        self.simulation.set_running(true);
        self
    }

    /// Installs the game's per-frame tick, run once before the frame's steps.
    ///
    /// This is where input belongs: it is read once per frame, so an edge — a
    /// key that went down *this* frame — is only counted once here. Runs only
    /// while the world is live, which for a game is always and for the editor
    /// is play mode.
    pub fn with_update(mut self, update: impl FnMut(&mut Tick<'_>) + 'static) -> Self {
        self.update = Some(Box::new(update));
        self
    }

    /// Installs the game's fixed tick, run before each physics step.
    ///
    /// Zero, one or several times per frame, with `delta` always the fixed
    /// step. This is where a force or a velocity belongs: applied per frame it
    /// would be scaled by the frame rate, which is the bug Unity's
    /// `FixedUpdate` and Godot's `_physics_process` exist to prevent.
    pub fn with_fixed_update(mut self, fixed: impl FnMut(&mut Tick<'_>) + 'static) -> Self {
        self.fixed_update = Some(Box::new(fixed));
        self
    }
}
