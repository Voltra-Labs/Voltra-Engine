//! Where a joint's anchor is: the one rule the solver, the overlay and the
//! editor share.

use voltra_render::glam::Vec2;

use super::Joint2D;
use crate::Transform2D;

impl Joint2D {
    /// Where `anchor`, in the space of a body placed at `transform`, is in
    /// the world: turned with the body and moved with it, **not scaled**.
    /// A body's scale sizes what is drawn and its collider; the solver
    /// turns an anchor only by the body's rotation, so a hinge stays where
    /// it was put however the sprite is stretched.
    pub fn anchor_in_world(transform: &Transform2D, anchor: Vec2) -> Vec2 {
        transform.translation + Vec2::from_angle(transform.rotation).rotate(anchor)
    }

    /// The anchor, in the space of a body placed at `transform`, that is at
    /// `point` in the world: [`anchor_in_world`](Self::anchor_in_world)
    /// undone.
    pub fn anchor_on(transform: &Transform2D, point: Vec2) -> Vec2 {
        Vec2::from_angle(-transform.rotation).rotate(point - transform.translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_anchor_turns_and_moves_with_its_body_but_does_not_scale() {
        let placed = Transform2D {
            translation: Vec2::new(2.0, 1.0),
            rotation: std::f32::consts::FRAC_PI_2,
            scale: Vec2::splat(3.0),
        };
        let world = Joint2D::anchor_in_world(&placed, Vec2::new(1.0, 0.0));
        assert!((world - Vec2::new(2.0, 2.0)).length() < 1e-5, "{world}");
        let back = Joint2D::anchor_on(&placed, world);
        assert!((back - Vec2::new(1.0, 0.0)).length() < 1e-5, "{back}");
    }
}
