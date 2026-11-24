//! Rebuilding the game when its sources change, and swapping it in.
//!
//! A change to the crate's sources queues a build; the build runs off the
//! frame while the old code keeps running; a library that built replaces
//! the loaded one mid-play ([`super::open_and_install`]); one that did not
//! leaves the old code running and says why in the Game menu. A change
//! made during a build queues one more, so the last edit is always the one
//! built.

use std::path::PathBuf;

use voltra_core::game::builder::{summary, BuildOutcome, GameBuild};
use voltra_core::game::watch::SourceWatcher;
use voltra_core::UiFrame;

use super::project::NamedGame;
use super::source::{locate, GameSource};
use crate::editor::Editor;

/// How many compiler errors the Game menu shows.
const SHOWN_ERRORS: usize = 6;

/// The watch on a game's sources and the build it last started.
#[derive(Default)]
pub(crate) struct Reload {
    /// The game being watched, and how it builds.
    source: Option<(NamedGame, GameSource)>,
    watcher: Option<SourceWatcher>,
    build: Option<GameBuild>,
    /// A build is wanted: something changed since the last one started.
    queued: bool,
    /// Why the last build failed, its errors as cargo gave them.
    failure: Option<String>,
}

impl Reload {
    /// Watches `named`'s sources, unless they are watched already.
    pub(crate) fn watch(&mut self, named: &NamedGame) {
        if self.source.as_ref().is_some_and(|(n, _)| n == named) {
            return;
        }
        *self = Self::default();
        let source = match locate(&named.project, &named.package) {
            Ok(source) => source,
            Err(problem) => {
                log::warn!("the game will not be rebuilt on a change: {problem}");
                return;
            }
        };
        match SourceWatcher::new(&source.dir) {
            Ok(watcher) => self.watcher = Some(watcher),
            Err(problem) => log::warn!("the game will not be rebuilt on a change: {problem}"),
        }
        self.source = Some((named.clone(), source));
    }

    /// Stops watching, and stops a build under way.
    pub(crate) fn stop(&mut self) {
        *self = Self::default();
    }

    /// Whether there are sources to rebuild from.
    pub(crate) fn can_build(&self) -> bool {
        self.source.is_some()
    }

    pub(crate) fn building(&self) -> bool {
        self.build.is_some()
    }

    /// The last build's errors, while it is the last.
    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    /// Asks for a build now, as a change to the sources would.
    pub(crate) fn rebuild(&mut self) {
        self.queued = true;
    }

    /// Starts the build asked for, if there is no build running.
    fn start_queued(&mut self) {
        if self.build.is_some() || !self.queued {
            return;
        }
        let Some((named, source)) = &self.source else {
            return;
        };
        self.queued = false;
        log::info!(
            "rebuilding {}: cargo {}",
            named.package,
            source.build.join(" ")
        );
        match GameBuild::start(&source.workspace, &source.build) {
            Ok(build) => self.build = Some(build),
            Err(e) => self.failure = Some(format!("could not start cargo: {e}")),
        }
    }

    /// The library a finished build produced, once the build is over and
    /// it built; `None` while it runs, or when it failed — said in
    /// [`failure`](Self::failure).
    fn finished(&mut self) -> Option<PathBuf> {
        let outcome = self.build.as_mut()?.poll()?;
        self.build = None;
        match outcome {
            BuildOutcome::Built => {
                self.failure = None;
                self.source.as_ref().map(|(named, _)| named.library.clone())
            }
            BuildOutcome::Failed(output) => {
                let errors = summary(&output, SHOWN_ERRORS);
                log::error!("the game did not build; the old code keeps running:\n{errors}");
                self.failure = Some(errors);
                None
            }
        }
    }
}

/// Once a frame: notices changed sources, starts the build they ask for,
/// and swaps in what a finished build made.
pub(crate) fn poll(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    let reload = &mut editor.game.reload;
    if reload.watcher.as_mut().is_some_and(SourceWatcher::changed) {
        reload.queued = true;
    }
    if let Some(library) = reload.finished() {
        log::info!("the game was rebuilt; swapping it in");
        super::open_and_install(editor, frame, &library);
    }
    editor.game.reload.start_queued();
}
