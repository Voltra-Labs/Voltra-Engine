//! The component fields a clip can animate.

use serde::{Deserialize, Serialize};
use voltra_ecs::{Entity, World};

use super::value::{Value, ValueKind};
use crate::{GlobalLight2D, PointLight2D, Sprite, Text, Tilemap, Transform};

/// One animatable field.
///
/// A closed list rather than a reflected path string ("Sprite.color"):
/// this engine has no reflection, and a typed list is checked when the clip
/// loads instead of each time it samples. A field joins by a variant, its
/// kind, and its two arms in [`read`](Self::read) and
/// [`write`](Self::write) — Godot's property tracks with the property
/// list spelled out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Property {
    /// `Transform::translation`, in the parent's space.
    Translation,
    /// `Transform::rotation`, radians. Not wrapped: keys at `0` and `2π`
    /// are a whole turn, which is what an author keying a spin means.
    Rotation,
    /// `Transform::scale`.
    Scale,
    /// `Sprite::color`.
    SpriteColor,
    /// `Sprite::frame`, an index into its atlas.
    SpriteFrame,
    /// `Text::color`.
    TextColor,
    /// `PointLight2D::color`; the keys' alpha is ignored.
    LightColor,
    /// `PointLight2D::intensity`.
    LightIntensity,
    /// `PointLight2D::radius`.
    LightRadius,
    /// `GlobalLight2D::color`; the keys' alpha is ignored.
    GlobalLightColor,
    /// `GlobalLight2D::intensity`.
    GlobalLightIntensity,
    /// `Tilemap::color`.
    TilemapColor,
}

impl Property {
    /// Every property, in the order an editor lists them.
    pub const ALL: [Self; 12] = [
        Self::Translation,
        Self::Rotation,
        Self::Scale,
        Self::SpriteColor,
        Self::SpriteFrame,
        Self::TextColor,
        Self::LightColor,
        Self::LightIntensity,
        Self::LightRadius,
        Self::GlobalLightColor,
        Self::GlobalLightIntensity,
        Self::TilemapColor,
    ];

    /// The kind of value its keys hold.
    pub fn kind(self) -> ValueKind {
        match self {
            Self::Translation | Self::Scale => ValueKind::Vec2,
            Self::Rotation
            | Self::LightIntensity
            | Self::LightRadius
            | Self::GlobalLightIntensity => ValueKind::Float,
            Self::SpriteColor
            | Self::TextColor
            | Self::LightColor
            | Self::GlobalLightColor
            | Self::TilemapColor => ValueKind::Color,
            Self::SpriteFrame => ValueKind::Int,
        }
    }

    /// What an editor calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Translation => "position",
            Self::Rotation => "rotation",
            Self::Scale => "scale",
            Self::SpriteColor => "sprite colour",
            Self::SpriteFrame => "sprite frame",
            Self::TextColor => "text colour",
            Self::LightColor => "light colour",
            Self::LightIntensity => "light intensity",
            Self::LightRadius => "light radius",
            Self::GlobalLightColor => "global light colour",
            Self::GlobalLightIntensity => "global light intensity",
            Self::TilemapColor => "tilemap colour",
        }
    }

    /// Its value on `entity` now, or `None` without the component.
    pub fn read(self, world: &World, entity: Entity) -> Option<Value> {
        let rgb = |c: [f32; 3]| Value::Color([c[0], c[1], c[2], 1.0]);
        Some(match self {
            Self::Translation => Value::Vec2(world.get::<Transform>(entity)?.translation),
            Self::Rotation => Value::Float(world.get::<Transform>(entity)?.rotation),
            Self::Scale => Value::Vec2(world.get::<Transform>(entity)?.scale),
            Self::SpriteColor => Value::Color(world.get::<Sprite>(entity)?.color),
            Self::SpriteFrame => Value::Int(i64::from(world.get::<Sprite>(entity)?.frame)),
            Self::TextColor => Value::Color(world.get::<Text>(entity)?.color),
            Self::LightColor => rgb(world.get::<PointLight2D>(entity)?.color),
            Self::LightIntensity => Value::Float(world.get::<PointLight2D>(entity)?.intensity),
            Self::LightRadius => Value::Float(world.get::<PointLight2D>(entity)?.radius),
            Self::GlobalLightColor => rgb(world.get::<GlobalLight2D>(entity)?.color),
            Self::GlobalLightIntensity => {
                Value::Float(world.get::<GlobalLight2D>(entity)?.intensity)
            }
            Self::TilemapColor => Value::Color(world.get::<Tilemap>(entity)?.color),
        })
    }

    /// Sets it on `entity` to `value`. Answers whether it could: `false`
    /// without the component, or for a value of another kind.
    ///
    /// Clamped where the field has a floor — a light's intensity and radius
    /// at zero, a frame at zero — since a key typed into a file is not
    /// checked by an inspector's range.
    pub fn write(self, world: &mut World, entity: Entity, value: Value) -> bool {
        let written = match (self, value) {
            (Self::Translation, Value::Vec2(v)) => world
                .get_mut::<Transform>(entity)
                .map(|t| t.translation = v),
            (Self::Rotation, Value::Float(v)) => {
                world.get_mut::<Transform>(entity).map(|t| t.rotation = v)
            }
            (Self::Scale, Value::Vec2(v)) => {
                world.get_mut::<Transform>(entity).map(|t| t.scale = v)
            }
            (Self::SpriteColor, Value::Color(c)) => {
                world.get_mut::<Sprite>(entity).map(|s| s.color = c)
            }
            (Self::SpriteFrame, Value::Int(i)) => world
                .get_mut::<Sprite>(entity)
                .map(|s| s.frame = i.clamp(0, i64::from(u32::MAX)) as u32),
            (Self::TextColor, Value::Color(c)) => {
                world.get_mut::<Text>(entity).map(|t| t.color = c)
            }
            (Self::LightColor, Value::Color(c)) => world
                .get_mut::<PointLight2D>(entity)
                .map(|l| l.color = [c[0], c[1], c[2]]),
            (Self::LightIntensity, Value::Float(v)) => world
                .get_mut::<PointLight2D>(entity)
                .map(|l| l.intensity = v.max(0.0)),
            (Self::LightRadius, Value::Float(v)) => world
                .get_mut::<PointLight2D>(entity)
                .map(|l| l.radius = v.max(0.0)),
            (Self::GlobalLightColor, Value::Color(c)) => world
                .get_mut::<GlobalLight2D>(entity)
                .map(|l| l.color = [c[0], c[1], c[2]]),
            (Self::GlobalLightIntensity, Value::Float(v)) => world
                .get_mut::<GlobalLight2D>(entity)
                .map(|l| l.intensity = v.max(0.0)),
            (Self::TilemapColor, Value::Color(c)) => {
                world.get_mut::<Tilemap>(entity).map(|m| m.color = c)
            }
            _ => None,
        };
        written.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_property_reads_back_what_it_wrote() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(entity, Sprite::default());
        world.insert(entity, Text::default());
        world.insert(entity, PointLight2D::default());
        world.insert(entity, GlobalLight2D::default());
        world.insert(entity, Tilemap::default());
        for property in Property::ALL {
            let value = match property.kind() {
                ValueKind::Float => Value::Float(0.25),
                ValueKind::Vec2 => Value::Vec2(voltra_render::glam::Vec2::new(1.0, 2.0)),
                ValueKind::Color => Value::Color([0.1, 0.2, 0.3, 1.0]),
                ValueKind::Int => Value::Int(3),
            };
            assert!(property.write(&mut world, entity, value), "{property:?}");
            assert_eq!(property.read(&world, entity), Some(value), "{property:?}");
            assert!(
                !property.write(&mut world, entity, Value::Int(i64::MIN))
                    || property.kind() == ValueKind::Int,
                "{property:?} takes only its own kind"
            );
        }
        let bare = world.spawn();
        assert!(!Property::LightRadius.write(&mut world, bare, Value::Float(1.0)));
    }
}
