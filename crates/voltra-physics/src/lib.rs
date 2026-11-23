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
//! rotate; every shape — box, circle, capsule, convex polygon — is oriented,
//! and a contact carries up to two points ([`narrow`]).
//!
//! Bodies at rest sleep, by island, and wake when disturbed — [`sleep`].
//!
//! [`query`] asks the world without stepping it — rays, points, boxes and
//! shape casts — and [`character`] moves a game's character with them.
//!
//! A fast body stops at what it meets instead of passing through it in one
//! step: against static geometry always, against moving bodies when it is a
//! `RigidBody::bullet` — continuous collision, by speculative contacts ahead
//! of the solve and a time of impact after it (`continuous`).
//!
//! What is not simulated yet, so that its absence is not mistaken for a bug:
//! **rolling resistance** (a circle on a floor rolls forever), and a fast
//! body that is not a bullet still passes through a thin *moving* one. Mass ratios
//! past roughly a hundred to one squeeze the lighter body out sideways rather
//! than holding it, which is the ordinary limit of an impulse solver.

pub mod broad;
pub mod character;
pub mod clock;
mod continuous;
pub mod debug;
pub mod events;
mod geometry;
/// The fast entity-keyed maps, which live in `voltra-ecs` beside the type
/// they hash.
pub use voltra_ecs::hash;
pub mod integrate;
pub mod narrow;
pub mod one_way;
pub mod query;
pub mod sleep;
pub mod solver;
pub mod step;
pub mod world;

pub use broad::{candidate_pairs, sweep, Sweep};
pub use character::{move_and_slide, Slide};
pub use clock::PhysicsClock;
pub use events::{CollisionEvent, Touch, Touching};
pub use integrate::{integrate_positions, integrate_velocities};
pub use narrow::{Contact, Manifold, ManifoldPoint};
pub use one_way::OneWayPasses;
pub use query::{QueryFilter, RayHit, ShapeHit};
pub use sleep::{Sleep, SleepParams};
pub use solver::{ImpulseCache, Softness, SolverBodies, SolverParams};
pub use step::{step, step_sleeping, Overlaps};
pub use world::PhysicsWorld;
