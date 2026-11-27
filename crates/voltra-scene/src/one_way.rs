//! A collider solid from one side only: a platform jumped up through and
//! landed on.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use crate::Transform2D;

/// Makes the entity's collider solid only to what meets it from
/// `direction`.
///
/// Unity's `PlatformEffector2D` and Godot's one-way collision. What meets
/// it from another side — or is already partly through it — passes until
/// it is clear: the physics step remembers a pair it let through, and a
/// shape cast ignores a collider it starts inside. Rays, points and box
/// overlaps still find it: they ask what is there, not what would collide.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OneWay {
    /// The side it is solid from, in the collider's own frame, so it turns
    /// with the entity. Up by default; one of no length is up.
    pub direction: Vec2,
    /// How far, in radians, a contact's normal may lean from `direction`
    /// and still be met — the corner of a platform is not its top.
    pub arc: f32,
}

impl Default for OneWay {
    fn default() -> Self {
        Self {
            direction: Vec2::Y,
            arc: std::f32::consts::FRAC_PI_4,
        }
    }
}

impl OneWay {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "OneWay";

    /// `direction` in world space, for a collider placed by `transform`.
    pub fn world_direction(&self, transform: &Transform2D) -> Vec2 {
        let local = self.direction.try_normalize().unwrap_or(Vec2::Y);
        Vec2::from_angle(transform.rotation).rotate(local)
    }

    /// Whether a contact whose normal, pushing the other shape away from
    /// this collider, is `normal` meets the solid side.
    pub fn admits(&self, transform: &Transform2D, normal: Vec2) -> bool {
        normal.dot(self.world_direction(transform))
            >= self.arc.clamp(0.0, std::f32::consts::PI).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_platform_admits_its_top_and_turns_with_its_entity() {
        let flat = Transform2D::default();
        let platform = OneWay::default();
        assert!(platform.admits(&flat, Vec2::Y));
        assert!(!platform.admits(&flat, Vec2::NEG_Y));
        assert!(!platform.admits(&flat, Vec2::X));
        let turned = Transform2D::default().with_rotation(std::f32::consts::FRAC_PI_2);
        assert!(platform.admits(&turned, Vec2::NEG_X));
        let back: OneWay = ron::from_str("()").expect("reads");
        assert_eq!(back, platform);
    }
}
