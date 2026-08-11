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
//! and relaxation — and [`solver`] documents each piece where it lives.
//!
//! What is not simulated yet, so that its absence is not mistaken for a bug:
//! **rotation** (no angular velocity, and an `Aabb` stays upright in world
//! space), **sleeping** (a settled body is still solved every step) and
//! **continuous collision** (a fast enough body tunnels).

pub mod broad;
pub mod clock;
pub mod debug;
pub mod integrate;
pub mod narrow;
pub mod solver;
pub mod step;
pub mod world;

pub use broad::candidate_pairs;
pub use clock::PhysicsClock;
pub use integrate::{integrate_positions, integrate_velocities};
pub use narrow::{Contact, Manifold, ManifoldPoint};
pub use solver::{ImpulseCache, Softness, SolverBodies, SolverParams};
pub use step::step;
pub use world::PhysicsWorld;
