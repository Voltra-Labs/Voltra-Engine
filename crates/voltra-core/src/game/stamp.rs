//! How a game library says what built it, and the check that refuses one
//! the editor cannot trust.
//!
//! Rust has no stable ABI: a `Box<dyn Game>` made by a library is only a
//! valid `Box<dyn Game>` to the editor if both were built by the same
//! compiler from the same engine sources with the same features and flags.
//! And the world is keyed by `TypeId`, which hashes all of that in — a
//! library whose `Transform` has another `TypeId` would find no transforms
//! at all, silently. So the stamp carries the compiler's version, the
//! profile, the engine's version and a fingerprint of the `TypeId`s of the
//! engine types a game touches, and [`Stamp::check`] names the first that
//! differs.
//!
//! Everything here is plain data with a C layout, read through a C call,
//! before any Rust value crosses the boundary: it is how the editor learns
//! whether the rest of the library may be called at all.

use std::any::TypeId;
use std::fmt;
use std::hash::{Hash, Hasher};

/// The plugin protocol's version: the exported symbols and their types.
/// Raised whenever either changes.
pub const ABI: u32 = 1;

/// What built a game library, or this binary.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    /// The plugin protocol version, [`ABI`].
    pub abi: u32,
    /// `1` for a build with debug assertions — a dev profile — and `0` for
    /// one without.
    pub debug: u8,
    /// `rustc -V`, NUL-padded.
    pub rustc: [u8; 96],
    /// `voltra-core`'s version, NUL-padded.
    pub engine: [u8; 24],
    /// A hash of the `TypeId`s and layouts of the engine types a game shares
    /// with its host.
    pub fingerprint: u64,
}

impl Stamp {
    /// This build's stamp.
    pub fn current() -> Self {
        Self {
            abi: ABI,
            debug: u8::from(cfg!(debug_assertions)),
            rustc: padded(env!("VOLTRA_RUSTC_VERSION")),
            engine: padded(env!("CARGO_PKG_VERSION")),
            fingerprint: fingerprint(),
        }
    }

    /// The compiler that built it.
    pub fn rustc(&self) -> String {
        unpadded(&self.rustc)
    }

    /// The engine version it was built against.
    pub fn engine(&self) -> String {
        unpadded(&self.engine)
    }

    /// Whether a library stamped `self` can run in a host stamped `host`,
    /// and if not, the first reason why.
    pub fn check(&self, host: &Stamp) -> Result<(), Mismatch> {
        if self.abi != host.abi {
            return Err(Mismatch::Protocol {
                library: self.abi,
                host: host.abi,
            });
        }
        if self.rustc != host.rustc {
            return Err(Mismatch::Compiler {
                library: self.rustc(),
                host: host.rustc(),
            });
        }
        if self.engine != host.engine {
            return Err(Mismatch::Engine {
                library: self.engine(),
                host: host.engine(),
            });
        }
        if self.debug != host.debug {
            return Err(Mismatch::Profile {
                library_debug: self.debug != 0,
                host_debug: host.debug != 0,
            });
        }
        if self.fingerprint != host.fingerprint {
            return Err(Mismatch::Types);
        }
        Ok(())
    }
}

impl fmt::Debug for Stamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stamp")
            .field("abi", &self.abi)
            .field("debug", &self.debug)
            .field("rustc", &self.rustc())
            .field("engine", &self.engine())
            .field("fingerprint", &format_args!("{:016x}", self.fingerprint))
            .finish()
    }
}

/// Why a game library was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mismatch {
    /// Another version of the plugin protocol.
    Protocol {
        /// The library's protocol version.
        library: u32,
        /// The host's protocol version.
        host: u32,
    },
    /// Another compiler.
    Compiler {
        /// The compiler that built the library.
        library: String,
        /// The compiler that built the host.
        host: String,
    },
    /// Another version of the engine.
    Engine {
        /// The engine version the library was built against.
        library: String,
        /// The host's engine version.
        host: String,
    },
    /// A release build in a debug host, or the other way round.
    Profile {
        /// Whether the library has debug assertions.
        library_debug: bool,
        /// Whether the host has debug assertions.
        host_debug: bool,
    },
    /// The same compiler and profile, but the engine's types are not the
    /// same types: built with other features, other flags, or engine sources
    /// that changed since one of the two was built.
    Types,
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol { library, host } => write!(
                f,
                "it speaks plugin protocol {library} and this editor {host}: rebuild it against this engine"
            ),
            Self::Compiler { library, host } => write!(
                f,
                "it was built by {library} and this editor by {host}: build both with the same toolchain"
            ),
            Self::Engine { library, host } => write!(
                f,
                "it was built against voltra {library} and this editor is {host}"
            ),
            Self::Profile {
                library_debug,
                host_debug,
            } => {
                let name = |debug: &bool| if *debug { "debug" } else { "release" };
                write!(
                    f,
                    "it is a {} build and this editor a {} one: build both with the same profile",
                    name(library_debug),
                    name(host_debug)
                )
            }
            Self::Types => write!(
                f,
                "its engine types differ from this editor's — built with other features or flags, or from sources that have changed since: build the game and the editor in one `cargo build`"
            ),
        }
    }
}

/// The types a game and its host hand each other or both read the world
/// through, hashed with their layouts. A difference in any — another
/// feature set of a dependency, another `-C` flag, an edited source — moves
/// the hash.
fn fingerprint() -> u64 {
    fn mix<T: ?Sized + 'static>(hasher: &mut impl Hasher) {
        TypeId::of::<T>().hash(hasher);
    }
    fn layout<T>(hasher: &mut impl Hasher) {
        std::mem::size_of::<T>().hash(hasher);
        std::mem::align_of::<T>().hash(hasher);
    }
    fn component<T: 'static>(hasher: &mut impl Hasher) {
        mix::<T>(hasher);
        layout::<T>(hasher);
    }
    // SipHash with fixed keys: the same answer in every process.
    let mut hasher = std::hash::DefaultHasher::new();
    mix::<dyn crate::Game>(&mut hasher);
    mix::<crate::Tick<'static>>(&mut hasher);
    mix::<voltra_ecs::World>(&mut hasher);
    mix::<voltra_ecs::Entity>(&mut hasher);
    mix::<voltra_scene::ComponentRegistry>(&mut hasher);
    // The components the sample games read out of the world directly, and
    // the camera rig a game drives (`add_trauma`, a group's targets). A
    // `TypeId` moves with the crate's metadata, not with a field added, so
    // each is hashed with its layout too: a library built before
    // `CharacterBody2D` grew a field would otherwise pass and read
    // garbage through it.
    component::<voltra_scene::Transform>(&mut hasher);
    component::<voltra_scene::Sprite>(&mut hasher);
    component::<voltra_scene::RigidBody>(&mut hasher);
    component::<voltra_scene::Collider>(&mut hasher);
    component::<voltra_scene::Sensor>(&mut hasher);
    component::<voltra_scene::CharacterBody2D>(&mut hasher);
    component::<voltra_scene::CharacterContact>(&mut hasher);
    component::<voltra_scene::Camera>(&mut hasher);
    component::<voltra_scene::CameraFollow2D>(&mut hasher);
    component::<voltra_scene::CameraShake>(&mut hasher);
    component::<voltra_scene::Name>(&mut hasher);
    component::<voltra_scene::Text>(&mut hasher);
    component::<voltra_scene::AudioSource>(&mut hasher);
    component::<voltra_scene::ParticleEmitter>(&mut hasher);
    component::<voltra_scene::PrefabInstance>(&mut hasher);
    component::<voltra_scene::PrefabPart>(&mut hasher);
    component::<voltra_scene::Tilemap>(&mut hasher);
    mix::<voltra_physics::CollisionEvent>(&mut hasher);
    mix::<voltra_physics::JointBroken>(&mut hasher);
    mix::<voltra_render::glam::Vec2>(&mut hasher);
    mix::<String>(&mut hasher);
    layout::<crate::Tick<'static>>(&mut hasher);
    layout::<voltra_ecs::World>(&mut hasher);
    layout::<voltra_scene::ComponentRegistry>(&mut hasher);
    hasher.finish()
}

fn padded<const N: usize>(text: &str) -> [u8; N] {
    let mut out = [0; N];
    // Cut on a character boundary, keeping a NUL at the end.
    let mut end = text.len().min(N - 1);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    out[..end].copy_from_slice(&text.as_bytes()[..end]);
    out
}

fn unpadded(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_matches_itself() {
        assert_eq!(Stamp::current().check(&Stamp::current()), Ok(()));
        assert!(
            Stamp::current().rustc().starts_with("rustc"),
            "{:?}",
            Stamp::current()
        );
    }

    #[test]
    fn each_difference_is_named() {
        let host = Stamp::current();
        let other = |change: fn(&mut Stamp)| {
            let mut library = host;
            change(&mut library);
            library.check(&host).expect_err("a different build")
        };

        let compiler = other(|s| s.rustc = padded("rustc 1.0.0"));
        assert!(compiler.to_string().contains("rustc 1.0.0"), "{compiler}");
        assert!(matches!(other(|s| s.debug ^= 1), Mismatch::Profile { .. }));
        assert!(matches!(other(|s| s.fingerprint ^= 1), Mismatch::Types));
        assert!(matches!(other(|s| s.abi += 1), Mismatch::Protocol { .. }));
        assert!(matches!(
            other(|s| s.engine = padded("9.9.9")),
            Mismatch::Engine { .. }
        ));
    }

    #[test]
    fn the_compiler_is_named_before_the_types_it_changes() {
        // Another compiler changes every TypeId too; the useful answer is
        // the compiler, not "the types differ".
        let host = Stamp::current();
        let mut library = host;
        library.rustc = padded("rustc 1.0.0");
        library.fingerprint ^= 1;
        assert!(matches!(
            library.check(&host),
            Err(Mismatch::Compiler { .. })
        ));
    }

    #[test]
    fn padding_keeps_a_terminator_and_whole_characters() {
        let text = "ééééééééééééé";
        let stamped: [u8; 8] = padded(text);
        assert_eq!(stamped[7], 0);
        assert_eq!(unpadded(&stamped), "ééé");
    }
}
