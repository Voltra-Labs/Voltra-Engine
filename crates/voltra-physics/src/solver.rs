//! Turning contacts into velocities: TGS Soft.
//!
//! The solver was chosen in 11b-1 and recorded in `docs/ARCHITECTURE.md` so it
//! would not be re-argued: **TGS Soft**, which Erin Catto took for Box2D v3
//! after comparing eight solvers in [Solver2D]. Sub-stepping instead of
//! iterations, warm starting, soft constraints, relaxation — all four, because
//! dropping any one of them is what the comparison measured.
//!
//! The pieces, one per module:
//!
//! - [`softness`] — the three spring coefficients a soft constraint needs.
//! - [`params`] — the tuning every one of them reads.
//! - [`body`] — the dense array of bodies a step solves over, gathered from the
//!   ECS and scattered back at the end.
//! - [`cache`] — the impulses that survive from one step to the next, which is
//!   what warm starting means.
//! - [`constraint`] — one contact, precomputed into what the passes need.
//! - [`contact`] — the passes themselves: warm start, solve, restitution.
//!
//! [Solver2D]: https://box2d.org/posts/2024/02/solver2d/

pub mod params;
pub mod softness;

pub use params::SolverParams;
pub use softness::Softness;
