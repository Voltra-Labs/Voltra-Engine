//! Where a light reaches: its disc, or a spot light's sector of it.
//!
//! What the frame culls a light by (its box against the view), what the
//! light pass draws over (the same box, so a spot light aimed one way does
//! not shade the whole square around it), and what decides which edges a
//! shadowed light draws into its rows of the shadow map.

use glam::Vec2;

use super::PointLight;

/// Angle added to a cone's half-angle before it is used to cull, so a
/// direction exactly on its edge, rounded either way, stays in.
const EDGE_SLACK: f32 = 1e-3;

/// A disc, or the sector of one between two edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reach {
    centre: Vec2,
    radius: f32,
    /// A spot light's way: where it points, unit length, and the half-angle
    /// either side of that it reaches, below a half turn. `None` reaches
    /// every way.
    cone: Option<(Vec2, f32)>,
}

impl PointLight {
    /// Where the light reaches, or `None` for one that gives nothing. A spot
    /// light reaches its cone's outer edge, as the shader draws it.
    pub fn reach(&self) -> Option<Reach> {
        if !self.gives_light() {
            return None;
        }
        let cone = self
            .cone
            .and_then(|cone| cone.sanitized())
            .map(|(direction, outer, _)| (direction, outer.clamp(-1.0, 1.0).acos()));
        Some(
            Reach {
                centre: self.position,
                radius: self.radius,
                cone: None,
            }
            .with_cone(cone, 0.0),
        )
    }
}

impl Reach {
    /// `cone`, its half-angle opened by `wider` (and the slack); a cone
    /// that comes to a half turn or more reaches every way.
    fn with_cone(self, cone: Option<(Vec2, f32)>, wider: f32) -> Self {
        let cone = cone
            .map(|(direction, half)| (direction, half + wider + EDGE_SLACK))
            .filter(|(_, half)| *half < std::f32::consts::PI);
        Self { cone, ..self }
    }

    /// The same reach with a cone opened by `angle` radians either side —
    /// for a shadow whose soft edge looks that far past where it lights.
    pub(super) fn widened(self, angle: f32) -> Self {
        let half = self
            .cone
            .map(|(direction, half)| (direction, half - EDGE_SLACK));
        self.with_cone(half, angle.max(0.0))
    }

    /// The box around it, in the world: `(min, max)`.
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let Some((direction, half)) = self.cone else {
            return (
                self.centre - Vec2::splat(self.radius),
                self.centre + Vec2::splat(self.radius),
            );
        };
        // A sector's box holds its centre, the ends of its two edges, and
        // the farthest point of the disc along each axis it faces.
        let rotate = |angle: f32| Vec2::from_angle(angle).rotate(direction);
        let mut points = vec![
            self.centre,
            self.centre + rotate(half) * self.radius,
            self.centre + rotate(-half) * self.radius,
        ];
        for axis in [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y] {
            if self.faces(axis) {
                points.push(self.centre + axis * self.radius);
            }
        }
        points
            .iter()
            .fold((self.centre, self.centre), |(lo, hi), &p| {
                (lo.min(p), hi.max(p))
            })
    }

    /// The box as fractions of the radius around the centre, each axis
    /// within `-1..=1`: what the light pass draws its quad over.
    pub(super) fn local_bounds(&self) -> (Vec2, Vec2) {
        let (min, max) = self.bounds();
        let to_local = |p: Vec2| ((p - self.centre) / self.radius).clamp(Vec2::NEG_ONE, Vec2::ONE);
        (to_local(min), to_local(max))
    }

    /// Whether the way `offset` points from the centre is inside the cone.
    /// The centre itself, and every way for a disc, is.
    fn faces(&self, offset: Vec2) -> bool {
        match (self.cone, offset.try_normalize()) {
            (Some((direction, half)), Some(way)) => way.dot(direction) >= half.cos(),
            _ => true,
        }
    }

    fn contains(&self, point: Vec2) -> bool {
        point.distance_squared(self.centre) < self.radius * self.radius
            && self.faces(point - self.centre)
    }

    /// Whether the segment `a`–`b` comes into it.
    pub(super) fn touches_segment(&self, a: Vec2, b: Vec2) -> bool {
        if distance_to_segment(self.centre, a, b) >= self.radius {
            return false;
        }
        let Some((direction, half)) = self.cone else {
            return true;
        };
        if self.contains(a) || self.contains(b) {
            return true;
        }
        // Neither end is inside, so a segment that comes in crosses the
        // sector's outline: one of its two edges, or its arc.
        let rotate = |angle: f32| Vec2::from_angle(angle).rotate(direction);
        let crosses_edge = [half, -half].into_iter().any(|angle| {
            segments_meet(a, b, self.centre, self.centre + rotate(angle) * self.radius)
        });
        crosses_edge
            || circle_crossings(a, b, self.centre, self.radius)
                .into_iter()
                .flatten()
                .any(|point| self.faces(point - self.centre))
    }
}

/// How far `p` is from the segment `a`–`b`.
pub(super) fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let along = b - a;
    let length_squared = along.length_squared();
    let t = if length_squared > 0.0 {
        ((p - a).dot(along) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + along * t)
}

/// Whether the segments `a`–`b` and `c`–`d` meet, touching included.
fn segments_meet(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let side = |p: Vec2, q: Vec2, r: Vec2| (q - p).perp_dot(r - p);
    let (d1, d2) = (side(c, d, a), side(c, d, b));
    let (d3, d4) = (side(a, b, c), side(a, b, d));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    // Touching, or along one line: an end of one lies on the other.
    let on = |p: Vec2, q: Vec2, r: Vec2| distance_to_segment(r, p, q) <= 1e-6;
    on(c, d, a) || on(c, d, b) || on(a, b, c) || on(a, b, d)
}

/// Where the segment `a`–`b` crosses the circle around `centre`.
fn circle_crossings(a: Vec2, b: Vec2, centre: Vec2, radius: f32) -> [Option<Vec2>; 2] {
    let along = b - a;
    let from = a - centre;
    let qa = along.length_squared();
    let qb = 2.0 * from.dot(along);
    let qc = from.length_squared() - radius * radius;
    let discriminant = qb * qb - 4.0 * qa * qc;
    if qa <= 0.0 || discriminant < 0.0 {
        return [None, None];
    }
    let root = discriminant.sqrt();
    [(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)]
        .map(|t| (0.0..=1.0).contains(&t).then(|| a + along * t))
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use super::*;
    use crate::lights::{Cone, ALL_LIGHT_LAYERS};

    fn lamp(cone: Option<Cone>) -> PointLight {
        PointLight {
            position: Vec2::new(10.0, 20.0),
            color: [1.0; 3],
            intensity: 1.0,
            radius: 4.0,
            falloff: 2.0,
            shadow: None,
            cone,
            height: None,
            layers: ALL_LIGHT_LAYERS,
            cookie: None,
        }
    }

    /// A hard-edged cone `angle` across, pointing along `direction`.
    fn spot(direction: Vec2, angle: f32) -> PointLight {
        lamp(Some(Cone {
            direction,
            angle,
            softness: 0.0,
        }))
    }

    fn near(a: Vec2, b: Vec2) -> bool {
        a.distance(b) < 1e-2
    }

    #[test]
    fn a_point_light_reaches_its_square() {
        let (min, max) = lamp(None).reach().expect("gives light").bounds();
        assert_eq!((min, max), (Vec2::new(6.0, 16.0), Vec2::new(14.0, 24.0)));
        let full_turn = spot(Vec2::X, 2.0 * PI).reach().expect("gives light");
        assert_eq!(full_turn.bounds(), (min, max), "a full turn is a disc");
    }

    #[test]
    fn a_spot_light_reaches_only_its_side() {
        // A quarter turn pointing right: from the centre out to the radius,
        // and up and down to its edges at ±45°.
        let reach = spot(Vec2::X, FRAC_PI_2).reach().expect("gives light");
        let (min, max) = reach.bounds();
        let edge = 4.0 * std::f32::consts::FRAC_1_SQRT_2;
        assert!(near(min, Vec2::new(10.0, 20.0 - edge)), "{min}");
        assert!(near(max, Vec2::new(14.0, 20.0 + edge)), "{max}");
        // As fractions of its radius, for the quad.
        let (lo, hi) = reach.local_bounds();
        assert!(near(lo, Vec2::new(0.0, -0.707)) && near(hi, Vec2::new(1.0, 0.707)));
    }

    #[test]
    fn a_wide_spot_light_reaches_round_behind_its_centre() {
        // Three quarters of a turn pointing up: the left and right extremes
        // are in, and its edges reach down behind the centre.
        let (min, max) = spot(Vec2::Y, 1.5 * PI)
            .reach()
            .expect("gives light")
            .bounds();
        let edge = 4.0 * std::f32::consts::FRAC_1_SQRT_2;
        assert!(near(min, Vec2::new(6.0, 20.0 - edge)), "{min}");
        assert!(near(max, Vec2::new(14.0, 24.0)), "{max}");
    }

    #[test]
    fn a_turned_spot_light_turns_its_box() {
        let (min, max) = spot(Vec2::NEG_Y, 0.2)
            .reach()
            .expect("gives light")
            .bounds();
        assert!(max.y <= 20.0 + 1e-4 && min.y < 16.1, "{min} {max}");
        assert!(max.x - min.x < 1.0, "narrow across: {min} {max}");
    }

    #[test]
    fn a_light_that_gives_nothing_reaches_nowhere() {
        let dark = PointLight {
            intensity: 0.0,
            ..lamp(None)
        };
        assert_eq!(dark.reach(), None);
        assert_eq!(spot(Vec2::ZERO, 1.0).reach(), None);
    }

    #[test]
    fn a_segment_touches_a_disc_within_its_radius() {
        let reach = lamp(None).reach().expect("gives light");
        let c = Vec2::new(10.0, 20.0);
        assert!(reach.touches_segment(c + Vec2::new(3.0, -5.0), c + Vec2::new(3.0, 5.0)));
        assert!(!reach.touches_segment(c + Vec2::new(5.0, -5.0), c + Vec2::new(5.0, 5.0)));
    }

    #[test]
    fn a_segment_touches_a_sector_only_where_it_comes_in() {
        let c = Vec2::new(10.0, 20.0);
        let reach = spot(Vec2::X, FRAC_PI_2).reach().expect("gives light");
        let wall = |x: f32, y0: f32, y1: f32| (c + Vec2::new(x, y0), c + Vec2::new(x, y1));
        // Ahead, crossing the middle.
        let (a, b) = wall(2.0, -0.5, 0.5);
        assert!(reach.touches_segment(a, b));
        // Behind: in the disc, not the cone.
        let (a, b) = wall(-2.0, -0.5, 0.5);
        assert!(!reach.touches_segment(a, b));
        // Beside, above the cone's upper edge.
        let (a, b) = wall(1.0, 2.0, 3.0);
        assert!(!reach.touches_segment(a, b));
        // Both ends outside, across the cone's edge.
        let (a, b) = (c + Vec2::new(1.0, 3.0), c + Vec2::new(3.0, 1.0));
        assert!(reach.touches_segment(a, b));
        // Both ends outside the disc, across the arc only, inside the cone.
        let (a, b) = wall(3.9, -1.5, 1.5);
        assert!(reach.touches_segment(a, b));
        // Through the light itself, the rest of it behind.
        let (a, b) = wall(0.0, -2.0, 2.0);
        assert!(reach.touches_segment(a, b));
    }

    #[test]
    fn widening_brings_an_edge_beside_the_cone_in() {
        let c = Vec2::new(10.0, 20.0);
        let reach = spot(Vec2::X, FRAC_PI_2).reach().expect("gives light");
        // Just past the 45° edge.
        let way = Vec2::from_angle(FRAC_PI_2 / 2.0 + 0.05);
        let (a, b) = (c + way * 2.0, c + way * 3.0);
        assert!(!reach.touches_segment(a, b));
        assert!(reach.widened(0.1).touches_segment(a, b));
        // A disc stays a disc, and a cone widened past a half turn is one.
        assert_eq!(
            lamp(None).reach().map(|r| r.widened(0.1)),
            lamp(None).reach()
        );
        let wide = spot(Vec2::X, 2.0 * PI - 0.1).reach().expect("gives light");
        assert_eq!(wide.widened(0.1).cone, None);
    }
}
