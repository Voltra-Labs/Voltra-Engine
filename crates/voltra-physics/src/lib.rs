//! 2D rigid bodies, integration and contact detection.
//!
//! The components are in `voltra-scene`, not here: `ComponentRegistry` lives
//! there, and registering physics components from there would need
//! `voltra-scene → voltra-physics`, while integration needs `Transform` and so
//! needs the edge back. This crate is the simulation over those components and
//! holds nothing a scene file contains.
//!
//! **Nothing in this stage resolves a contact.** Bodies move, overlaps are
//! found and reported, and a body will sink through a floor. The solver is the
//! next stage, and it consumes exactly the contact list produced here.

pub mod broad;
pub mod clock;
pub mod debug;
pub mod integrate;
pub mod narrow;
pub mod step;

pub use broad::candidate_pairs;
pub use clock::PhysicsClock;
pub use integrate::integrate;
pub use narrow::Contact;
pub use step::step;
