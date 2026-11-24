use std::f32::consts::FRAC_PI_4;

use super::*;

/// A floor whose top is at y = -0.5.
const FLOOR: ParticleCollider = ParticleCollider::Box {
    center: Vec2::new(0.0, -1.0),
    rotation: 0.0,
    half_extents: Vec2::new(5.0, 0.5),
};

fn world() -> Space {
    Space::new(Mat3::IDENTITY).expect("the identity undoes itself")
}

fn bounce(bounce: f32, friction: f32) -> ParticleResponse {
    ParticleResponse {
        bounce,
        friction,
        lifetime_loss: 0.0,
    }
}

/// A particle that moved from `before` to `after` at `velocity`.
fn moved(before: Vec2, after: Vec2, velocity: Vec2) -> (Particle, Vec2) {
    (
        Particle {
            position: after,
            velocity,
            age: 0.5,
            lifetime: 1.0,
            rotation: 0.0,
            spin: 0.0,
        },
        before,
    )
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).length() < 1e-4
}

#[test]
fn a_particle_falling_onto_a_box_bounces_off_its_top() {
    let (mut p, before) = moved(Vec2::ZERO, Vec2::new(0.0, -1.0), Vec2::new(0.0, -10.0));
    collide(&mut p, before, &[FLOOR], &bounce(0.5, 0.0), &world());
    assert!(close(p.position, Vec2::new(0.0, -0.5 + SKIN)), "{p:?}");
    assert!(close(p.velocity, Vec2::new(0.0, 5.0)), "{p:?}");
    assert_eq!(p.lifetime, 1.0);
}

#[test]
fn friction_takes_its_share_of_the_speed_along_the_surface() {
    let (mut p, before) = moved(Vec2::ZERO, Vec2::new(0.4, -1.0), Vec2::new(4.0, -10.0));
    collide(&mut p, before, &[FLOOR], &bounce(0.0, 0.25), &world());
    assert!(close(p.velocity, Vec2::new(3.0, 0.0)), "{p:?}");
}

#[test]
fn a_fast_particle_does_not_pass_through_a_thin_wall() {
    let wall = ParticleCollider::Box {
        center: Vec2::new(5.0, 0.0),
        rotation: 0.0,
        half_extents: Vec2::new(0.01, 2.0),
    };
    let (mut p, before) = moved(Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(100.0, 0.0));
    collide(&mut p, before, &[wall], &bounce(1.0, 0.0), &world());
    assert!(p.position.x < 5.0, "stopped at the wall: {p:?}");
    assert!(close(p.velocity, Vec2::new(-100.0, 0.0)), "{p:?}");
}

#[test]
fn a_particle_starting_inside_a_shape_dies() {
    let (mut p, before) = moved(
        Vec2::new(0.0, -1.0),
        Vec2::new(0.0, -1.1),
        Vec2::new(0.0, -1.0),
    );
    collide(&mut p, before, &[FLOOR], &bounce(1.0, 0.0), &world());
    assert_eq!(p.lifetime, 0.0);
}

#[test]
fn a_particle_leaving_a_surface_it_rests_on_is_not_hit() {
    let top = -0.5 + SKIN;
    let (mut p, before) = moved(
        Vec2::new(0.0, top),
        Vec2::new(0.1, top + 0.1),
        Vec2::new(1.0, 1.0),
    );
    let was = p;
    collide(&mut p, before, &[FLOOR], &bounce(0.5, 0.5), &world());
    assert_eq!(p, was);
}

#[test]
fn a_circle_turns_a_particle_along_its_radius() {
    let ball = ParticleCollider::Circle {
        center: Vec2::new(2.0, 0.0),
        radius: 1.0,
    };
    let (mut p, before) = moved(Vec2::ZERO, Vec2::new(2.0, 0.0), Vec2::new(10.0, 0.0));
    collide(&mut p, before, &[ball], &bounce(1.0, 0.0), &world());
    assert!(close(p.position, Vec2::new(1.0 - SKIN, 0.0)), "{p:?}");
    assert!(close(p.velocity, Vec2::new(-10.0, 0.0)), "{p:?}");
}

#[test]
fn a_turned_box_bounces_along_its_turned_normal() {
    // A diamond: a unit box turned 45°, its upper-right face facing (1, 1).
    let diamond = ParticleCollider::Box {
        center: Vec2::ZERO,
        rotation: FRAC_PI_4,
        half_extents: Vec2::ONE,
    };
    let from = Vec2::splat(2.0);
    let (mut p, before) = moved(from, Vec2::ZERO, Vec2::splat(-1.0));
    collide(&mut p, before, &[diamond], &bounce(1.0, 0.0), &world());
    assert!(close(p.velocity, Vec2::splat(1.0)), "{p:?}");
    let face = Vec2::splat(std::f32::consts::FRAC_1_SQRT_2);
    assert!(close(p.position, face * (1.0 + SKIN)), "{p:?}");
}

#[test]
fn the_earliest_of_two_hits_wins() {
    let near = ParticleCollider::Circle {
        center: Vec2::new(3.0, 0.0),
        radius: 0.5,
    };
    let far = ParticleCollider::Circle {
        center: Vec2::new(6.0, 0.0),
        radius: 0.5,
    };
    let (mut p, before) = moved(Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(10.0, 0.0));
    collide(&mut p, before, &[far, near], &bounce(0.0, 0.0), &world());
    assert!(close(p.position, Vec2::new(2.5 - SKIN, 0.0)), "{p:?}");
}

#[test]
fn a_full_lifetime_loss_kills_on_the_first_hit() {
    let (mut p, before) = moved(Vec2::ZERO, Vec2::new(0.0, -1.0), Vec2::new(0.0, -10.0));
    let response = ParticleResponse {
        lifetime_loss: 1.0,
        ..ParticleResponse::default()
    };
    collide(&mut p, before, &[FLOOR], &response, &world());
    assert_eq!(p.lifetime, 0.0);
}

#[test]
fn a_local_space_particle_collides_where_it_is_in_the_world() {
    // The emitter sits 10 up and is scaled 2×: local (0, -5) is world
    // (0, 0), and the floor's top at world -0.5 is local -5.25.
    let space = Space::new(Mat3::from_scale_angle_translation(
        Vec2::splat(2.0),
        0.0,
        Vec2::new(0.0, 10.0),
    ))
    .expect("invertible");
    let (mut p, before) = moved(
        Vec2::new(0.0, -5.0),
        Vec2::new(0.0, -5.5),
        Vec2::new(0.0, -5.0),
    );
    collide(&mut p, before, &[FLOOR], &bounce(1.0, 0.0), &space);
    assert!(
        close(p.position, Vec2::new(0.0, -5.25 + SKIN / 2.0)),
        "{p:?}"
    );
    assert!(close(p.velocity, Vec2::new(0.0, 5.0)), "{p:?}");
}

#[test]
fn a_space_scaled_to_nothing_has_no_way_back() {
    assert!(Space::new(Mat3::from_scale(Vec2::new(0.0, 1.0))).is_none());
    assert!(Space::new(Mat3::from_cols_array(&[f32::NAN; 9])).is_none());
}
