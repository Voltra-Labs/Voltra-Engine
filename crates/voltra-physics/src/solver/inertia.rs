//! How hard a shape is to turn about its body's origin, per unit of mass.
//!
//! In world units, so a scaled collider resists exactly as much as it looks
//! like it should, and about the transform's origin — which is where every
//! body's mass sits — so a polygon drawn off its origin turns heavier, by the
//! parallel axis theorem, as it would pinned there.

use voltra_render::glam::Vec2;
use voltra_scene::{Collider2D, Transform2D};

use crate::geometry::Hull;

/// `I / m` for `collider` placed by `transform`. Zero when the shape has no
/// extent to turn.
pub(super) fn unit_inertia(collider: &Collider2D, transform: &Transform2D) -> f32 {
    match collider {
        // `m(hx² + hy²)/3` and `m·r²/2`, the standard two.
        Collider2D::Box { .. } => {
            let half = collider.world_half_extents(transform);
            (half.x * half.x + half.y * half.y) / 3.0
        }
        Collider2D::Circle { .. } => {
            let radius = collider.world_radius(transform);
            radius * radius / 2.0
        }
        Collider2D::Capsule { .. } => {
            let hull = Hull::of(collider, transform);
            let length = match *hull.vertices() {
                [low, high] => low.distance(high),
                _ => 0.0,
            };
            capsule(length, hull.radius)
        }
        Collider2D::Polygon { .. } => {
            let hull = Hull::of(collider, transform);
            let around: Vec<Vec2> = hull
                .vertices()
                .iter()
                .map(|v| *v - transform.translation)
                .collect();
            polygon(&around, hull.normals(), hull.radius)
        }
    }
}

/// Box2D's `b2ComputeCapsuleMass`: the rectangle between the caps, and the
/// two half discs each turned about the middle.
fn capsule(length: f32, radius: f32) -> f32 {
    let square = radius * radius;
    let box_area = 2.0 * radius * length;
    let disc_area = std::f32::consts::PI * square;
    let area = box_area + disc_area;
    if area <= 0.0 {
        return 0.0;
    }
    let reach = 0.5 * length;
    // How far a half disc's centroid sits from its flat side.
    let centroid = 4.0 * radius / (3.0 * std::f32::consts::PI);
    let box_inertia = box_area * (4.0 * square + length * length) / 12.0;
    let disc_inertia = disc_area * (0.5 * square + reach * reach + 2.0 * reach * centroid);
    (box_inertia + disc_inertia) / area
}

/// Box2D's `b2ComputePolygonMass`, about the origin of `vertices`: a fan of
/// triangles from the first corner. A rounded polygon is approximated, as
/// there, by pushing each corner out along its bisector.
fn polygon(vertices: &[Vec2], normals: &[Vec2], radius: f32) -> f32 {
    let n = vertices.len();
    let pushed: Vec<Vec2> = (0..n)
        .map(|i| {
            if radius <= 0.0 {
                return vertices[i];
            }
            let bisector = (normals[(i + n - 1) % n] + normals[i]).normalize_or_zero();
            vertices[i] + bisector * (std::f32::consts::SQRT_2 * radius)
        })
        .collect();
    let origin = pushed[0];
    let (mut area, mut centre, mut inertia) = (0.0, Vec2::ZERO, 0.0);
    for i in 1..n.saturating_sub(1) {
        let (e1, e2) = (pushed[i] - origin, pushed[i + 1] - origin);
        let d = e1.perp_dot(e2);
        let triangle = 0.5 * d;
        area += triangle;
        centre += (e1 + e2) * (triangle / 3.0);
        let along_x = e1.x * e1.x + e2.x * e1.x + e2.x * e2.x;
        let along_y = e1.y * e1.y + e2.y * e1.y + e2.y * e2.y;
        inertia += (0.25 / 3.0 * d) * (along_x + along_y);
    }
    if area <= 0.0 {
        return 0.0;
    }
    let centre = centre / area;
    // About the first corner, then about the centroid, then about the origin.
    let about_origin =
        inertia + area * ((origin + centre).length_squared() - centre.length_squared());
    about_origin / area
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::ConvexPolygon;

    #[test]
    fn a_square_polygon_turns_like_a_box() {
        let half = Vec2::new(1.0, 2.0);
        let as_box = unit_inertia(
            &Collider2D::Box { half_extents: half },
            &Transform2D::default(),
        );
        let as_polygon = unit_inertia(
            &Collider2D::Polygon {
                points: ConvexPolygon::rectangle(half).expect("a box"),
                radius: 0.0,
            },
            &Transform2D::default(),
        );
        assert!((as_box - as_polygon).abs() < 1e-4, "{as_box} {as_polygon}");
    }

    #[test]
    fn a_polygon_off_its_origin_turns_heavier_by_the_parallel_axis() {
        let half = Vec2::ONE;
        let shifted: Vec<Vec2> = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
            .map(|(x, y)| Vec2::new(x + 3.0, y))
            .to_vec();
        let centred = unit_inertia(
            &Collider2D::Box { half_extents: half },
            &Transform2D::default(),
        );
        let off = unit_inertia(
            &Collider2D::Polygon {
                points: ConvexPolygon::new(&shifted).expect("a box"),
                radius: 0.0,
            },
            &Transform2D::default(),
        );
        assert!((off - (centred + 9.0)).abs() < 1e-3, "{off}");
    }

    #[test]
    fn a_capsule_with_no_length_turns_like_its_circle() {
        assert!((capsule(0.0, 2.0) - 2.0).abs() < 1e-5);
        // Longer is harder to turn.
        assert!(capsule(4.0, 0.5) > capsule(1.0, 0.5));
    }
}
