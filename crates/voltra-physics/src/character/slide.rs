//! The cast-and-slide loop: what every cast of one move shares, and the
//! loop that casts, stops at a surface and slides along it.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{CharacterBody2D, Collider2D, Transform2D};

use crate::query::{self, QueryFilter, ShapeHit};

use super::push::{record, Push};
use super::slope::along_floor;
use super::{Slide, STILL, TOUCH};

/// What every cast of one move shares.
pub(super) struct Mover<'w> {
    pub(super) world: &'w World,
    /// The collider grown by the skin — see [`inflated`](super::inflated).
    pub(super) shape: Collider2D,
    pub(super) filter: QueryFilter,
    pub(super) up: Vec2,
    /// The cosine of the steepest floor.
    pub(super) floor_cos: f32,
}

impl Mover<'_> {
    pub(super) fn cast(&self, at: &Transform2D, motion: Vec2) -> Option<ShapeHit> {
        query::cast(self.world, &self.shape, at, motion, self.filter)
    }

    pub(super) fn is_floor(&self, normal: Vec2) -> bool {
        normal.dot(self.up) >= self.floor_cos
    }

    /// Records a surface as floor, wall or ceiling.
    pub(super) fn touch(&self, slide: &mut Slide, hit: ShapeHit) {
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

    /// The cast-and-slide loop, and the dynamic bodies it pushes.
    pub(super) fn slide(
        &self,
        body: &CharacterBody2D,
        at: &mut Transform2D,
        velocity: Vec2,
        delta: f32,
    ) -> (Slide, Vec<Push>) {
        let up = self.up;
        let mut velocity = velocity;
        let mut motion = velocity * delta;
        // Walking down the slope it stands on, at constant speed: along it
        // from the start, rather than off into the air for the snap to
        // bring back. Up a slope is turned along it where it is met, below.
        let floor_normal = body.contact.floor_normal;
        let walking = body.constant_speed_on_slopes
            && body.contact.on_floor
            && velocity.dot(up) <= 0.0
            && self.is_floor(floor_normal);
        if walking && (motion - up * motion.dot(up)).dot(floor_normal) > 0.0 {
            motion = along_floor(motion, floor_normal, up) + up * motion.dot(up);
        }
        // Gravity alone: straight down, nothing across.
        let falling_only = motion.dot(up) < 0.0 && (motion - up * motion.dot(up)).length() < STILL;
        let mut slide = Slide::default();
        let mut pushes = Vec::new();
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
            if body.push_force > 0.0 {
                let standing_on = body.contact.floor.filter(|_| body.contact.on_floor);
                if let Some(push) = self.push(at, &hit, velocity, floor, standing_on) {
                    record(&mut pushes, push);
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
            // Only with a walk across: standing, or sliding down under
            // gravity alone with `stop_on_slope` off, is left to the slope.
            if floor
                && grounded
                && body.constant_speed_on_slopes
                && !falling_only
                && velocity.dot(up) <= 0.0
            {
                // Walking on: whatever is left across, along this floor, and
                // the speed kept across — none of it lost to the slope. The
                // fall into it is taken off, unless the slope is to be slid
                // down: then the part of the fall along it is kept, and the
                // slide gathers speed as it does without constant speed.
                motion = along_floor(motion, normal, up);
                let fall = up * velocity.dot(up);
                velocity -= fall;
                if !body.stop_on_slope {
                    velocity += off_surface(fall, normal);
                }
            } else {
                motion = off_surface(motion, normal);
                velocity = off_surface(velocity, normal);
            }
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
        (slide, pushes)
    }
}

/// `vector` with whatever goes into a surface of `normal` taken off.
fn off_surface(vector: Vec2, normal: Vec2) -> Vec2 {
    vector - normal * vector.dot(normal).min(0.0)
}
