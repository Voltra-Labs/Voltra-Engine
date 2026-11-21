//! Scene components and the geometry they turn into.
//!
//! Sits between `voltra-ecs`, which knows nothing about rendering, and
//! `voltra-render`, which knows nothing about entities. Dependencies point
//! down into both and never back up.

pub mod animation;
pub mod audio_listener;
pub mod audio_source;
pub mod batch;
pub mod body;
pub mod camera;
pub mod collider;
pub mod filter;
pub mod format;
pub mod hierarchy;
pub mod material;
pub mod name;
pub mod parallax;
pub mod particles;
pub mod persistent;
pub mod pick;
pub mod post;
pub mod prefab;
pub mod scene_id;
pub mod sorting;
pub mod sprite;
pub mod text;
pub mod tilemap;
pub mod transform;
pub mod ui;

pub use animation::SpriteAnimation;
pub use audio_listener::AudioListener;
pub use audio_source::AudioSource;
pub use batch::{SpriteBatch, SpriteRange};
pub use body::{BodyType, RigidBody};
pub use camera::Camera;
pub use collider::Collider;
pub use filter::{CollisionLayers, Sensor};
pub use format::{ComponentRegistry, SceneError};
pub use hierarchy::{Parent, WorldTransforms};
pub use material::PhysicsMaterial;
pub use name::Name;
pub use parallax::Parallax;
pub use particles::ParticleEmitter;
pub use persistent::Persistent;
pub use post::PostProcess;
pub use prefab::{Instance, Prefab, PrefabInstance, Prefabs};
pub use scene_id::{SceneId, UnknownComponents};
pub use sorting::SortingLayers;
pub use sprite::{draw_key, Sprite};
pub use text::{Align, Text};
pub use tilemap::{Tile, TileMeshCache, Tilemap};
pub use transform::Transform;
pub use ui::{UiButton, UiButtons, UiLayout, UiRect};
