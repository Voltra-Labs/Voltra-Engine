//! In-house entity-component storage.

pub mod entity;
pub mod storage;
pub mod world;

pub use entity::{Entities, Entity};
pub use storage::SparseSet;
pub use world::World;
