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
//! After the move, a character that stood on a floor and is not moving up
//! reaches down by `snap` for one, so walking down a slope or off a small
//! step stays on the ground. Standing on a slope under gravity alone does
//! not slide down it unless `stop_on_slope` is off.
//!
//! [`query::cast`]: crate::query::cast

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{CharacterBody2D, CharacterContact, Collider, CollisionLayers, Transform};

use crate::query::{self, QueryFilter, ShapeHit};

mod step;

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

    let mut slide = mover.slide(&body, &mut at, velocity, delta);
    if body.contact.on_floor
        && !slide.contact.on_floor
        && body.snap > 0.0
        && slide.velocity.dot(mover.up) <= 0.0
    {
        mover.snap(&mut at, &mut slide, body.snap);
    }

    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.translation = at.translation;
    }
    if let Some(body) = world.get_mut::<CharacterBody2D>(entity) {
        body.contact = slide.contact;
    }
    Some(slide)
}

/// What every cast of one move shares.
struct Mover<'w> {
    world: &'w World,
    /// The collider grown by the skin — see [`inflated`].
    shape: Collider,
    filter: QueryFilter,
    up: Vec2,
    /// The cosine of the steepest floor.
    floor_cos: f32,
}

impl Mover<'_> {
    fn cast(&self, at: &Transform, motion: Vec2) -> Option<ShapeHit> {
        query::cast(self.world, &self.shape, at, motion, self.filter)
    }

    fn is_floor(&self, normal: Vec2) -> bool {
        normal.dot(self.up) >= self.floor_cos
    }

    /// Records a surface as floor, wall or ceiling.
    fn touch(&self, slide: &mut Slide, hit: ShapeHit) {
        let rise = hit.normal.dot(self.up);
        let contact = &mut slide.contact;
        if rise >= self.floor_cos {
            contact.on_floor = true;
            contact.floor_normal = hit.normal;
            contact.floor = Some(hit.entity);
        } else if rise <= -self.floor_cos {
            contact.on_ceiling = true;
        } else {
            contact.on_wall = true;
            contact.wall_normal = hit.normal;
        }
        slide.hits.push(hit);
    }

    /// The cast-and-slide loop.
    fn slide(
        &self,
        body: &CharacterBody2D,
        at: &mut Transform,
        velocity: Vec2,
        delta: f32,
    ) -> Slide {
        let up = self.up;
        let mut velocity = velocity;
        let mut motion = velocity * delta;
        // Gravity alone: straight down, nothing across.
        let falling_only = motion.dot(up) < 0.0 && (motion - up * motion.dot(up)).length() < STILL;
        let mut slide = Slide::default();
        let mut previous: Option<Vec2> = None;
        for _ in 0..body.max_slides.max(1) {
            let Some(hit) = self.cast(at, motion) else {
                at.translation += motion;
                break;
            };
            if hit.depth > TOUCH {
                // Inside the skin — or inside the collider: out to the skin
                // first, then the same motion again.
                self.touch(&mut slide, hit);
                at.translation += hit.normal * hit.depth;
                continue;
            }
            let length = motion.length();
            if length < STILL {
                self.touch(&mut slide, hit);
                break;
            }
            let direction = motion / length;
            at.translation += direction * hit.distance;
            motion -= direction * hit.distance;

            let floor = self.is_floor(hit.normal);
            let grounded = body.contact.on_floor || slide.contact.on_floor;
            let wall = !floor && hit.normal.dot(up) > -self.floor_cos;
            if wall && grounded {
                let across = motion - up * motion.dot(up);
                if let Some(step) = self.step_up(at, across, body.step_height) {
                    // Over it: a seam or a ledge, not a wall, and what it
                    // stands on is what it landed on.
                    self.touch(&mut slide, step.landing);
                    *at = step.at;
                    motion = across - step.covered;
                    continue;
                }
            }
            self.touch(&mut slide, hit);
            if floor && body.stop_on_slope && falling_only {
                // Landing, or standing, on a slope: stay put on it.
                velocity -= up * velocity.dot(up).min(0.0);
                break;
            }
            // On the ground, a wall is a wall however it leans: slid along as
            // if upright, so a slope too steep to stand on is not climbed by
            // walking into it. In the air its own normal applies, and it is
            // slid down.
            let normal = if !floor && grounded && hit.normal.dot(up) > 0.0 {
                (hit.normal - up * hit.normal.dot(up)).normalize_or(hit.normal)
            } else {
                hit.normal
            };
            motion = off_surface(motion, normal);
            velocity = off_surface(velocity, normal);
            if let Some(before) = previous {
                // A crease: sliding along this surface goes back into the last.
                if motion.dot(before) < -STILL {
                    motion = Vec2::ZERO;
                }
                if velocity.dot(before) < 0.0 {
                    velocity = off_surface(velocity, before);
                    if velocity.dot(normal) < 0.0 {
                        velocity = Vec2::ZERO;
                    }
                }
            }
            previous = Some(normal);
            if motion.length() < STILL {
                break;
            }
        }
        slide.velocity = velocity;
        slide
    }

    /// Reaches down by `reach` for a floor, and stands on it.
    fn snap(&self, at: &mut Transform, slide: &mut Slide, reach: f32) {
        let Some(hit) = self.cast(at, -self.up * reach) else {
            return;
        };
        if hit.depth > TOUCH || !self.is_floor(hit.normal) {
            return;
        }
        at.translation -= self.up * hit.distance;
        self.touch(slide, hit);
    }
}

/// The collider grown by `skin` all round, in world units, with the
/// transform it is cast from: the entity's place and turn, at unit scale.
///
/// Cast instead of the collider, it keeps the collider a skin from every
/// surface along that surface's normal — exactly, whatever the angle of
/// the move — and a collider that ends up nearer than the skin overlaps it,
/// and is pushed back out.
fn inflated(collider: &Collider, transform: &Transform, skin: f32) -> (Collider, Transform) {
    let shape = match collider {
        Collider::Box { .. } => Collider::Box {
            half_extents: collider.world_half_extents(transform) + Vec2::splat(skin),
        },
        Collider::Circle { .. } => Collider::Circle {
            radius: collider.world_radius(transform) + skin,
        },
    };
    let at = Transform {
        scale: Vec2::ONE,
        ..*transform
    };
    (shape, at)
}

/// `vector` with whatever goes into a surface of `normal` taken off.
fn off_surface(vector: Vec2, normal: Vec2) -> Vec2 {
    vector - normal * vector.dot(normal).min(0.0)
}
