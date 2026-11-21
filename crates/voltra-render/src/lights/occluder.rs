//! Shapes that block light, and which of their edges cast a light's shadow.
//!
//! An occluder is an outline or an open line, in the world. Its edges are
//! prepared once a frame — sanitised, oriented, boxed — and then asked, per
//! shadowed light, for the ones that light reaches.
//!
//! **Which edges cast.** An open line casts from every edge, both sides. A
//! closed outline without [`self_shadow`](Occluder::self_shadow) casts only
//! from the edges facing *away* from the light, so its shadow starts at its
//! far side and the light reaches the shape itself — a wall's lit face stays
//! lit. With it, every edge casts and the shape is in its own shadow from the
//! near edge on. Unity's `ShadowCaster2D` "Self Shadows" and Godot's
//! `OccluderPolygon2D` cull mode, as one switch.

use std::ops::Range;

use glam::Vec2;

/// A shape that blocks light, in the world.
#[derive(Debug, Clone, PartialEq)]
pub struct Occluder {
    /// The outline's corners, or the line's points, in order.
    pub points: Vec<Vec2>,
    /// The last point joins the first: an outline with an inside, rather
    /// than an open line.
    pub closed: bool,
    /// The outline is shadowed by its own near side. Off, light reaches its
    /// inside and the shadow starts behind it. Meaningless for an open line,
    /// which has no inside.
    pub self_shadow: bool,
}

/// One edge, and the side of it that is out of the shape.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Edge {
    a: Vec2,
    b: Vec2,
    /// Away from the shape's inside, for an edge whose front is culled.
    /// `None` casts whichever side the light is on.
    culled_front: Option<Vec2>,
}

/// One occluder's box and where its edges are.
#[derive(Debug, Clone, PartialEq)]
struct Shape {
    min: Vec2,
    max: Vec2,
    edges: Range<usize>,
}

/// Every occluder's edges, prepared for asking which of them a light casts
/// from.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Edges {
    edges: Vec<Edge>,
    shapes: Vec<Shape>,
}

impl Edges {
    /// The edges of `occluders`. An occluder with a point that is not a
    /// number is left out whole — half an outline would cast a shadow with
    /// a hole in it — and so is one with fewer than two distinct points.
    pub(super) fn new(occluders: &[Occluder]) -> Self {
        let mut edges = Vec::new();
        let mut shapes = Vec::new();
        for occluder in occluders {
            if !occluder.points.iter().all(|p| p.is_finite()) {
                continue;
            }
            let mut points = occluder.points.clone();
            points.dedup();
            if occluder.closed && points.len() > 1 && points.first() == points.last() {
                points.pop();
            }
            if points.len() < 2 {
                continue;
            }
            // An outline needs an inside to cull against: three corners
            // that do not all lie on one line. Less than that is a line.
            let closed = occluder.closed && points.len() >= 3;
            let area = if closed {
                twice_signed_area(&points)
            } else {
                0.0
            };
            let cull = closed && !occluder.self_shadow && area != 0.0;

            let start = edges.len();
            let count = if closed {
                points.len()
            } else {
                points.len() - 1
            };
            for i in 0..count {
                let (a, b) = (points[i], points[(i + 1) % points.len()]);
                let along = b - a;
                // Counter-clockwise, the inside is on the left, so out is
                // on the right; clockwise, the other way round.
                let out = if area > 0.0 {
                    Vec2::new(along.y, -along.x)
                } else {
                    Vec2::new(-along.y, along.x)
                };
                edges.push(Edge {
                    a,
                    b,
                    culled_front: cull.then_some(out),
                });
            }
            let (min, max) = points.iter().fold((points[0], points[0]), |(lo, hi), &p| {
                (lo.min(p), hi.max(p))
            });
            shapes.push(Shape {
                min,
                max,
                edges: start..edges.len(),
            });
        }
        Self { edges, shapes }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// The edges that cast a shadow of a light at `centre` reaching
    /// `radius`: those that come within its reach and are not a culled
    /// front, as `(a, b)`.
    pub(super) fn casting(
        &self,
        centre: Vec2,
        radius: f32,
    ) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
        self.shapes
            .iter()
            .filter(move |shape| {
                let nearest = centre.clamp(shape.min, shape.max);
                nearest.distance_squared(centre) < radius * radius
            })
            .flat_map(move |shape| self.edges[shape.edges.clone()].iter())
            .filter(move |edge| {
                // Seen from behind — the light on the shape's side of the
                // edge's line — or not culled at all.
                let faces_away = edge
                    .culled_front
                    .is_none_or(|out| out.dot(centre - edge.a) < 0.0);
                faces_away && distance_to_segment(centre, edge.a, edge.b) < radius
            })
            .map(|edge| (edge.a, edge.b))
    }
}

/// Twice the signed area: positive counter-clockwise, negative clockwise,
/// zero when every point lies on one line.
fn twice_signed_area(points: &[Vec2]) -> f32 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.perp_dot(*b))
        .sum()
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let along = b - a;
    let length_squared = along.length_squared();
    let t = if length_squared > 0.0 {
        ((p - a).dot(along) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + along * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit box around `(3, 0)`, counter-clockwise or not.
    fn box_at_three(ccw: bool, self_shadow: bool) -> Occluder {
        let mut points = vec![
            Vec2::new(2.5, -0.5),
            Vec2::new(3.5, -0.5),
            Vec2::new(3.5, 0.5),
            Vec2::new(2.5, 0.5),
        ];
        if !ccw {
            points.reverse();
        }
        Occluder {
            points,
            closed: true,
            self_shadow,
        }
    }

    fn casting(occluders: &[Occluder], centre: Vec2, radius: f32) -> Vec<(Vec2, Vec2)> {
        Edges::new(occluders).casting(centre, radius).collect()
    }

    #[test]
    fn an_outline_casts_from_its_far_side_whichever_way_it_winds() {
        for ccw in [true, false] {
            let edges = casting(&[box_at_three(ccw, false)], Vec2::ZERO, 10.0);
            // The light is to the left: the right edge faces away, and the
            // top and bottom edges are seen side-on from inside their lines.
            let xs: Vec<f32> = edges.iter().map(|(a, b)| a.x.max(b.x)).collect();
            assert_eq!(edges.len(), 3, "ccw {ccw}: {edges:?}");
            assert!(
                !edges.iter().any(|(a, b)| a.x == 2.5 && b.x == 2.5),
                "the near edge faces the light and is culled: {edges:?}"
            );
            assert!(xs.contains(&3.5));
        }
    }

    #[test]
    fn a_self_shadowed_outline_casts_from_every_edge() {
        assert_eq!(
            casting(&[box_at_three(true, true)], Vec2::ZERO, 10.0).len(),
            4
        );
    }

    #[test]
    fn an_open_line_casts_from_both_sides() {
        let wall = Occluder {
            points: vec![Vec2::new(1.0, -1.0), Vec2::new(1.0, 1.0)],
            closed: false,
            self_shadow: false,
        };
        assert_eq!(
            casting(std::slice::from_ref(&wall), Vec2::ZERO, 5.0).len(),
            1
        );
        assert_eq!(casting(&[wall], Vec2::new(2.0, 0.0), 5.0).len(), 1);
    }

    #[test]
    fn edges_out_of_reach_cast_nothing() {
        let occluders = [box_at_three(true, true)];
        assert!(
            casting(&occluders, Vec2::ZERO, 2.4).is_empty(),
            "short of the box"
        );
        // Reaches the near edge only.
        assert_eq!(casting(&occluders, Vec2::ZERO, 2.6).len(), 3);
    }

    #[test]
    fn a_light_inside_an_outline_is_shut_in_by_every_edge() {
        let edges = casting(&[box_at_three(true, false)], Vec2::new(3.0, 0.0), 10.0);
        assert_eq!(edges.len(), 4, "every edge has the light on its inner side");
    }

    #[test]
    fn nonsense_and_degenerate_occluders_are_left_out() {
        let occluders = [
            Occluder {
                points: vec![Vec2::new(1.0, 0.0), Vec2::new(f32::NAN, 1.0)],
                closed: false,
                self_shadow: false,
            },
            Occluder {
                points: vec![Vec2::ONE, Vec2::ONE, Vec2::ONE],
                closed: true,
                self_shadow: false,
            },
            Occluder {
                points: Vec::new(),
                closed: true,
                self_shadow: false,
            },
        ];
        assert!(Edges::new(&occluders).is_empty());
    }

    #[test]
    fn a_flat_or_two_point_outline_is_a_line_that_casts_both_ways() {
        let flat = Occluder {
            points: vec![
                Vec2::new(1.0, -1.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
            ],
            closed: true,
            self_shadow: false,
        };
        // No inside to cull against: all three edges (two along the line and
        // the one back) cast.
        assert_eq!(casting(&[flat], Vec2::ZERO, 5.0).len(), 3);
        let pair = Occluder {
            points: vec![Vec2::new(1.0, -1.0), Vec2::new(1.0, 1.0)],
            closed: true,
            self_shadow: false,
        };
        assert_eq!(casting(&[pair], Vec2::ZERO, 5.0).len(), 1);
    }

    #[test]
    fn a_repeated_closing_point_is_not_an_edge() {
        let mut closed = box_at_three(true, true);
        closed.points.push(closed.points[0]);
        assert_eq!(casting(&[closed], Vec2::ZERO, 10.0).len(), 4);
    }
}
