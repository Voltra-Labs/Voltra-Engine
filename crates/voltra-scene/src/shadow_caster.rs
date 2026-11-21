//! What blocks 2D light: the shadow caster component.
//!
//! Unity's `ShadowCaster2D` and Godot's `LightOccluder2D`. By default the
//! shape is the entity's own [`Collider`](crate::Collider) — or, on a solid
//! tilemap layer, the boxes generated from its tiles — so a wall casts
//! where it blocks and the two cannot drift apart; a polygon of its own is
//! there for a shape that should shadow differently from how it collides.
//!
//! Its outlines in the world are built in `shadow_caster/outline.rs`, and
//! the polygon it starts from when given one in `shadow_caster/starting.rs`.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

mod outline;
mod starting;

pub use outline::{occluders, outlines_of, CIRCLE_SIDES};
pub use starting::{starting_polygon, EDIT_CIRCLE_SIDES};

/// Blocks the light of every [`PointLight2D`](crate::PointLight2D) that casts
/// shadows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShadowCaster2D {
    pub shape: CasterShape,
    /// Shadowed by its own near side. Off — the default — light reaches it
    /// and its shadow starts behind it: a wall's lit face stays lit.
    pub self_shadow: bool,
}

/// What a caster's outline is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum CasterShape {
    /// The entity's [`Collider`](crate::Collider), placed as the physics places it; on a
    /// solid [`Tilemap`](crate::Tilemap) layer, its generated boxes. Nothing, on an entity
    /// with neither.
    #[default]
    Collider,
    /// Points in the entity's own space, carried by its transform and its
    /// parents' like a sprite.
    Polygon {
        points: Vec<Vec2>,
        /// The last point joins the first. Open, it is a line that blocks
        /// light from both sides.
        closed: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caster_round_trips_through_ron_and_defaults_to_its_collider() {
        let caster: ShadowCaster2D = ron::from_str("()").expect("reads");
        assert_eq!(caster, ShadowCaster2D::default());
        let polygon = ShadowCaster2D {
            shape: CasterShape::Polygon {
                points: vec![Vec2::ZERO, Vec2::X, Vec2::Y],
                closed: true,
            },
            self_shadow: true,
        };
        let text = ron::to_string(&polygon).expect("writes");
        assert_eq!(
            ron::from_str::<ShadowCaster2D>(&text).expect("reads"),
            polygon
        );
    }
}
