//! A character moved by the game rather than by forces.

use serde::{Deserialize, Serialize};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;

/// Marks an entity with a collider as a character a game moves with
/// `voltra_physics::character::move_and_slide`, and holds how it slides and
/// what it touched.
///
/// Godot's `CharacterBody2D`: a velocity in, a move that stops at walls and
/// slides along them, floors told from walls by their slope. The entity
/// should carry no `RigidBody2D` — the step would move it too — and is a
/// static obstacle to the bodies the step does move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterBody2D {
    /// Which way is up: what a floor faces, and a ceiling faces away from.
    /// Normalised on use; one of no length is `+y`.
    pub up: Vec2,
    /// The steepest a floor may be, in radians from `up`. Steeper is a wall.
    pub floor_max_angle: f32,
    /// The gap kept from what it touches, in world units, so the next move
    /// starts clear of it rather than a rounding inside.
    pub skin: f32,
    /// How far down it reaches for a floor to stay on — walking off a small
    /// step, or down a slope faster than it falls. Zero never snaps. Not
    /// while moving up: a jump leaves the floor.
    pub snap: f32,
    /// The highest ledge it walks up onto rather than stopping at, in world
    /// units. Above zero, too, so the seam between two colliders level with
    /// each other — a floor of tiles — is walked over, not caught on.
    pub step_height: f32,
    /// Whether standing on a slope holds still rather than sliding down it
    /// under gravity.
    pub stop_on_slope: bool,
    /// The most surfaces one move slides along.
    pub max_slides: u32,
    /// The most force, in newtons (world units of mass × units per second
    /// squared), it pushes a dynamic body it walks into with: an impulse
    /// of at most `push_force × delta` a move, along the contact's normal at
    /// the contact point, so a heavier crate moves less and one pushed low
    /// may tip. Never faster than the character moves into it; a body it
    /// stands on is pressed when landed on, not again while stood on. Zero
    /// pushes nothing — the body is a wall, as any other.
    pub push_force: f32,
    /// Whether walking up or down a floor's slope covers the same ground
    /// across `up` per second as walking on the flat — Godot's
    /// `floor_constant_speed`, measured across rather than along the slope.
    /// Off, the part of a walk going into a slope is taken off — slower up
    /// it — and down one it walks off into the air for `snap` to bring back.
    /// On, the velocity kept is the walk across, with no part up or down —
    /// but for, with `stop_on_slope` off, the slide down the slope.
    pub constant_speed_on_slopes: bool,
    /// Whether a turning floor turns the character's own `Transform2D`
    /// rotation with it, as well as carrying it round. Off keeps it as it
    /// was — a capsule stays upright on a tilting platform.
    pub carry_rotation: bool,
    /// What the last move touched. Never saved.
    #[serde(skip)]
    pub contact: CharacterContact,
}

/// What a character's last move touched.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CharacterContact {
    /// Whether the last move ended on a floor.
    pub on_floor: bool,
    /// Whether the last move touched a wall.
    pub on_wall: bool,
    /// Whether the last move touched a ceiling.
    pub on_ceiling: bool,
    /// The floor's normal while on one; zero otherwise.
    pub floor_normal: Vec2,
    /// The last wall's normal while on one; zero otherwise.
    pub wall_normal: Vec2,
    /// What it stands on.
    pub floor: Option<Entity>,
    /// Where the floor was, and how it was turned, when it was last stood
    /// on: the next move carries the character by however far it has moved
    /// and turned since — a moving platform, kinematic, animated or moved
    /// by the game.
    pub floor_pose: Option<(Vec2, f32)>,
    /// How fast the floor carried it on the last move, in world units per
    /// second: what a game adds to a jump off a moving platform, if it
    /// wants the platform's speed kept.
    pub platform_velocity: Vec2,
}

impl Default for CharacterBody2D {
    fn default() -> Self {
        Self {
            up: Vec2::Y,
            floor_max_angle: std::f32::consts::FRAC_PI_4,
            skin: 0.01,
            snap: 0.1,
            step_height: 0.05,
            stop_on_slope: true,
            max_slides: 4,
            push_force: 0.0,
            constant_speed_on_slopes: false,
            carry_rotation: false,
            contact: CharacterContact::default(),
        }
    }
}

impl CharacterBody2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "CharacterBody2D";

    /// Whether the last move ended on a floor.
    pub fn is_on_floor(&self) -> bool {
        self.contact.on_floor
    }

    /// Whether the last move touched a wall.
    pub fn is_on_wall(&self) -> bool {
        self.contact.on_wall
    }

    /// Whether the last move touched a ceiling.
    pub fn is_on_ceiling(&self) -> bool {
        self.contact.on_ceiling
    }

    /// `up`, normalised, or `+y` for one of no length.
    pub fn up_direction(&self) -> Vec2 {
        self.up.try_normalize().unwrap_or(Vec2::Y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_character_writes_its_tuning_and_not_its_contact() {
        let text = ron::to_string(&CharacterBody2D::default()).expect("writes");
        assert!(!text.contains("contact"), "{text}");
        let back: CharacterBody2D = ron::from_str("(snap: 0.0)").expect("reads");
        assert_eq!(back.snap, 0.0);
        assert_eq!(back.max_slides, 4);
        assert_eq!(back.push_force, 0.0, "pushes nothing unless told to");
        assert!(!back.constant_speed_on_slopes && !back.carry_rotation);
        assert_eq!(
            CharacterBody2D {
                up: Vec2::ZERO,
                ..back
            }
            .up_direction(),
            Vec2::Y
        );
    }
}
