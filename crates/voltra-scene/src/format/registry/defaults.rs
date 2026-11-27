//! The component types every registry starts with.

use super::ComponentRegistry;
use crate::{
    AudioListener, AudioSource, Camera, Collider, CollisionLayers, GlobalLight2D, Name, Parallax,
    Parent, ParticleEmitter, Persistent, PhysicsMaterial, PointLight2D, PrefabInstance, RigidBody,
    Sensor, ShadowCaster2D, Sprite, SpriteAnimation, Text, Tilemap, Transform, UiButton, UiRect,
    UiScroll, UiSlice, UiSlider, UiSliderPart, UiStack, UiTextField, UiToggle,
};

impl ComponentRegistry {
    /// Every component type this crate defines.
    ///
    /// Registering here rather than making callers opt in is deliberate: there
    /// are several call sites, and forgetting one would not fail — it would
    /// silently drop that component from that save. A format that loses data
    /// when a caller forgets a line is not a format.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register::<Name>("Name").with_template();
        // Replicated by default: where a networked thing is, is what every
        // networked game sends, and remote copies are drawn between
        // snapshots rather than snapped to each.
        registry
            .register::<Transform>("Transform")
            .with_template()
            .replicated_interpolated();
        registry
            .register::<crate::Replicated>(crate::Replicated::NAME)
            .with_template();
        registry
            .register::<crate::LagCompensated>(crate::LagCompensated::NAME)
            .with_template();
        // Stored as the parent's `SceneId`; the live `Entity` is resolved after
        // the whole file is in, by `hierarchy::resolve_parents`.
        registry.register::<Parent>("Parent");
        registry.register::<Sprite>("Sprite").with_template();
        registry
            .register::<SpriteAnimation>("SpriteAnimation")
            .with_template();
        registry
            .register::<crate::Animator>(crate::Animator::NAME)
            .with_template();
        registry
            .register::<crate::AnimationPlayer>(crate::AnimationPlayer::NAME)
            .with_template();
        registry.register::<Camera>("Camera").with_template();
        registry
            .register_with_refs::<crate::CameraFollow2D>(crate::CameraFollow2D::NAME)
            .with_template();
        registry
            .register::<crate::CameraShake>(crate::CameraShake::NAME)
            .with_template();
        registry
            .register::<AudioSource>("AudioSource")
            .with_template();
        registry
            .register::<AudioListener>("AudioListener")
            .with_template();
        registry.register::<RigidBody>("RigidBody").with_template();
        registry.register::<Collider>("Collider").with_template();
        registry
            .register::<PhysicsMaterial>("PhysicsMaterial")
            .with_template();
        registry
            .register::<CollisionLayers>("CollisionLayers")
            .with_template();
        registry.register::<Sensor>("Sensor").with_template();
        registry
            .register::<crate::OneWay>(crate::OneWay::NAME)
            .with_template();
        registry
            .register::<crate::CharacterBody2D>(crate::CharacterBody2D::NAME)
            .with_template();
        registry.register::<PrefabInstance>(PrefabInstance::NAME);
        registry.register::<crate::prefab::PrefabPart>(crate::prefab::PrefabPart::NAME);
        registry.register::<Tilemap>("Tilemap").with_template();
        registry.register::<Text>("Text").with_template();
        registry.register::<UiRect>("UiRect").with_template();
        registry.register::<UiButton>("UiButton").with_template();
        registry.register::<UiSlice>("UiSlice").with_template();
        registry.register::<UiStack>("UiStack").with_template();
        registry
            .register::<crate::CameraTarget>("CameraTarget")
            .with_template();
        registry.register::<UiScroll>("UiScroll").with_template();
        registry.register::<UiToggle>("UiToggle").with_template();
        registry.register::<UiSlider>("UiSlider").with_template();
        registry
            .register::<UiSliderPart>("UiSliderPart")
            .with_template();
        registry
            .register::<UiTextField>("UiTextField")
            .with_template();
        registry
            .register::<crate::UiDropdown>("UiDropdown")
            .with_template();
        registry
            .register::<crate::UiTooltip>("UiTooltip")
            .with_template();
        registry.register::<crate::UiTabs>("UiTabs").with_template();
        registry.register::<crate::UiTab>("UiTab").with_template();
        registry
            .register::<crate::UiHidden>("UiHidden")
            .with_template();
        registry
            .register::<crate::UiOwnStyle>("UiOwnStyle")
            .with_template();
        registry
            .register::<Persistent>("Persistent")
            .with_template();
        registry
            .register::<crate::StreamingAnchor>(crate::StreamingAnchor::NAME)
            .with_template();
        registry.register::<Parallax>("Parallax").with_template();
        registry
            .register::<ParticleEmitter>("ParticleEmitter")
            .with_template();
        registry
            .register::<PointLight2D>("PointLight2D")
            .with_template();
        registry
            .register::<GlobalLight2D>("GlobalLight2D")
            .with_template();
        registry
            .register_with_refs::<crate::Joint2D>(crate::Joint2D::NAME)
            .with_template();
        registry
            .register::<ShadowCaster2D>("ShadowCaster2D")
            .with_template();
        registry.register::<crate::Line2D>("Line2D").with_template();
        registry
            .register::<crate::Polygon2D>("Polygon2D")
            .with_template();
        registry
            .register::<crate::Trail2D>("Trail2D")
            .with_template();
        registry
            .register::<crate::Skeleton2D>("Skeleton2D")
            .with_template();
        registry.register::<crate::Bone2D>("Bone2D").with_template();
        registry
            .register::<crate::TwoBoneIk>("TwoBoneIk")
            .with_template();
        registry
            .register::<crate::SkinnedSprite>("SkinnedSprite")
            .with_template();
        registry
    }
}
