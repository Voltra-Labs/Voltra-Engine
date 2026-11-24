//! In-house entity-component storage.

#![warn(missing_docs)]

pub mod entity;
pub mod hash;
pub mod storage;
pub mod world;

pub use entity::{Entities, Entity};
pub use storage::SparseSet;
pub use world::World;
