//! Moving platforms: a character carried by the floor it stood on, and
//! turned with it when it asks to be (`carry_rotation`).
//!
//! By the floor's own pose, not its velocity: a platform moved by a
//! kinematic body, by an animation clip or by the game's code moves its
//! riders the same way, and one that turns carries them round its centre.
//! Godot carries by the platform's velocity, which a clip or a script
//! setting the position does not have.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{CharacterContact, Transform2D};

/// How far the floor `contact` names has carried a character at `at` since
/// it was stood on — moved, and turned about its own centre — and how far
/// it has turned. Nothing for a floor that is gone, or none.
pub(super) fn carry(world: &World, contact: &CharacterContact, at: Vec2) -> (Vec2, f32) {
    let (Some(floor), Some((was, was_turned))) = (contact.floor, contact.floor_pose) else {
        return (Vec2::ZERO, 0.0);
    };
    let Some(now) = world.get::<Transform2D>(floor) else {
        return (Vec2::ZERO, 0.0);
    };
    let turn = wrapped(now.rotation - was_turned);
    let moved = now.translation + Vec2::from_angle(turn).rotate(at - was);
    let carried = moved - at;
    if carried.is_finite() && turn.is_finite() {
        (carried, turn)
    } else {
        (Vec2::ZERO, 0.0)
    }
}

/// `angle` as the same turn in (−π, π]: a floor whose rotation crossed from
/// π to −π turned a little, not nearly all the way round the other way.
fn wrapped(angle: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let turn = angle.rem_euclid(TAU);
    if turn > PI {
        turn - TAU
    } else {
        turn
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;

    #[test]
    fn a_turn_is_the_short_way_round() {
        assert!((wrapped(0.3) - 0.3).abs() < 1e-6);
        assert!((wrapped(-0.3) + 0.3).abs() < 1e-6);
        assert!((wrapped(-3.1 - 3.1) - (2.0 * PI - 6.2)).abs() < 1e-5);
        assert!((wrapped(3.0 * PI) - PI).abs() < 1e-5);
    }
}
