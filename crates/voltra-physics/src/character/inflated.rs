//! The shape a character is cast as: its collider grown by the skin.

use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Transform};

/// The collider grown by `skin` all round, in world units, with the
/// transform it is cast from: the entity's place and turn.
///
/// Cast instead of the collider, it keeps the collider a skin from every
/// surface along that surface's normal — exactly, whatever the angle of
/// the move — and a collider that ends up nearer than the skin overlaps it,
/// and is pushed back out. A circle, a capsule and a polygon grow round, by
/// their radius, which is exactly that; a box grows square, so its corners
/// stand a little further off, and a box character stands on a ledge's very
/// edge as it always has.
pub(super) fn inflated(
    collider: &Collider,
    transform: &Transform,
    skin: f32,
) -> (Collider, Transform) {
    let unit = Transform {
        scale: Vec2::ONE,
        ..*transform
    };
    match collider {
        Collider::Box { .. } => (
            Collider::Box {
                half_extents: collider.world_half_extents(transform) + Vec2::splat(skin),
            },
            unit,
        ),
        Collider::Circle { .. } => (
            Collider::Circle {
                radius: collider.world_radius(transform) + skin,
            },
            unit,
        ),
        Collider::Capsule { .. } => {
            let shape = collider.world_shape(transform);
            let reach = match *shape.vertices() {
                [low, high] => (high - low).length() * 0.5,
                _ => 0.0,
            };
            let radius = shape.radius() + skin;
            (
                Collider::Capsule {
                    half_height: reach + radius,
                    radius,
                },
                unit,
            )
        }
        // The points keep their scale, so the rounding is set in the units
        // the scale will turn into world ones.
        Collider::Polygon { points, .. } => {
            let scale = transform.scale.abs().max_element();
            (
                Collider::Polygon {
                    points: *points,
                    radius: (collider.world_radius(transform) + skin) / scale,
                },
                *transform,
            )
        }
    }
}
