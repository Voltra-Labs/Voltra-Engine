//! The bounds the store reads stored instances within, and why one was
//! left as its file has it.

use std::fmt;

use voltra_assets::AssetPath;

/// How deep stored instances may go — a variant of a variant of …, a prefab
/// nested in a prefab nested in … — before the rest is left as its file
/// has it. Bounds the store's recursion on a hand-made chain.
pub const MAX_DEPTH: usize = 32;

/// How many records a prefab may hold once its stored instances are
/// resolved. Each nesting level can multiply what the level below adds, so
/// a few small files could otherwise ask for billions; past this, an
/// instance is left as its file has it.
pub const MAX_RECORDS: usize = 1 << 17;

/// How many records of one prefab file may carry a link — be the root of an
/// instance stored in it — before the store follows none of them. A level
/// with this many prefab instances is read as a scene instead
/// ([`super::Prefabs::load_scene`]), whose instances a sync brings in step.
pub const MAX_LINKS: usize = 1 << 14;

/// How many different prefabs one file may hold instances of before the
/// store follows none of them: each is a file to read.
pub const MAX_SOURCES: usize = 256;

/// Why an instance stored in a prefab was left as its file has it rather
/// than brought in step with its own file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveProblem {
    /// Following the instances leads back to a prefab already being read.
    Cycle {
        /// The prefabs on the way, the one being read first, ending with
        /// the one that closes the loop.
        chain: Vec<AssetPath>,
    },
    /// The instance of `source` is deeper than [`MAX_DEPTH`].
    TooDeep {
        /// The prefab the instance would follow.
        source: AssetPath,
    },
    /// Following `source` would take the prefab past [`MAX_RECORDS`].
    TooLarge {
        /// The prefab the instance would follow.
        source: AssetPath,
    },
    /// The file holds more stored instances than [`MAX_LINKS`], or of more
    /// prefabs than [`MAX_SOURCES`]: none are followed.
    TooManyLinks {
        /// Records carrying a link.
        links: usize,
        /// Different prefabs they name.
        sources: usize,
    },
    /// The instance of `source` would not come into step with it.
    Failed {
        /// The prefab the instance would follow.
        source: AssetPath,
        /// What went wrong.
        error: String,
    },
}

impl fmt::Display for ResolveProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cycle { chain } => {
                let names: Vec<&str> = chain.iter().map(AssetPath::as_str).collect();
                write!(f, "holds an instance of itself: {}", names.join(" › "))
            }
            Self::TooDeep { source } => write!(
                f,
                "its instance of {} is nested deeper than {MAX_DEPTH} prefabs",
                source.as_str()
            ),
            Self::TooLarge { source } => write!(
                f,
                "following {} would hold more than {MAX_RECORDS} entities",
                source.as_str()
            ),
            Self::TooManyLinks { links, sources } => write!(
                f,
                "holds {links} instances of {sources} prefabs, past the {MAX_LINKS} / \
                 {MAX_SOURCES} the store follows"
            ),
            Self::Failed { source, error } => {
                write!(
                    f,
                    "its instance of {} would not follow it: {error}",
                    source.as_str()
                )
            }
        }
    }
}
