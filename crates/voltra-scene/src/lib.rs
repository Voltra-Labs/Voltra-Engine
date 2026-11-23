//! Scene components and the geometry they turn into.
//!
//! Sits between `voltra-ecs`, which knows nothing about rendering, and
//! `voltra-render`, which knows nothing about entities. Dependencies point
//! down into both and never back up.

pub mod animation;
pub mod animator;
pub mod audio_listener;
pub mod audio_source;
pub mod batch;
pub mod body;
pub mod camera;
pub mod camera_rig;
pub mod character;
pub mod collider;
pub mod edit;
pub mod filter;
pub mod format;
pub mod global_light;
pub mod hierarchy;
pub mod joint;
pub mod keyframes;
pub mod light;
pub mod material;
pub mod name;
pub mod one_way;
pub mod parallax;
pub mod particles;
pub mod persistent;
pub mod pick;
pub mod post;
pub mod prefab;
pub mod ron_store;
pub mod scene_id;
pub mod shadow_caster;
pub mod sorting;
pub mod sprite;
pub mod text;
pub mod tilemap;
pub mod transform;
pub mod ui;

pub use animation::SpriteAnimation;
pub use animator::{Animator, StateMachine, StateMachines};
pub use audio_listener::AudioListener;
pub use audio_source::AudioSource;
pub use batch::{GpuEmitterDraw, SpriteBatch, SpriteRange, Surface};
pub use body::{BodyType, RigidBody};
pub use camera::Camera;
pub use camera_rig::{CameraFollow2D, CameraShake, FollowBounds};
pub use character::{CharacterBody2D, CharacterContact};
pub use collider::{Collider, ConvexPolygon, PolygonError, WorldShape};
pub use filter::{CollisionLayers, Sensor};
pub use format::{ComponentRegistry, SceneError};
pub use global_light::GlobalLight2D;
pub use hierarchy::{Parent, WorldTransforms};
pub use joint::{AngleLimit, Joint2D, JointImpulses, JointKind, Motor, Spring};
pub use keyframes::{AnimationClip, AnimationClips, AnimationPlayer};
pub use light::{LightCone, LightShadows, PointLight2D};
pub use material::PhysicsMaterial;
pub use name::Name;
pub use one_way::OneWay;
pub use parallax::Parallax;
pub use particles::ParticleEmitter;
pub use persistent::Persistent;
pub use post::PostProcess;
pub use prefab::{Instance, Prefab, PrefabInstance, PrefabPart, Prefabs};
pub use scene_id::{SceneId, UnknownComponents};
pub use shadow_caster::{CasterShape, ShadowCaster2D};
pub use sorting::SortingLayers;
pub use sprite::{draw_key, Sprite};
pub use text::{Align, Text};
pub use tilemap::{Tile, TileMeshCache, Tilemap};
pub use transform::Transform;
pub use ui::{
    UiButton, UiControls, UiLayout, UiRect, UiScroll, UiSlice, UiSlider, UiSliderPart, UiStack,
    UiTextField, UiToggle,
};
