//! Scene components and the geometry they turn into.
//!
//! Sits between `voltra-ecs`, which knows nothing about rendering, and
//! `voltra-render`, which knows nothing about entities. Dependencies point
//! down into both and never back up.

#![warn(missing_docs)]

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
pub mod curve;
pub mod edit;
pub mod filter;
pub mod format;
pub mod global_light;
pub mod gradient;
pub mod hierarchy;
pub mod interpolate;
pub mod joint;
pub mod keyframes;
pub mod light;
pub mod line2d;
pub mod locale;
pub mod material;
pub mod name;
pub mod one_way;
pub mod parallax;
pub mod particles;
pub mod persistent;
pub mod pick;
pub mod polygon2d;
pub mod post;
pub mod prefab;
pub mod replicated;
pub mod ron_store;
pub mod scene_id;
pub mod shadow_caster;
pub mod skeleton;
pub mod skinned;
pub mod sorting;
pub mod sprite;
pub mod sprite_material;
pub mod streaming_anchor;
pub mod text;
pub mod tilemap;
pub mod timer;
pub mod trail2d;
pub mod transform;
pub mod triangulate;
pub mod tween;
pub mod ui;
pub mod world_regions;

pub use animation::SpriteAnimation;
pub use animator::{Animator, StateMachine, StateMachines};
pub use audio_listener::AudioListener;
pub use audio_source::AudioSource;
pub use batch::{GpuEmitterDraw, MeshVertex, SpriteBatch, SpriteRange, Surface, TriangleMesh};
pub use body::{BodyType, RigidBody};
pub use camera::{Camera, CameraTarget, Viewport};
pub use camera_rig::{CameraFollow2D, CameraShake, FollowBounds};
pub use character::{CharacterBody2D, CharacterContact};
pub use collider::{Collider, ConvexPolygon, PolygonError, WorldShape};
pub use curve::{Curve, CurveKey};
pub use filter::{CollisionLayers, Sensor};
pub use format::{ComponentRegistry, ReplicateError, ReplicatedComponent, SceneError};
pub use global_light::GlobalLight2D;
pub use gradient::{Gradient, GradientKey};
pub use hierarchy::{Parent, WorldTransforms};
pub use interpolate::Interpolate;
pub use joint::{
    AngleLimit, Joint2D, JointImpulses, JointKind, LinearMotor, Motor, RestSpring, Spring,
    TranslationLimit,
};
pub use keyframes::{AnimationClip, AnimationClips, AnimationPlayer};
pub use light::{LightCone, LightShadows, PointLight2D};
pub use line2d::{Line2D, LineCap, LineJoint, LineTexture};
pub use locale::Locale;
pub use material::PhysicsMaterial;
pub use name::Name;
pub use one_way::OneWay;
pub use parallax::Parallax;
pub use particles::ParticleEmitter;
pub use persistent::Persistent;
pub use polygon2d::Polygon2D;
pub use post::PostProcess;
pub use prefab::{Instance, Prefab, PrefabInstance, PrefabPart, Prefabs};
pub use replicated::Replicated;
pub use scene_id::{SceneId, UnknownComponents};
pub use shadow_caster::{CasterShape, ShadowCaster2D};
pub use skeleton::{Bone2D, Skeleton2D, TwoBoneIk};
pub use skinned::SkinnedSprite;
pub use sorting::SortingLayers;
pub use sprite::{draw_key, Sprite};
pub use sprite_material::SpriteMaterial;
pub use streaming_anchor::StreamingAnchor;
pub use text::{Align, Text};
pub use tilemap::{Tile, TileMeshCache, Tilemap};
pub use timer::{TimerId, Timers};
pub use trail2d::Trail2D;
pub use transform::Transform;
pub use tween::{TweenId, Tweens};
pub use ui::{
    UiButton, UiControls, UiDropdown, UiHidden, UiLayout, UiOwnStyle, UiRect, UiScroll, UiSlice,
    UiSlider, UiSliderPart, UiStack, UiTab, UiTabs, UiTextField, UiTheme, UiToggle, UiTooltip,
};
pub use world_regions::{Region, WorldRegions, WorldRegionsError};
