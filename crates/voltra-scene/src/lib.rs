//! Scene components and the geometry they turn into.
//!
//! Sits between `voltra-ecs`, which knows nothing about rendering, and
//! `voltra-render`, which knows nothing about entities. Dependencies point
//! down into both and never back up.

pub mod batch;
pub mod body;
pub mod camera;
pub mod collider;
pub mod filter;
pub mod format;
pub mod hierarchy;
pub mod material;
pub mod name;
pub mod pick;
pub mod scene_id;
pub mod sprite;
pub mod transform;

pub use batch::{SpriteBatch, SpriteRange};
pub use body::{BodyType, RigidBody};
pub use camera::Camera;
pub use collider::Collider;
pub use filter::{CollisionLayers, Sensor};
pub use format::{ComponentRegistry, SceneError};
pub use hierarchy::{Parent, WorldTransforms};
pub use material::PhysicsMaterial;
pub use name::Name;
pub use scene_id::{SceneId, UnknownComponents};
pub use sprite::{draw_key, Sprite};
pub use transform::Transform;
