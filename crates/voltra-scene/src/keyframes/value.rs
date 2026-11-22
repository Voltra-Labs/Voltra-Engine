//! What a key holds.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

/// One key's value. Which variant a track takes is its property's
/// [`ValueKind`]; a key of another kind is dropped when the clip loads.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Float(f32),
    Vec2(Vec2),
    /// RGBA, each `0..=1` — the components' own colour fields, so a key
    /// holds what the inspector shows. A light's colour has no alpha; its
    /// keys' fourth channel is ignored.
    Color([f32; 4]),
    /// A whole number: a sprite's frame.
    Int(i64),
}

/// Which [`Value`] variant a property takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Float,
    Vec2,
    Color,
    Int,
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Float(_) => ValueKind::Float,
            Self::Vec2(_) => ValueKind::Vec2,
            Self::Color(_) => ValueKind::Color,
            Self::Int(_) => ValueKind::Int,
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
        }
    }

    /// `self` moved `t` of the way to `to`.
    ///
    /// A whole number counts through the ones between, truncated towards
    /// `self` — a frame keyed `0` and then `7` a second later is a flipbook
    /// at seven frames a second, each shown for its full share. Values of two
    /// kinds answer `self`: a clip's load drops such keys, so only a clip
    /// edited into that state reaches here, and holding beats a panic.
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
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
