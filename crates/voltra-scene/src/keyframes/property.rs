//! The component fields a clip can animate.

use serde::{Deserialize, Serialize};
use voltra_ecs::{Entity, World};

use super::blend::Mixing;
use super::value::{Value, ValueKind};
use crate::{GlobalLight2D, PointLight2D, Sprite2D, Text, Tilemap, Transform2D, Transform3D};

/// One animatable field.
///
/// A closed list rather than a reflected path string ("Sprite.color"):
/// this engine has no reflection, and a typed list is checked when the clip
/// loads instead of each time it samples. A field joins by a variant, its
/// kind, and its two arms in [`read`](Self::read) and
/// [`write`](Self::write) — Godot's property tracks with the property
/// list spelled out.
///
/// A variant is named by what it writes, with its dimension where the other
/// dimension gets one of its own (ARCHITECTURE, "Naming"). A renamed one
/// keeps its old name as a `serde` alias, so a clip written before still
/// reads; a save writes the new name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Property {
    /// `Transform2D::translation`, in the parent's space.
    #[serde(alias = "Translation")]
    Translation2D,
    /// `Transform2D::rotation`, radians. Not wrapped: keys at `0` and `2π`
    /// are a whole turn, which is what an author keying a spin means.
    #[serde(alias = "Rotation")]
    Rotation2D,
    /// `Transform2D::scale`.
    #[serde(alias = "Scale")]
    Scale2D,
    /// `Transform3D::translation`, in the parent's space.
    Translation3D,
    /// `Transform3D::rotation`, a unit quaternion. Between two keys it turns
    /// the short way round (less than half a turn per segment); in a mix,
    /// by `slerp`; in an additive layer, by composing rotations.
    Rotation3D,
    /// `Transform3D::scale`.
    Scale3D,
    /// `Sprite2D::color`.
    SpriteColor,
    /// `Sprite2D::frame`, an index into its atlas.
    SpriteFrame,
    /// `Text::color`.
    TextColor,
    /// `PointLight2D::color`; the keys' alpha is ignored.
    #[serde(alias = "LightColor")]
    PointLight2DColor,
    /// `PointLight2D::intensity`.
    #[serde(alias = "LightIntensity")]
    PointLight2DIntensity,
    /// `PointLight2D::radius`.
    #[serde(alias = "LightRadius")]
    PointLight2DRadius,
    /// `GlobalLight2D::color`; the keys' alpha is ignored.
    GlobalLightColor,
    /// `GlobalLight2D::intensity`.
    GlobalLightIntensity,
    /// `Tilemap::color`.
    TilemapColor,
}

impl Property {
    /// Every property, in the order an editor lists them.
    pub const ALL: [Self; 15] = [
        Self::Translation2D,
        Self::Rotation2D,
        Self::Scale2D,
        Self::Translation3D,
        Self::Rotation3D,
        Self::Scale3D,
        Self::SpriteColor,
        Self::SpriteFrame,
        Self::TextColor,
        Self::PointLight2DColor,
        Self::PointLight2DIntensity,
        Self::PointLight2DRadius,
        Self::GlobalLightColor,
        Self::GlobalLightIntensity,
        Self::TilemapColor,
    ];

    /// The kind of value its keys hold.
    pub fn kind(self) -> ValueKind {
        match self {
            Self::Translation2D | Self::Scale2D => ValueKind::Vec2,
            Self::Rotation2D
            | Self::PointLight2DIntensity
            | Self::PointLight2DRadius
            | Self::GlobalLightIntensity => ValueKind::Float,
            Self::SpriteColor
            | Self::TextColor
            | Self::PointLight2DColor
            | Self::GlobalLightColor
            | Self::TilemapColor => ValueKind::Color,
            Self::SpriteFrame => ValueKind::Int,
            Self::Translation3D | Self::Scale3D => ValueKind::Vec3,
            Self::Rotation3D => ValueKind::Quat,
        }
    }

    /// How two clips' values of it mix in a blend: a rotation the short way
    /// round (a 2D angle as a number, a 3D one by `slerp`), a frame by
    /// switching, everything else linearly.
    ///
    /// Not the same as how one clip moves between its own keys: a rotation
    /// keyed `0` then `2π` is a whole turn (see [`Rotation2D`](Self::Rotation2D)),
    /// but two clips holding those angles mean one pose.
    pub fn mixing(self) -> Mixing {
        match (self, self.kind()) {
            (Self::Rotation2D, _) => Mixing::Angle,
            (_, ValueKind::Quat) => Mixing::Quaternion,
            (_, ValueKind::Int) => Mixing::Discrete,
            _ => Mixing::Linear,
        }
    }

    /// What an editor calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Translation2D => "position",
            Self::Rotation2D => "rotation",
            Self::Scale2D => "scale",
            Self::Translation3D => "3D position",
            Self::Rotation3D => "3D rotation",
            Self::Scale3D => "3D scale",
            Self::SpriteColor => "sprite colour",
            Self::SpriteFrame => "sprite frame",
            Self::TextColor => "text colour",
            Self::PointLight2DColor => "light colour",
            Self::PointLight2DIntensity => "light intensity",
            Self::PointLight2DRadius => "light radius",
            Self::GlobalLightColor => "global light colour",
            Self::GlobalLightIntensity => "global light intensity",
            Self::TilemapColor => "tilemap colour",
        }
    }

    /// Its value on `entity` now, or `None` without the component.
    pub fn read(self, world: &World, entity: Entity) -> Option<Value> {
        let rgb = |c: [f32; 3]| Value::Color([c[0], c[1], c[2], 1.0]);
        Some(match self {
            Self::Translation2D => Value::Vec2(world.get::<Transform2D>(entity)?.translation),
            Self::Rotation2D => Value::Float(world.get::<Transform2D>(entity)?.rotation),
            Self::Scale2D => Value::Vec2(world.get::<Transform2D>(entity)?.scale),
            Self::Translation3D => Value::Vec3(world.get::<Transform3D>(entity)?.translation),
            Self::Rotation3D => Value::Quat(world.get::<Transform3D>(entity)?.rotation),
            Self::Scale3D => Value::Vec3(world.get::<Transform3D>(entity)?.scale),
            Self::SpriteColor => Value::Color(world.get::<Sprite2D>(entity)?.color),
            Self::SpriteFrame => Value::Int(i64::from(world.get::<Sprite2D>(entity)?.frame)),
            Self::TextColor => Value::Color(world.get::<Text>(entity)?.color),
            Self::PointLight2DColor => rgb(world.get::<PointLight2D>(entity)?.color),
            Self::PointLight2DIntensity => {
                Value::Float(world.get::<PointLight2D>(entity)?.intensity)
            }
            Self::PointLight2DRadius => Value::Float(world.get::<PointLight2D>(entity)?.radius),
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
    /// checked by an inspector's range. A 3D rotation is written unit length
    /// ([`Value::usable`]); one with no usable length (zero, or so long
    /// its square overflows) is refused, `false`.
    pub fn write(self, world: &mut World, entity: Entity, value: Value) -> bool {
        let written = match (self, value) {
            (Self::Translation2D, Value::Vec2(v)) => world
                .get_mut::<Transform2D>(entity)
                .map(|t| t.translation = v),
            (Self::Rotation2D, Value::Float(v)) => {
                world.get_mut::<Transform2D>(entity).map(|t| t.rotation = v)
            }
            (Self::Scale2D, Value::Vec2(v)) => {
                world.get_mut::<Transform2D>(entity).map(|t| t.scale = v)
            }
            (Self::Translation3D, Value::Vec3(v)) => world
                .get_mut::<Transform3D>(entity)
                .map(|t| t.translation = v),
            (Self::Rotation3D, Value::Quat(q)) => match Value::Quat(q).usable() {
                Some(Value::Quat(q)) => {
                    world.get_mut::<Transform3D>(entity).map(|t| t.rotation = q)
                }
                _ => None,
            },
            (Self::Scale3D, Value::Vec3(v)) => {
                world.get_mut::<Transform3D>(entity).map(|t| t.scale = v)
            }
            (Self::SpriteColor, Value::Color(c)) => {
                world.get_mut::<Sprite2D>(entity).map(|s| s.color = c)
            }
            (Self::SpriteFrame, Value::Int(i)) => world
                .get_mut::<Sprite2D>(entity)
                .map(|s| s.frame = i.clamp(0, i64::from(u32::MAX)) as u32),
            (Self::TextColor, Value::Color(c)) => {
                world.get_mut::<Text>(entity).map(|t| t.color = c)
            }
            (Self::PointLight2DColor, Value::Color(c)) => world
                .get_mut::<PointLight2D>(entity)
                .map(|l| l.color = [c[0], c[1], c[2]]),
            (Self::PointLight2DIntensity, Value::Float(v)) => world
                .get_mut::<PointLight2D>(entity)
                .map(|l| l.intensity = v.max(0.0)),
            (Self::PointLight2DRadius, Value::Float(v)) => world
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
mod tests;
