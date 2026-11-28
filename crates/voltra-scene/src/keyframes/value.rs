//! What a key holds.

use serde::{Deserialize, Serialize};
use voltra_render::glam::{Quat, Vec2, Vec3};

use crate::Interpolate;

/// One key's value. Which variant a track takes is its property's
/// [`ValueKind`]; a key of another kind is dropped when the clip loads.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Value {
    /// A single number.
    Float(f32),
    /// A 2D vector.
    Vec2(Vec2),
    /// RGBA, each `0..=1` — the components' own colour fields, so a key
    /// holds what the inspector shows. A light's colour has no alpha; its
    /// keys' fourth channel is ignored.
    Color([f32; 4]),
    /// A whole number: a sprite's frame.
    Int(i64),
    /// A 3D vector: a 3D position or scale.
    Vec3(Vec3),
    /// A 3D rotation, `(x, y, z, w)` as a [`Transform3D`](crate::Transform3D)
    /// stores it. A clip's load makes a key of it unit length (a hand-typed
    /// `(0.0, 0.7, 0.0, 0.7)` is near unit, not unit) and drops one of zero
    /// length, which is no rotation at all.
    Quat(Quat),
}

/// Which [`Value`] variant a property takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    /// A single number.
    Float,
    /// A 2D vector.
    Vec2,
    /// An RGBA colour.
    Color,
    /// A whole number.
    Int,
    /// A 3D vector.
    Vec3,
    /// A 3D rotation.
    Quat,
}

impl Value {
    /// Which kind of value this is.
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Float(_) => ValueKind::Float,
            Self::Vec2(_) => ValueKind::Vec2,
            Self::Color(_) => ValueKind::Color,
            Self::Int(_) => ValueKind::Int,
            Self::Vec3(_) => ValueKind::Vec3,
            Self::Quat(_) => ValueKind::Quat,
        }
    }

    /// Whether every number in it is finite. A NaN in a key would spread
    /// to every value sampled near it, and from a transform to the whole
    /// subtree's.
    pub fn is_finite(&self) -> bool {
        match self {
            Self::Float(v) => v.is_finite(),
            Self::Vec2(v) => v.is_finite(),
            Self::Color(c) => c.iter().all(|v| v.is_finite()),
            Self::Int(_) => true,
            Self::Vec3(v) => v.is_finite(),
            Self::Quat(q) => q.is_finite(),
        }
    }

    /// The value as a sampler can trust it, or `None` for one it cannot:
    /// a number that is not finite, or a rotation whose squared length is
    /// not a normal `f32` — zero, subnormal, or past `f32::MAX` (a
    /// component near 10²⁰ is finite but normalises to zero). A
    /// rotation off unit length by more than glam's `is_normalized`
    /// tolerance is normalised; one within it keeps its bits, so a clip this
    /// build wrote reads back exactly.
    pub fn usable(self) -> Option<Self> {
        if !self.is_finite() {
            return None;
        }
        match self {
            Self::Quat(q) if !q.length_squared().is_normal() => None,
            Self::Quat(q) if !q.is_normalized() => Some(Self::Quat(q.normalize())),
            value => Some(value),
        }
    }

    /// `self` moved `t` of the way to `to`.
    ///
    /// A whole number counts through the ones between, truncated towards
    /// `self` — a frame keyed `0` and then `7` a second later is a flipbook
    /// at seven frames a second, each shown for its full share. Values of two
    /// kinds answer `self`: a clip's load drops such keys, so only a clip
    /// edited into that state reaches here, and holding beats a panic.
    ///
    /// A rotation turns the short way round, exactly `self` at `t = 0` and
    /// `to` at `t = 1` ([`Interpolate`] for `Quat`): one segment of a clip
    /// turns less than half a turn, so a spin is keyed every quarter or so
    /// — glTF's rule for its rotation channels too. Unlike a 2D angle, a
    /// quaternion cannot say "a whole turn".
    pub fn lerp(self, to: Self, t: f32) -> Self {
        match (self, to) {
            (Self::Float(a), Self::Float(b)) => Self::Float(a + (b - a) * t),
            (Self::Vec2(a), Self::Vec2(b)) => Self::Vec2(a.lerp(b, t)),
            (Self::Color(a), Self::Color(b)) => {
                Self::Color(std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t))
            }
            (Self::Int(a), Self::Int(b)) => {
                let step = ((b - a) as f64 * f64::from(t)).trunc() as i64;
                Self::Int(a.saturating_add(step))
            }
            (Self::Vec3(a), Self::Vec3(b)) => Self::Vec3(a + (b - a) * t),
            (Self::Quat(a), Self::Quat(b)) => Self::Quat(a.interpolate(&b, t)),
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_testkit::rotation_apart;

    #[test]
    fn whole_numbers_count_through_the_ones_between() {
        let (a, b) = (Value::Int(0), Value::Int(7));
        assert_eq!(a.lerp(b, 0.0), Value::Int(0));
        assert_eq!(a.lerp(b, 0.15), Value::Int(1));
        assert_eq!(a.lerp(b, 0.99), Value::Int(6), "the last gets its share");
        assert_eq!(
            b.lerp(a, 0.1),
            Value::Int(7),
            "down, truncated towards the start"
        );
        assert_eq!(
            Value::Vec2(Vec2::ZERO).lerp(Value::Vec2(Vec2::new(2.0, 4.0)), 0.5),
            Value::Vec2(Vec2::new(1.0, 2.0))
        );
        assert_eq!(
            Value::Float(1.0).lerp(Value::Int(3), 0.5),
            Value::Float(1.0)
        );
    }

    #[test]
    fn a_3d_rotation_key_turns_the_short_way_and_lands_on_its_keys() {
        let (a, b) = (
            Quat::from_rotation_z(-100f32.to_radians()),
            Quat::from_rotation_z(100f32.to_radians()),
        );
        let Value::Quat(half) = Value::Quat(a).lerp(Value::Quat(b), 0.5) else {
            panic!("a rotation");
        };
        let through = rotation_apart(half, Quat::from_rotation_z(180f32.to_radians()));
        assert!(through < 1e-4, "through 180°: {through} rad off");
        assert_eq!(Value::Quat(a).lerp(Value::Quat(b), 0.0), Value::Quat(a));
        assert_eq!(Value::Quat(a).lerp(Value::Quat(b), 1.0), Value::Quat(b));
        assert_eq!(
            Value::Vec3(Vec3::ZERO).lerp(Value::Vec3(Vec3::new(2.0, 4.0, -6.0)), 0.5),
            Value::Vec3(Vec3::new(1.0, 2.0, -3.0))
        );
    }

    #[test]
    fn a_rotation_typed_by_hand_is_made_unit_and_a_zero_one_dropped() {
        let Some(Value::Quat(q)) = Value::Quat(Quat::from_xyzw(0.0, 0.7, 0.0, 0.7)).usable() else {
            panic!("kept");
        };
        assert!((q.length() - 1.0).abs() < 1e-6);
        let unit = Quat::from_rotation_y(0.3);
        assert_eq!(
            Value::Quat(unit).usable(),
            Some(Value::Quat(unit)),
            "bits kept"
        );
        assert_eq!(
            Value::Quat(Quat::from_xyzw(0.0, 0.0, 0.0, 0.0)).usable(),
            None
        );
        assert_eq!(
            Value::Quat(Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0)).usable(),
            None
        );
        assert_eq!(
            Value::Vec3(Vec3::new(1.0, f32::INFINITY, 0.0)).usable(),
            None
        );
        // Finite, but squared past f32 or below its normals: no length to
        // divide by.
        for (huge, tiny) in [(1.0e20, 1.0e-30), (3.0e19, 1.0e-20)] {
            let long = Quat::from_xyzw(0.0, huge, 0.0, huge);
            assert_eq!(Value::Quat(long).usable(), None, "{huge}");
            let short = Quat::from_xyzw(tiny, 0.0, 0.0, 0.0);
            assert_eq!(Value::Quat(short).usable(), None, "{tiny}");
        }
    }
}
