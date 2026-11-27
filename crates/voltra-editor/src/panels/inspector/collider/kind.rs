//! Which kind a collider is, and turning one kind into another at about
//! the same size.

use voltra_render::glam::Vec2;
use voltra_scene::{Collider2D, ConvexPolygon};

/// The kinds a collider can be switched between, in the order offered.
pub(super) const KINDS: [&str; 4] = ["Box", "Circle", "Capsule", "Polygon"];

/// `collider` as a shape of `kind`, covering about what it covered.
pub(super) fn converted(collider: &Collider2D, kind: &str) -> Collider2D {
    let extent = match collider {
        Collider2D::Box { half_extents } => *half_extents,
        Collider2D::Circle { radius } => Vec2::splat(*radius),
        Collider2D::Capsule {
            half_height,
            radius,
        } => Vec2::new(*radius, half_height.max(*radius)),
        Collider2D::Polygon { points, radius } => {
            points
                .points()
                .iter()
                .fold(Vec2::ZERO, |most, p| most.max(p.abs()))
                + Vec2::splat(*radius)
        }
    };
    let extent = if extent.x > 0.0 && extent.y > 0.0 && extent.is_finite() {
        extent
    } else {
        Vec2::splat(voltra_scene::Sprite2D::HALF_EXTENT)
    };
    match kind {
        "Circle" => Collider2D::Circle {
            radius: extent.max_element(),
        },
        "Capsule" => Collider2D::Capsule {
            half_height: extent.y.max(extent.x),
            radius: extent.x,
        },
        "Polygon" => match ConvexPolygon::rectangle(extent) {
            Ok(points) => Collider2D::Polygon {
                points,
                radius: 0.0,
            },
            Err(_) => Collider2D::default(),
        },
        _ => Collider2D::Box {
            half_extents: extent,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_kind_keeps_about_the_same_size() {
        let wide = Collider2D::Box {
            half_extents: Vec2::new(1.0, 2.0),
        };
        assert_eq!(
            converted(&wide, "Capsule"),
            Collider2D::Capsule {
                half_height: 2.0,
                radius: 1.0
            }
        );
        assert_eq!(
            converted(&wide, "Circle"),
            Collider2D::Circle { radius: 2.0 }
        );
        let polygon = converted(&wide, "Polygon");
        assert_eq!(converted(&polygon, "Box"), wide);
        assert_eq!(polygon.kind(), "Polygon");
    }

    #[test]
    fn a_collider_of_no_size_converts_to_a_sprites_size() {
        let nothing = Collider2D::Circle { radius: 0.0 };
        assert_eq!(converted(&nothing, "Box"), Collider2D::default());
    }
}
