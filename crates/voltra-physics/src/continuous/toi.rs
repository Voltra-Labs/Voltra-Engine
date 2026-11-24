//! When a moving, turning shape first meets a still one: conservative
//! advancement over GJK, with the turn bounded.
//!
//! Mirtich's method, as Box2D's time of impact started: measure the gap,
//! and advance by the gap over the fastest any point of the shape can
//! close it — the move's length plus its turn times the shape's reach. No
//! point covers the gap in that time, so a step never passes through; and
//! the walk ends within a hair of first contact.

use voltra_render::glam::Vec2;
use voltra_scene::{Collider, Transform};

use crate::geometry::{closest, Hull};

/// The gap the walk closes to, in world units: what is left between the
/// shapes at the time of impact.
pub(super) const TARGET: f32 = 1e-3;

/// Within this of [`TARGET`] the walk has arrived.
const TOLERANCE: f32 = 2.5e-4;

/// A turn dominates the bound and slows the walk; past this many steps the
/// last safe time is the answer.
const MAX_ITERATIONS: usize = 32;

/// A shape's move through one step: from `start`, by `translation` and
/// `rotation` radians, both linear in time.
pub(super) struct Motion<'a> {
    pub collider: &'a Collider,
    pub start: Transform,
    pub translation: Vec2,
    pub rotation: f32,
}

impl Motion<'_> {
    fn at(&self, t: f32) -> Transform {
        Transform {
            translation: self.start.translation + self.translation * t,
            rotation: self.start.rotation + self.rotation * t,
            ..self.start
        }
    }
}

/// The share of `motion` free of `target`, and the normal pushing the moved
/// shape away from it there; `None` when they never meet within the move,
/// or overlap already — the step's own contacts' business.
pub(super) fn time_of_impact(motion: &Motion<'_>, target: &Hull) -> Option<(f32, Vec2)> {
    let start = Hull::of(motion.collider, &motion.start);
    let reach = start
        .vertices()
        .iter()
        .map(|v| v.distance(motion.start.translation))
        .fold(0.0, f32::max);
    let bound = motion.translation.length() + motion.rotation.abs() * reach;
    if !(bound.is_finite() && bound > 0.0) {
        return None;
    }
    let radius = start.radius + target.radius;

    let mut t = 0.0_f32;
    let mut normal = Vec2::ZERO;
    for _ in 0..MAX_ITERATIONS {
        let hull = if t == 0.0 {
            start
        } else {
            Hull::of(motion.collider, &motion.at(t))
        };
        let nearest = closest(hull.vertices(), target.vertices());
        let gap = nearest.distance - radius;
        let Some(towards) = nearest.face_normal(hull.vertices(), target.vertices()) else {
            // The cores meet: overlapping from the start, or — never, since
            // each step stops short — overlapping later.
            return (t > 0.0).then_some((t, normal));
        };
        normal = towards;
        if t == 0.0 && gap < 0.0 {
            return None;
        }
        if gap <= TARGET + TOLERANCE {
            // Within reach from the start: met now if it is moving in.
            if t == 0.0 && motion.translation.dot(normal) >= 0.0 {
                return None;
            }
            return Some((t, normal));
        }
        t += (gap - TARGET) / bound;
        if t >= 1.0 {
            return None;
        }
    }
    Some((t, normal))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall() -> Hull {
        // 0.1 thick, its left face at x = 5.
        Hull::of(
            &Collider::Box {
                half_extents: Vec2::new(0.05, 5.0),
            },
            &Transform::from_translation(Vec2::new(5.05, 0.0)),
        )
    }

    #[test]
    fn a_fast_box_meets_a_thin_wall_where_its_face_reaches_it() {
        let ball = Collider::Box {
            half_extents: Vec2::splat(0.5),
        };
        let motion = Motion {
            collider: &ball,
            start: Transform::default(),
            translation: Vec2::new(10.0, 0.0),
            rotation: 0.0,
        };
        let (t, normal) = time_of_impact(&motion, &wall()).expect("it meets the wall");

        assert!((t * 10.0 - (4.5 - TARGET)).abs() < 1e-3, "{t}");
        assert!((normal - Vec2::NEG_X).length() < 1e-4, "{normal:?}");
    }

    #[test]
    fn a_spinning_bar_meets_a_wall_its_centre_never_nears() {
        let bar = Collider::Box {
            half_extents: Vec2::new(5.5, 0.05),
        };
        // Upright to flat in one step, its centre still: only its end moves.
        let motion = Motion {
            collider: &bar,
            start: Transform::default().with_rotation(std::f32::consts::FRAC_PI_2),
            translation: Vec2::ZERO,
            rotation: -std::f32::consts::FRAC_PI_2,
        };
        let (t, _) = time_of_impact(&motion, &wall()).expect("its end sweeps into the wall");
        assert!(t < 1.0 && t > 0.0, "{t}");

        let ends_at = motion.at(t);
        let hull = Hull::of(&bar, &ends_at);
        let gap = closest(hull.vertices(), wall().vertices()).distance;
        assert!(gap > 0.0 && gap < 0.01, "stopped just short: {gap}");
    }

    #[test]
    fn a_shape_moving_away_or_short_of_it_meets_nothing() {
        let ball = Collider::Circle { radius: 0.5 };
        let away = Motion {
            collider: &ball,
            start: Transform::default(),
            translation: Vec2::new(-10.0, 0.0),
            rotation: 0.0,
        };
        assert!(time_of_impact(&away, &wall()).is_none());
        let short = Motion {
            translation: Vec2::new(4.0, 0.0),
            ..away
        };
        assert!(time_of_impact(&short, &wall()).is_none());
    }

    #[test]
    fn a_shape_already_overlapping_is_left_to_the_contacts() {
        let ball = Collider::Circle { radius: 0.5 };
        let motion = Motion {
            collider: &ball,
            start: Transform::from_translation(Vec2::new(4.7, 0.0)),
            translation: Vec2::new(10.0, 0.0),
            rotation: 0.0,
        };
        assert!(time_of_impact(&motion, &wall()).is_none());
    }
}
