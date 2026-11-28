//! The component types every registry starts with.

use super::ComponentRegistry;
use crate::{
    AudioListener, AudioSource, Camera2D, Collider2D, CollisionLayers, GlobalLight2D, Name,
    Parallax, Parent, ParticleEmitter, Persistent, PhysicsMaterial, PointLight2D, PrefabInstance,
    RigidBody2D, Sensor, ShadowCaster2D, Sprite2D, SpriteAnimation, Text, Tilemap, Transform2D,
    UiButton, UiRect, UiScroll, UiSlice, UiSlider, UiSliderPart, UiStack, UiTextField, UiToggle,
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
        registry.register::<Name>(Name::NAME).with_template();
        // Replicated by default: where a networked thing is, is what every
        // networked game sends, and remote copies are drawn between
        // snapshots rather than snapped to each.
        registry
            .register::<Transform2D>(Transform2D::NAME)
            .with_template()
            .replicated_interpolated();
        // The same for 3D: interpolated (translation and scale linearly, the
        // rotation by slerp), and placed for interest at its ground-plane
        // (x, z). Plain postcard, 40 bytes, until its own wire form (156).
        registry
            .register::<crate::Transform3D>(crate::Transform3D::NAME)
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
        registry.register::<Parent>(Parent::NAME);
        registry
            .register::<Sprite2D>(Sprite2D::NAME)
            .with_template();
        registry
            .register::<SpriteAnimation>(SpriteAnimation::NAME)
            .with_template();
        registry
            .register::<crate::Animator>(crate::Animator::NAME)
            .with_template();
        registry
            .register::<crate::AnimationPlayer>(crate::AnimationPlayer::NAME)
            .with_template();
        registry
            .register::<Camera2D>(Camera2D::NAME)
            .with_template();
        registry
            .register::<crate::Camera3D>(crate::Camera3D::NAME)
            .with_template();
        registry
            .register::<crate::MeshInstance3D>(crate::MeshInstance3D::NAME)
            .with_template();
        registry
            .register::<crate::DirectionalLight3D>(crate::DirectionalLight3D::NAME)
            .with_template();
        registry
            .register::<crate::Environment3D>(crate::Environment3D::NAME)
            .with_template();
        registry
            .register_with_refs::<crate::CameraFollow2D>(crate::CameraFollow2D::NAME)
            .with_template();
        registry
            .register::<crate::CameraShake>(crate::CameraShake::NAME)
            .with_template();
        registry
            .register::<AudioSource>(AudioSource::NAME)
            .with_template();
        registry
            .register::<AudioListener>(AudioListener::NAME)
            .with_template();
        registry
            .register::<RigidBody2D>(RigidBody2D::NAME)
            .with_template();
        registry
            .register::<Collider2D>(Collider2D::NAME)
            .with_template();
        registry
            .register::<PhysicsMaterial>(PhysicsMaterial::NAME)
            .with_template();
        registry
            .register::<CollisionLayers>(CollisionLayers::NAME)
            .with_template();
        registry.register::<Sensor>(Sensor::NAME).with_template();
        registry
            .register::<crate::OneWay>(crate::OneWay::NAME)
            .with_template();
        registry
            .register::<crate::CharacterBody2D>(crate::CharacterBody2D::NAME)
            .with_template();
        registry.register::<PrefabInstance>(PrefabInstance::NAME);
        registry.register::<crate::prefab::PrefabPart>(crate::prefab::PrefabPart::NAME);
        registry.register::<Tilemap>(Tilemap::NAME).with_template();
        registry.register::<Text>(Text::NAME).with_template();
        registry.register::<UiRect>(UiRect::NAME).with_template();
        registry
            .register::<UiButton>(UiButton::NAME)
            .with_template();
        registry.register::<UiSlice>(UiSlice::NAME).with_template();
        registry.register::<UiStack>(UiStack::NAME).with_template();
        registry
            .register::<crate::CameraTarget>(crate::CameraTarget::NAME)
            .with_template();
        registry
            .register::<UiScroll>(UiScroll::NAME)
            .with_template();
        registry
            .register::<UiToggle>(UiToggle::NAME)
            .with_template();
        registry
            .register::<UiSlider>(UiSlider::NAME)
            .with_template();
        registry
            .register::<UiSliderPart>(UiSliderPart::NAME)
            .with_template();
        registry
            .register::<UiTextField>(UiTextField::NAME)
            .with_template();
        registry
            .register::<crate::UiDropdown>(crate::UiDropdown::NAME)
            .with_template();
        registry
            .register::<crate::UiTooltip>(crate::UiTooltip::NAME)
            .with_template();
        registry
            .register::<crate::UiTabs>(crate::UiTabs::NAME)
            .with_template();
        registry
            .register::<crate::UiTab>(crate::UiTab::NAME)
            .with_template();
        registry
            .register::<crate::UiHidden>(crate::UiHidden::NAME)
            .with_template();
        registry
            .register::<crate::UiOwnStyle>(crate::UiOwnStyle::NAME)
            .with_template();
        registry
            .register::<Persistent>(Persistent::NAME)
            .with_template();
        registry
            .register::<crate::StreamingAnchor>(crate::StreamingAnchor::NAME)
            .with_template();
        registry
            .register::<Parallax>(Parallax::NAME)
            .with_template();
        registry
            .register::<ParticleEmitter>(ParticleEmitter::NAME)
            .with_template();
        registry
            .register::<PointLight2D>(PointLight2D::NAME)
            .with_template();
        registry
            .register::<GlobalLight2D>(GlobalLight2D::NAME)
            .with_template();
        registry
            .register_with_refs::<crate::Joint2D>(crate::Joint2D::NAME)
            .with_template();
        registry
            .register::<ShadowCaster2D>(ShadowCaster2D::NAME)
            .with_template();
        registry
            .register::<crate::Line2D>(crate::Line2D::NAME)
            .with_template();
        registry
            .register::<crate::Polygon2D>(crate::Polygon2D::NAME)
            .with_template();
        registry
            .register::<crate::Trail2D>(crate::Trail2D::NAME)
            .with_template();
        registry
            .register::<crate::Skeleton2D>(crate::Skeleton2D::NAME)
            .with_template();
        registry
            .register::<crate::Bone2D>(crate::Bone2D::NAME)
            .with_template();
        registry
            .register::<crate::TwoBoneIk>(crate::TwoBoneIk::NAME)
            .with_template();
        registry
            .register::<crate::SkinnedSprite>(crate::SkinnedSprite::NAME)
            .with_template();
        registry
    }
}
