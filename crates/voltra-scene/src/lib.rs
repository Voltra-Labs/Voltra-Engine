//! Scene components and the geometry they turn into.
//!
//! Sits between `voltra-ecs`, which knows nothing about rendering, and
//! `voltra-render`, which knows nothing about entities. Dependencies point
//! down into both and never back up.

pub mod batch;
pub mod format;
pub mod pick;
pub mod scene_id;
pub mod sprite;
pub mod transform;

pub use batch::{SpriteBatch, SpriteRange};
pub use format::{ComponentRegistry, SceneError};
pub use scene_id::{SceneId, UnknownComponents};
pub use sprite::{draw_key, Sprite};
pub use transform::Transform;
