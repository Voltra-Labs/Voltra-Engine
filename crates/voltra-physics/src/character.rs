//! Moving a character: a velocity in, a move that stops at what it meets
//! and slides along it.
//!
//! Godot's `move_and_slide`, on [`query::cast`]: cast the collider along
//! the motion, stop at the first surface, take the part of the
//! motion that went into it off what is left — and off the velocity, so a
//! fall ends on landing and a jump on a ceiling — and cast again, up to
//! `max_slides` times. Each surface is a floor, a wall or a ceiling by its
//! normal's angle from `up`. What is cast is the collider grown by the skin,
//! so the collider stays a skin clear of every surface, and a surface within
//! the skin counts as touched. On the ground a leaning wall is slid along
//! as if upright — a slope too steep to stand on is not
//! climbed by walking into it (Godot's `floor_block_on_wall`); in the air it
//! is slid down. Two surfaces facing each other across a crease stop the
//! move rather than trade it between them for ever.
//!
//! A wall met on the ground is first tried as a step (`step_height`): over
//! a seam between two colliders level with each other, or onto a ledge low
//! enough — see [`step`].
//!
//! Before it, the character is carried by the floor it stood on — however
//! far that has moved and turned since (`CharacterContact::floor_pose`): a
//! moving platform, whatever moves it.
//!
//! After the move, a character that stood on a floor and is not moving up
//! reaches down by `snap` for one, so walking down a slope or off a small
//! step stays on the ground. Standing on a slope under gravity alone does
//! not slide down it unless `stop_on_slope` is off.
//!
//! [`query::cast`]: crate::query::cast

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{CharacterBody2D, CharacterContact, Collider, CollisionLayers, Transform};

use crate::query::{QueryFilter, ShapeHit};

mod carry;
mod inflated;
mod slide;
mod snap;
mod step;

use carry::carry;
use inflated::inflated;
use slide::Mover;

/// Below this a motion is no motion.
const STILL: f32 = 1e-7;

/// An overlap shallower than this is a touch: pushing out of it would only
/// chase rounding.
pub(super) const TOUCH: f32 = 1e-5;

/// What one move did.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Slide {
    /// The velocity, less what went into the surfaces met — the one to keep
    /// for the next tick.
    pub velocity: Vec2,
    /// What it touched, also stored on the component.
    pub contact: CharacterContact,
    /// Every surface met, in order.
    pub hits: Vec<ShapeHit>,
}

/// Moves the character `entity` by `velocity` for `delta` seconds.
///
/// `None`, moving nothing, unless the entity has a [`CharacterBody2D`], a
/// [`Collider`] and a [`Transform`]. It looks at the layers its
/// `CollisionLayers` mask names, as a body would, and never at itself or at
/// sensors.
pub fn move_and_slide(
    world: &mut World,
    entity: Entity,
    velocity: Vec2,
    delta: f32,
) -> Option<Slide> {
    let body = world.get::<CharacterBody2D>(entity)?.clone();
    let collider = *world.get::<Collider>(entity)?;
    let transform = *world.get::<Transform>(entity)?;
    let mask = world
        .get::<CollisionLayers>(entity)
        .map_or(CollisionLayers::ALL, |l| l.mask);
    let (shape, mut at) = inflated(&collider, &transform, body.skin.max(0.0));
    let mover = Mover {
        world,
        shape,
        filter: QueryFilter::new().looking_at(mask).excluding(entity),
        up: body.up_direction(),
        floor_cos: body.floor_max_angle.clamp(0.0, std::f32::consts::PI).cos(),
    };
    let delta = if delta.is_finite() {
        delta.max(0.0)
    } else {
        0.0
    };
    let velocity = if velocity.is_finite() {
        velocity
    } else {
        Vec2::ZERO
    };

    // Carried first, by the floor it stood on: a platform that moved under
    // it, or into it, takes it along. Put there outright — a wall it was
    // carried into is pushed out of by the move that follows.
    let carried = carry(world, &body.contact, at.translation);
    at.translation += carried;
    let mut slide = mover.slide(&body, &mut at, velocity, delta);
    if body.contact.on_floor
        && !slide.contact.on_floor
        && body.snap > 0.0
        && slide.velocity.dot(mover.up) <= 0.0
    {
        mover.snap(&mut at, &mut slide, body.snap);
    }

    slide.contact.floor_pose = slide
        .contact
        .floor
        .and_then(|floor| world.get::<Transform>(floor))
        .map(|floor| (floor.translation, floor.rotation));
    slide.contact.platform_velocity = if delta > 0.0 {
        carried / delta
    } else {
        Vec2::ZERO
    };

    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.translation = at.translation;
    }
    if let Some(body) = world.get_mut::<CharacterBody2D>(entity) {
        body.contact = slide.contact;
    }
    Some(slide)
}
