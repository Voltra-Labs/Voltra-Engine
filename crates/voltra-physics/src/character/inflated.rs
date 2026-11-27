//! The shape a character is cast as: its collider grown by the skin.

use voltra_render::glam::Vec2;
use voltra_scene::{Collider2D, Transform2D};

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
    collider: &Collider2D,
    transform: &Transform2D,
    skin: f32,
) -> (Collider2D, Transform2D) {
    let unit = Transform2D {
        scale: Vec2::ONE,
        ..*transform
    };
    match collider {
        Collider2D::Box { .. } => (
            Collider2D::Box {
                half_extents: collider.world_half_extents(transform) + Vec2::splat(skin),
            },
            unit,
        ),
        Collider2D::Circle { .. } => (
            Collider2D::Circle {
                radius: collider.world_radius(transform) + skin,
            },
            unit,
        ),
        Collider2D::Capsule { .. } => {
            let shape = collider.world_shape(transform);
            let reach = match *shape.vertices() {
                [low, high] => (high - low).length() * 0.5,
                _ => 0.0,
            };
            let radius = shape.radius() + skin;
            (
                Collider2D::Capsule {
                    half_height: reach + radius,
                    radius,
                },
                unit,
            )
        }
        // The points keep their scale, so the rounding is set in the units
        // the scale will turn into world ones.
        Collider2D::Polygon { points, .. } => {
            let scale = transform.scale.abs().max_element();
            (
                Collider2D::Polygon {
                    points: *points,
                    radius: (collider.world_radius(transform) + skin) / scale,
                },
                *transform,
            )
        }
    }
}
