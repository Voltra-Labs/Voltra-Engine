//! Scene components and the geometry they turn into.
//!
//! Sits between `voltra-ecs`, which knows nothing about rendering, and
//! `voltra-render`, which knows nothing about entities. Dependencies point
//! down into both and never back up.

pub mod batch;
pub mod pick;
pub mod sprite;
pub mod transform;

pub use batch::SpriteBatch;
pub use sprite::Sprite;
pub use transform::Transform;
