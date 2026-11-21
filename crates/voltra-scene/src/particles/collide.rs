//! A particle's step, tested against the shapes it can hit — the CPU twin
//! of `particles_sim.wgsl`'s `collide`, line for line, so an emitter
//! bounces the same simulated either way.
//!
//! The path from where the particle was to where the step put it, in the
//! world, is swept against every shape. The earliest hit puts it [`SKIN`]
//! outside the surface and turns its velocity: the part into the surface
//! reflected and scaled by the bounce, the part along it cut by the
//! friction. The rest of the step's motion is dropped, which at a frame's
//! step is too small to see. A particle that starts a step inside a shape —
//! born in a wall, or caught by a moving one — dies.
//!
//! A swept test rather than an overlap test, so a fast spark does not pass
//! through a thin wall between two frames.

use voltra_render::glam::{Mat3, Vec2};
use voltra_render::{ParticleCollider, ParticleResponse};

use super::Particle;

/// How far outside a surface a hit particle is put, so its next step
/// starts clear of it. Must match `SKIN` in `particles_sim.wgsl`.
pub const SKIN: f32 = 1e-3;

/// Where a path meets a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Sweep {
    Miss,
    /// The path starts inside.
    Inside,
    /// It enters at `t` along it, `0` to `1`, through a surface facing
    /// `normal`.
    Hit {
        t: f32,
        normal: Vec2,
    },
}

/// A particle's space and the world, both ways. `None` when the space is
/// scaled to nothing and has no way back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Space {
    to_world: Mat3,
    from_world: Mat3,
}

impl Space {
    pub(crate) fn new(to_world: Mat3) -> Option<Self> {
        (to_world.is_finite() && to_world.determinant().abs() > 1e-12).then(|| Self {
            to_world,
            from_world: to_world.inverse(),
        })
    }

    /// A point of the particle's space, in the world.
    pub(crate) fn to_world(self, at: Vec2) -> Vec2 {
        self.to_world.transform_point2(at)
    }
}

/// Tests `particle`'s step from `before` against `shapes` and answers the
/// earliest hit with `response`, which is clamped already.
pub(crate) fn collide(
    particle: &mut Particle,
    before: Vec2,
    shapes: &[ParticleCollider],
    response: &ParticleResponse,
    space: &Space,
) {
    if shapes.is_empty() {
        return;
    }
    let was = space.to_world.transform_point2(before);
    let path = space.to_world.transform_point2(particle.position) - was;
    let mut best: Option<(f32, Vec2)> = None;
    for shape in shapes {
        match sweep(shape, was, path) {
            Sweep::Inside => {
                particle.lifetime = 0.0;
                return;
            }
            Sweep::Hit { t, normal } if best.is_none_or(|(best, _)| t < best) => {
                best = Some((t, normal));
            }
            _ => {}
        }
    }
    let Some((t, normal)) = best else {
        return;
    };
    let velocity = space.to_world.transform_vector2(particle.velocity);
    let into = velocity.dot(normal);
    let turned = if into < 0.0 {
        let along = velocity - normal * into;
        along * (1.0 - response.friction) - normal * into * response.bounce
    } else {
        velocity
    };
    particle.position = space
        .from_world
        .transform_point2(was + path * t + normal * SKIN);
    particle.velocity = space.from_world.transform_vector2(turned);
    particle.lifetime *= 1.0 - response.lifetime_loss;
}

fn sweep(shape: &ParticleCollider, was: Vec2, path: Vec2) -> Sweep {
    match *shape {
        ParticleCollider::Circle { center, radius } => {
            let m = was - center;
            let k = m.length_squared() - radius * radius;
            if k < 0.0 {
                return Sweep::Inside;
            }
            let a = path.length_squared();
            let b = m.dot(path);
            let disc = b * b - a * k;
            if a <= 0.0 || b >= 0.0 || disc < 0.0 {
                return Sweep::Miss;
            }
            let t = (-b - disc.sqrt()) / a;
            if t > 1.0 {
                return Sweep::Miss;
            }
            Sweep::Hit {
                t,
                normal: (m + path * t).normalize_or_zero(),
            }
        }
        ParticleCollider::Box {
            center,
            rotation,
            half_extents: h,
        } => {
            // Into the box's own frame, where it is axis-aligned.
            let turn = Vec2::from_angle(-rotation);
            let o = turn.rotate(was - center);
            let v = turn.rotate(path);
            if o.x.abs() < h.x && o.y.abs() < h.y {
                return Sweep::Inside;
            }
            let (mut enter, mut leave) = (-1e30_f32, 1e30_f32);
            let mut normal = Vec2::ZERO;
            for axis in 0..2 {
                let (start, delta, half) = (o[axis], v[axis], h[axis]);
                if delta.abs() < 1e-12 {
                    if start.abs() >= half {
                        return Sweep::Miss;
                    }
                    continue;
                }
                let (mut near, mut far) = ((-half - start) / delta, (half - start) / delta);
                let mut side = -1.0;
                if near > far {
                    (near, far) = (far, near);
                    side = 1.0;
                }
                if near > enter {
                    enter = near;
                    normal = Vec2::ZERO;
                    normal[axis] = side;
                }
                leave = leave.min(far);
                if enter > leave {
                    return Sweep::Miss;
                }
            }
            // Outside, so a path that reaches the box enters it at or after
            // its start; before it is the box behind the particle.
            if !(0.0..=1.0).contains(&enter) {
                return Sweep::Miss;
            }
            Sweep::Hit {
                t: enter,
                normal: Vec2::from_angle(rotation).rotate(normal),
            }
        }
    }
}

#[cfg(test)]
#[path = "collide_tests.rs"]
mod tests;
