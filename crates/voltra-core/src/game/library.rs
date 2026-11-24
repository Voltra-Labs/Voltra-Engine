//! Opening a game's dynamic library and making games out of it.
//!
//! The library is opened from a **copy**: Windows will not let a build
//! overwrite a library that is loaded, and a dynamic loader asked twice for
//! one path hands back the first load. A copy per load leaves the build
//! output free and makes every load a fresh one.
//!
//! What a library made — a game, the storages its components live in, the
//! functions it registered under their names — runs its code from the
//! library's pages, and calling any of it after the library is unloaded is
//! a crash. So a [`GameLibrary`] dropped as it is stays loaded until the
//! process ends, and only [`GameLibrary::close`] unloads it, for a caller
//! that has rebuilt everything the library touched without it.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use voltra_scene::ComponentRegistry;

use super::plugin::{HostLog, NewFn, StampFn, NEW_SYMBOL, STAMP_SYMBOL};
use super::stamp::{Mismatch, Stamp};
use crate::{Game, Tick};

/// The file name cargo gives `package`'s `cdylib` on this platform:
/// `libmy_game.so`, `my_game.dll`, `libmy_game.dylib`.
pub fn library_file(package: &str) -> String {
    format!(
        "{}{}{}",
        std::env::consts::DLL_PREFIX,
        package.replace('-', "_"),
        std::env::consts::DLL_SUFFIX
    )
}

/// A loaded game library, checked.
pub struct GameLibrary {
    /// `None` only in `drop`, which takes it.
    library: Option<Arc<libloading::Library>>,
    new: NewFn,
    path: PathBuf,
    copy: PathBuf,
    stamp: Stamp,
    /// Set by `close`, the one way the drop unloads.
    closing: bool,
}

/// Why a game library could not be used.
#[derive(Debug)]
pub enum LoadError {
    /// There is no file there: the game has not been built.
    Missing(PathBuf),
    /// The copy it is loaded from could not be made.
    Copy {
        /// The library that was to be copied.
        path: PathBuf,
        /// Why the copy failed.
        error: std::io::Error,
    },
    /// The platform's loader refused it.
    Open {
        /// The library that was refused.
        path: PathBuf,
        /// The loader's error.
        error: libloading::Error,
    },
    /// It exports no stamp: not a game library, or one from before them.
    NotAGame(PathBuf),
    /// Built in a way this host cannot trust; see [`Mismatch`].
    Refused {
        /// The library that was refused.
        path: PathBuf,
        /// Why.
        mismatch: Mismatch,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(path) => write!(f, "no game library at {}: build the game first", path.display()),
            Self::Copy { path, error } => {
                write!(f, "could not copy {} to load it: {error}", path.display())
            }
            Self::Open { path, error } => write!(f, "could not load {}: {error}", path.display()),
            Self::NotAGame(path) => write!(
                f,
                "{} is not a game library: it exports no `voltra_game_stamp` (see `voltra_core::export_game!`)",
                path.display()
            ),
            Self::Refused { path, mismatch } => {
                write!(f, "{} cannot be run: {mismatch}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// Numbers the copies one process makes.
static COPIES: AtomicU32 = AtomicU32::new(0);

impl GameLibrary {
    /// Loads the game library at `path` and checks its stamp against this
    /// build's.
    ///
    /// Nothing of the library runs before the stamp is read, and nothing
    /// but the stamp unless it matches: a refused library is unloaded
    /// before this returns, having made nothing.
    pub fn open(path: &Path) -> Result<Self, LoadError> {
        if !path.is_file() {
            return Err(LoadError::Missing(path.to_owned()));
        }
        let copy = copy_to_load(path)?;
        // SAFETY: loading runs the library's initialisers. A cdylib built by
        // cargo from a Rust crate has none beyond the standard library's.
        let library = unsafe { libloading::Library::new(&copy) }.map_err(|error| {
            remove_copy(&copy);
            LoadError::Open {
                path: path.to_owned(),
                error,
            }
        })?;
        let refuse = |library: libloading::Library, error: LoadError| {
            drop(library);
            remove_copy(&copy);
            Err(error)
        };
        // SAFETY: the symbol's type is the one `export_game!` declares it
        // with, a C function of no arguments answering a C struct.
        let stamp = match unsafe { library.get::<StampFn>(STAMP_SYMBOL) } {
            Ok(symbol) => symbol(),
            Err(_) => return refuse(library, LoadError::NotAGame(path.to_owned())),
        };
        if let Err(mismatch) = stamp.check(&Stamp::current()) {
            let path = path.to_owned();
            return refuse(library, LoadError::Refused { path, mismatch });
        }
        // SAFETY: the stamps match, so `voltra_game_new` was compiled by
        // this compiler against these engine types, with the type
        // `export_game!` gives it.
        let new = match unsafe { library.get::<NewFn>(NEW_SYMBOL) } {
            Ok(symbol) => *symbol,
            Err(_) => return refuse(library, LoadError::NotAGame(path.to_owned())),
        };
        log::info!("loaded the game library {}", path.display());
        Ok(Self {
            library: Some(Arc::new(library)),
            new,
            path: path.to_owned(),
            copy,
            stamp,
            closing: false,
        })
    }

    /// The library's own path, not the copy it runs from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What built the library.
    pub fn stamp(&self) -> &Stamp {
        &self.stamp
    }

    /// A fresh game. It keeps the library loaded for as long as it lives,
    /// whatever happens to `self`.
    pub fn new_game(&self) -> Box<dyn Game> {
        let game = (self.new)(HostLog::of_this_process());
        let library = self
            .library
            .clone()
            .expect("the library is only taken by the drop");
        Box::new(Loaded {
            game,
            _library: library,
        })
    }

    /// Names the game's components in `registry`, through a game made for
    /// the purpose and dropped after.
    pub fn register(&self, registry: &mut ComponentRegistry) {
        self.new_game().register(registry);
    }

    /// Unloads the library, once the last game it made is gone.
    ///
    /// # Safety
    ///
    /// Nothing the library made may outlive the unload but the games
    /// themselves, which keep it loaded until they drop: no registry
    /// holding its components' functions, no world holding a storage it
    /// created, no value of one of its types anywhere. The editor rebuilds
    /// its world and registries without the game before calling this.
    pub unsafe fn close(mut self) {
        self.closing = true;
    }
}

impl Drop for GameLibrary {
    fn drop(&mut self) {
        let Some(library) = self.library.take() else {
            return;
        };
        if !self.closing {
            // Kept loaded for the rest of the process: see the module's
            // comment. The count is never given back.
            std::mem::forget(library);
            return;
        }
        match Arc::try_unwrap(library) {
            Ok(library) => {
                drop(library);
                remove_copy(&self.copy);
                log::info!("unloaded the game library {}", self.path.display());
            }
            // A game still holds it, and unloads it when it drops.
            Err(shared) => drop(shared),
        }
    }
}

/// A game from a library, holding the library open.
///
/// Field order is drop order: the game goes first, while its code is still
/// loaded.
struct Loaded {
    game: Box<dyn Game>,
    _library: Arc<libloading::Library>,
}

impl Game for Loaded {
    fn register(&self, registry: &mut ComponentRegistry) {
        self.game.register(registry);
    }

    fn start(&mut self, tick: &mut Tick<'_>) {
        self.game.start(tick);
    }

    fn update(&mut self, tick: &mut Tick<'_>) {
        self.game.update(tick);
    }

    fn fixed_update(&mut self, tick: &mut Tick<'_>) {
        self.game.fixed_update(tick);
    }
}

/// Where copies are loaded from: a folder of this user's temporary files.
fn copies_dir() -> PathBuf {
    std::env::temp_dir().join("voltra-games")
}

fn copy_to_load(path: &Path) -> Result<PathBuf, LoadError> {
    let dir = copies_dir();
    let failed = |error| LoadError::Copy {
        path: path.to_owned(),
        error,
    };
    std::fs::create_dir_all(&dir).map_err(failed)?;
    remove_stale_copies(&dir);
    let name = path
        .file_name()
        .map_or_else(|| "game".into(), |n| n.to_string_lossy().into_owned());
    let copy = dir.join(format!(
        "{}-{}-{name}",
        std::process::id(),
        COPIES.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::copy(path, &copy).map_err(failed)?;
    Ok(copy)
}

/// Removes the copies earlier processes left: a library kept loaded until
/// its process ended was never unloaded, so its copy was never removed, and
/// a debug build's library is hundreds of megabytes. One another process
/// still has loaded is left be where the system refuses, and unlinked where
/// it allows, which leaves that process's mapping intact.
fn remove_stale_copies(dir: &Path) {
    let ours = format!("{}-", std::process::id());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(&ours) {
            remove_copy(&entry.path());
        }
    }
}

fn remove_copy(copy: &Path) {
    // A copy the loader still holds cannot be removed on Windows; it is a
    // temporary file and the system clears those.
    let _ = std::fs::remove_file(copy);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_is_named_as_cargo_names_it() {
        let name = library_file("voltra-platformer");
        assert!(name.contains("voltra_platformer"), "{name}");
        assert!(name.ends_with(std::env::consts::DLL_SUFFIX), "{name}");
    }

    #[test]
    fn a_game_not_built_yet_says_so() {
        let error = GameLibrary::open(Path::new("no/such/libgame.so"))
            .err()
            .expect("nothing there");
        assert!(matches!(error, LoadError::Missing(_)), "{error}");
        assert!(
            error.to_string().contains("build the game first"),
            "{error}"
        );
    }

    #[test]
    fn a_file_that_is_no_library_is_refused_and_leaves_no_copy() {
        let dir = voltra_testkit::scratch_root();
        let file = dir.join(library_file("fake"));
        std::fs::write(&file, b"not a library").expect("write");
        let before = std::fs::read_dir(copies_dir()).map_or(0, |d| d.count());

        let error = GameLibrary::open(&file).err().expect("refused");

        assert!(matches!(error, LoadError::Open { .. }), "{error}");
        let after = std::fs::read_dir(copies_dir()).map_or(0, |d| d.count());
        assert!(after <= before, "the copy was cleaned up");
    }
}
