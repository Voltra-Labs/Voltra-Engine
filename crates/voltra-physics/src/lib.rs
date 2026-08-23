//! 2D rigid bodies: integration, contact detection and the solver that
//! resolves them.
//!
//! The components are in `voltra-scene`, not here: `ComponentRegistry` lives
//! there, and registering physics components from there would need
//! `voltra-scene → voltra-physics`, while integration needs `Transform` and so
//! needs the edge back. This crate is the simulation over those components and
//! holds nothing a scene file contains.
//!
//! [`PhysicsWorld`] is the entry point: it owns the fixed clock, the solver's
//! tuning and the contact impulses that have to survive from one step to the
//! next. The solver is TGS Soft — sub-stepping, warm starting, soft constraints
//! and relaxation — and [`solver`] documents each piece where it lives. Bodies
//! rotate, boxes are oriented, and a contact carries up to two points.
//!
//! What is not simulated yet, so that its absence is not mistaken for a bug:
//! **sleeping** (a settled body is still solved every step, and keeps a
//! millimetre of jitter), **continuous collision** (a fast enough body tunnels)
//! and **rolling resistance** (a circle on a floor rolls forever). Mass ratios
//! past roughly a hundred to one squeeze the lighter body out sideways rather
//! than holding it, which is the ordinary limit of an impulse solver.

pub mod broad;
pub mod clock;
pub mod debug;
pub mod events;
pub mod integrate;
pub mod narrow;
pub mod query;
pub mod solver;
pub mod step;
pub mod world;

pub use broad::candidate_pairs;
pub use clock::PhysicsClock;
pub use events::{CollisionEvent, Touch, Touching};
pub use integrate::{integrate_positions, integrate_velocities};
pub use narrow::{Contact, Manifold, ManifoldPoint};
pub use query::{QueryFilter, RayHit};
pub use solver::{ImpulseCache, Softness, SolverBodies, SolverParams};
pub use step::{step, Overlaps};
pub use world::PhysicsWorld;
