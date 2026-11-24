//! The project's game, loaded from its library and run by Play.
//!
//! The editor links no game code; it loads the game crate's `cdylib`
//! ([`voltra_core::game::library`]) — the one `build.ron` names, or the one
//! `--game` points at — registers its components, and reads the scene back
//! in so the components the scene held as unknowns become the game's own
//! types. Play makes a fresh game from the library and the loop runs its
//! turns; Stop drops it with the rest of the play session.
//!
//! A game named by a manifest is also watched ([`reload`]): a change to
//! its sources rebuilds it, and the new library replaces the old one —
//! mid-play too — with the world carried across through the registries.
//!
//! Unloading is the same in reverse, with one rule on top: nothing the
//! library made may outlive it. The scene is written out through the
//! game's registry and read back through the engine's, which leaves the
//! world with no storage the game's code created, the registries are reset,
//! and only then is the library closed.

use std::path::PathBuf;

use voltra_core::game::library::GameLibrary;
use voltra_core::{Game, UiFrame};
use voltra_scene::ComponentRegistry;

use crate::editor::Editor;
use crate::play::PlayState;
use crate::selection::Selection;

mod project;
pub(crate) mod reload;
mod source;

use voltra_core::game::swap::rebuild;

/// The asset root the build manifest at `manifest` names.
pub(crate) fn project_assets(manifest: &std::path::Path) -> Result<PathBuf, String> {
    project::assets_of(manifest)
}

/// The loaded game, or why there is none, and how it is rebuilt.
#[derive(Default)]
pub(crate) struct GameHost {
    library: Option<GameLibrary>,
    /// Asked for with `--game`; otherwise the project's is looked up.
    asked: Option<PathBuf>,
    /// Why the last load did not happen, for the menu to say.
    problem: Option<String>,
    /// Whether the first frame's load has been tried.
    tried: bool,
    /// Watching the game's sources and rebuilding them, when the game was
    /// named by a manifest.
    pub(crate) reload: reload::Reload,
}

impl GameHost {
    /// A host that loads the library at `path` instead of the project's, or
    /// the one the build manifest at `path` names.
    pub(crate) fn at(path: PathBuf) -> Self {
        Self {
            asked: Some(path),
            ..Self::default()
        }
    }

    /// The loaded library's path.
    pub(crate) fn loaded(&self) -> Option<&std::path::Path> {
        self.library.as_ref().map(GameLibrary::path)
    }

    /// Why no game is loaded, when one was looked for and not found.
    pub(crate) fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// A fresh game for a play session, if one is loaded.
    pub(crate) fn new_game(&self) -> Option<Box<dyn Game>> {
        self.library.as_ref().map(GameLibrary::new_game)
    }

    /// The game to load: the one asked for — a library named directly, or
    /// the game a build manifest names — or the project's.
    fn wanted(&self) -> Result<Option<Wanted>, String> {
        let named = match &self.asked {
            Some(path) if path.extension().is_some_and(|ext| ext == "ron") => {
                project::named_by(path)?
                    .ok_or_else(|| format!("{} names no game crate", path.display()))?
            }
            Some(path) => {
                return Ok(Some(Wanted {
                    library: path.clone(),
                    named: None,
                }))
            }
            None => match project::named(&voltra_assets::default_root())? {
                Some(named) => named,
                None => return Ok(None),
            },
        };
        Ok(Some(Wanted {
            library: named.library.clone(),
            named: Some(named),
        }))
    }
}

/// A library to load, and the crate it comes from when that is known.
struct Wanted {
    library: PathBuf,
    named: Option<project::NamedGame>,
}

/// Loads the game once, on the first frame, before a scene the command
/// line asked for is read: its components must be named by then.
pub(crate) fn load_at_start(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    if std::mem::replace(&mut editor.game.tried, true) {
        return;
    }
    load(editor, frame);
}

/// Loads the project's game, replacing one already loaded — mid-play too —
/// and watches its sources when it knows them.
pub(crate) fn load(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    let wanted = match editor.game.wanted() {
        Ok(Some(wanted)) => wanted,
        Ok(None) => {
            editor.game.problem = None;
            return;
        }
        Err(problem) => {
            log::warn!("no game loaded: {problem}");
            editor.game.problem = Some(problem);
            return;
        }
    };
    if let Some(named) = &wanted.named {
        editor.game.reload.watch(named);
    }
    open_and_install(editor, frame, &wanted.library);
}

/// Opens the library at `path` and puts it in place of the loaded one, or
/// says why not and keeps the one there is.
pub(crate) fn open_and_install(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    path: &std::path::Path,
) -> bool {
    let library = match GameLibrary::open(path) {
        Ok(library) => library,
        Err(error) => {
            log::warn!("no game loaded: {error}");
            editor.game.problem = Some(error.to_string());
            return false;
        }
    };
    install(editor, frame, library)
}

/// Puts `library` in place of the loaded game's, the world read across
/// from one's components to the other's, and closes the old one. A play
/// session goes on with a fresh game from the new library. `false`, with
/// the old game still in place and running, when the scene does not read
/// with the new one's components.
fn install(editor: &mut Editor, frame: &mut UiFrame<'_>, library: GameLibrary) -> bool {
    let mut registry = ComponentRegistry::with_defaults();
    library.register(&mut registry);
    let playing = editor.play.state() != PlayState::Editing;
    // The old game's turns end before its types are written out.
    if playing {
        frame.set_game(None);
    }
    if !swap_registry(editor, frame, registry) {
        let problem = format!(
            "{} was not loaded: the scene does not read with its components (see the log)",
            library.path().display()
        );
        editor.game.problem = Some(problem);
        if playing {
            frame.set_game(editor.game.new_game());
        }
        // SAFETY: the swap was undone; nothing of the new library's is left.
        unsafe { library.close() };
        return false;
    }
    *frame.registry = ComponentRegistry::with_defaults();
    library.register(frame.registry);
    if playing {
        frame.set_game(Some(library.new_game()));
    }
    editor.game.problem = None;
    if let Some(old) = editor.game.library.replace(library) {
        // SAFETY: the world and both registries were rebuilt through the new
        // library; the old game was dropped by `set_game` above.
        unsafe { old.close() };
    }
    true
}

/// Unloads the game, if one is loaded, leaving its components in the scene
/// as unknown components the next load reads back.
pub(crate) fn unload(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    if editor.game.library.is_none() {
        return;
    }
    editor.stop_play(frame);
    frame.set_game(None);
    editor.game.reload.stop();
    swap_registry(editor, frame, ComponentRegistry::with_defaults());
    *frame.registry = ComponentRegistry::with_defaults();
    if let Some(library) = editor.game.library.take() {
        // SAFETY: the world was rebuilt through the engine's registry, so no
        // storage the library created is left in it; both registries are the
        // engine's again; the play session that ran its game is over, and
        // the game was dropped with it.
        unsafe { library.close() };
    }
}

/// Reads the scene back in through `registry` and makes it the editor's,
/// keeping the selection. `false`, with the scene as it was, when the scene
/// does not read through it.
fn swap_registry(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    registry: ComponentRegistry,
) -> bool {
    let selected = editor.selection.ids(frame.world);
    // Both hold entities the rebuild is about to replace.
    editor.gizmo.cancel_drag();
    editor.shape.cancel_drag();
    let result = rebuild(frame.world, &editor.registry, &registry);
    frame.reset_physics();
    frame.resolve_scene_assets();
    editor.selection = Selection::from_ids(frame.world, &selected);
    match result {
        Ok(()) => {
            editor.registry = registry;
            true
        }
        Err(error) => {
            log::error!("could not read the scene through the game's components: {error}");
            false
        }
    }
}
