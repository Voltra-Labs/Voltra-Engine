//! Pushing: a character walking into a dynamic body shoves it.
//!
//! An impulse along the contact's normal, into the body, at the contact
//! point — so a crate pushed low tips — of just enough to bring that point
//! up to the speed the character moves into it, and never more than
//! `push_force × delta`. A heavier body takes the same impulse with less
//! speed. The character's own move still stops at the body this step: the
//! body moves when the physics step that follows integrates it, so it is
//! never pushed into a wall through the character, and the character never
//! through it.
//!
//! Unity's `CharacterController` and Godot's `CharacterBody2D` leave this
//! to the game, in `OnControllerColliderHit` or a loop over the slide's
//! collisions; every platformer then writes the same few lines. Here it is
//! a field.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{BodyType, Collider, RigidBody, Transform, WorldShape};

use crate::query::ShapeHit;
use crate::solver::SolverBody;

use super::slide::Mover;

/// How near a point must be to a shape's furthest along a normal to be part
/// of the face it meets with, as a fraction of the shape's width across.
const FACE: f32 = 0.01;

/// A dynamic body met during a move, to be pushed once the move is over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Push {
    pub entity: Entity,
    /// The body's surface normal where they met, towards the character.
    pub normal: Vec2,
    /// Where they met, in the world.
    pub point: Vec2,
    /// How fast the character was moving into the body.
    pub speed: f32,
    /// Whether the body was the floor: pressed on, but the character's own
    /// speed into a floor is taken off as on any floor.
    pub floor: bool,
}

impl Mover<'_> {
    /// The push the character, at `at` and moving at `velocity`, gives what
    /// `hit` met — `None` for anything but a dynamic body with mass, or a
    /// character not moving into it.
    pub(super) fn push(
        &self,
        at: &Transform,
        hit: &ShapeHit,
        velocity: Vec2,
        floor: bool,
    ) -> Option<Push> {
        let speed = -velocity.dot(hit.normal);
        if speed.is_nan() || speed <= 0.0 {
            return None;
        }
        let body = self.world.get::<RigidBody>(hit.entity)?;
        if body.body_type != BodyType::Dynamic
            || body.inverse_mass.is_nan()
            || body.inverse_mass <= 0.0
        {
            return None;
        }
        let collider = self.world.get::<Collider>(hit.entity)?;
        let transform = self.world.get::<Transform>(hit.entity)?;
        let point = contact_point(
            &self.shape.world_shape(at),
            &collider.world_shape(transform),
            hit.normal,
        );
        point.is_finite().then_some(Push {
            entity: hit.entity,
            normal: hit.normal,
            point,
            speed,
            floor,
        })
    }
}

/// Adds `push` to `pushes`, or keeps the harder of two on one body: a
/// move sliding along a body meets it more than once.
pub(super) fn record(pushes: &mut Vec<Push>, push: Push) {
    match pushes.iter_mut().find(|p| p.entity == push.entity) {
        Some(known) if known.speed < push.speed => *known = push,
        Some(_) => {}
        None => pushes.push(push),
    }
}

/// Pushes every body in `pushes` with up to `force × delta`, and gives
/// `velocity` back the speed each body took off a wall or ceiling: a light
/// crate is walked on at full speed, a heavy one at its own pace. A body
/// gone since the move found it is skipped.
pub(super) fn apply(
    world: &mut World,
    pushes: &[Push],
    force: f32,
    delta: f32,
    velocity: &mut Vec2,
) {
    let limit = force * delta;
    if !limit.is_finite() || limit <= 0.0 {
        return;
    }
    for push in pushes {
        let Some(kept) = shove(world, push, limit) else {
            continue;
        };
        if push.floor {
            continue;
        }
        let into = -push.normal;
        let short = kept - velocity.dot(into);
        if short > 0.0 {
            *velocity += into * short;
        }
    }
}

/// Gives `push`'s body an impulse of at most `limit`, and answers the speed
/// its contact point now moves at along the push — no more than the
/// character's. `None` when the body is gone or cannot be moved.
fn shove(world: &mut World, push: &Push, limit: f32) -> Option<f32> {
    let collider = world.get::<Collider>(push.entity).copied();
    let transform = *world.get::<Transform>(push.entity)?;
    let body = world.get::<RigidBody>(push.entity)?;
    let solver = SolverBody::from_body(
        push.entity,
        body,
        collider.as_ref().map(|c| (c, &transform)),
    );
    let into = -push.normal;
    let arm = push.point - transform.translation;
    let lever = arm.perp_dot(into);
    let resistance = solver.inverse_mass + solver.inverse_inertia * lever * lever;
    if resistance.is_nan() || resistance <= 0.0 {
        return None;
    }
    let moving = (solver.velocity + arm.perp() * solver.angular_velocity).dot(into);
    let wanted = push.speed - moving;
    let impulse = (wanted / resistance).clamp(0.0, limit);
    if impulse > 0.0 {
        let body = world.get_mut::<RigidBody>(push.entity)?;
        body.velocity += into * (impulse * solver.inverse_mass);
        body.angular_velocity += solver.inverse_inertia * lever * impulse;
    }
    Some((moving + impulse * resistance).clamp(0.0, push.speed))
}

/// Where two shapes touching along `normal` — the second's surface normal,
/// towards the first — meet: the middle of where the faces each turns to
/// the other overlap across the normal, on the second's surface. A corner
/// against a face is the corner; two faces, the middle of what they share.
fn contact_point(mover: &WorldShape, body: &WorldShape, normal: Vec2) -> Vec2 {
    let across = normal.perp();
    let (top, (body_low, body_high)) = face(body.vertices(), normal, across);
    let (_, (mover_low, mover_high)) = face(mover.vertices(), -normal, across);
    // Apart across — a corner just past another — this is midway between
    // the nearest ends, which is the same formula.
    let middle = (body_low.max(mover_low) + body_high.min(mover_high)) * 0.5;
    across * middle + normal * (top + body.radius())
}

/// How far `points` reach along `toward`, and the span across of those
/// that reach it: the face, edge or corner they meet things with there.
fn face(points: &[Vec2], toward: Vec2, across: Vec2) -> (f32, (f32, f32)) {
    let top = points
        .iter()
        .map(|p| p.dot(toward))
        .fold(f32::NEG_INFINITY, f32::max);
    let (low, high) = span(points.iter().map(|p| p.dot(across)));
    let tolerance = FACE * (high - low) + 1e-5;
    let on_face = points
        .iter()
        .filter(|p| p.dot(toward) >= top - tolerance)
        .map(|p| p.dot(across));
    (top, span(on_face))
}

fn span(values: impl Iterator<Item = f32>) -> (f32, f32) {
    values.fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), v| {
        (low.min(v), high.max(v))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(collider: Collider, at: Vec2, rotation: f32) -> WorldShape {
        collider.world_shape(&Transform::from_translation(at).with_rotation(rotation))
    }

    fn square(half: f32) -> Collider {
        Collider::Box {
            half_extents: Vec2::splat(half),
        }
    }

    #[test]
    fn two_faces_meet_in_the_middle_of_what_they_share() {
        // A small box left of a big one, its centre a little high.
        let mover = shape(square(0.25), Vec2::new(-1.25, 0.5), 0.0);
        let body = shape(square(1.0), Vec2::ZERO, 0.0);
        let point = contact_point(&mover, &body, Vec2::NEG_X);
        assert!(point.distance(Vec2::new(-1.0, 0.5)) < 1e-5, "{point}");

        // Taller than the body: the body's own side, centred.
        let mover = shape(
            Collider::Box {
                half_extents: Vec2::new(0.25, 3.0),
            },
            Vec2::new(-1.25, 0.0),
            0.0,
        );
        let point = contact_point(&mover, &body, Vec2::NEG_X);
        assert!(point.distance(Vec2::new(-1.0, 0.0)) < 1e-5, "{point}");
    }

    #[test]
    fn a_corner_meets_at_the_corner_and_a_circle_on_its_rim() {
        // A diamond: its left corner at (-1, 0).
        let body = shape(
            square(std::f32::consts::FRAC_1_SQRT_2),
            Vec2::ZERO,
            std::f32::consts::FRAC_PI_4,
        );
        let mover = shape(square(0.25), Vec2::new(-1.25, 0.1), 0.0);
        let point = contact_point(&mover, &body, Vec2::NEG_X);
        assert!(point.distance(Vec2::new(-1.0, 0.0)) < 1e-4, "{point}");

        let ball = shape(Collider::Circle { radius: 0.5 }, Vec2::new(2.0, 0.0), 0.0);
        let mover = shape(square(0.25), Vec2::new(1.25, 0.0), 0.0);
        let point = contact_point(&mover, &ball, Vec2::NEG_X);
        assert!(point.distance(Vec2::new(1.5, 0.0)) < 1e-5, "{point}");
    }
}
