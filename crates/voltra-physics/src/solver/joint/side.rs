//! One body as a joint reads it, and the two little vector helpers every
//! joint's arithmetic uses.

use voltra_render::glam::Vec2;

use crate::solver::SolverBodies;

/// One body as a joint reads it, copied out of the solver's array and back.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Side {
    pub(super) v: Vec2,
    pub(super) w: f32,
    pub(super) m: f32,
    pub(super) i: f32,
    pub(super) dp: Vec2,
    pub(super) dq: f32,
}

impl Side {
    pub(super) fn load(bodies: &SolverBodies, index: Option<usize>) -> Self {
        let Some(index) = index else {
            return Self::default();
        };
        let body = bodies.get(index);
        Self {
            v: body.velocity,
            w: body.angular_velocity,
            m: body.inverse_mass,
            i: body.inverse_inertia,
            dp: body.delta_position,
            dq: body.delta_rotation,
        }
    }

    pub(super) fn store(self, bodies: &mut SolverBodies, index: Option<usize>) {
        if let Some(index) = index {
            let body = bodies.get_mut(index);
            body.velocity = self.v;
            body.angular_velocity = self.w;
        }
    }
}

/// `v` turned by `angle`.
pub(super) fn turn(angle: f32, v: Vec2) -> Vec2 {
    Vec2::from_angle(angle).rotate(v)
}

/// 2D cross product of a scalar spin with a vector: the velocity of a point
/// at `r` on a body turning at `w`.
pub(super) fn spin(w: f32, r: Vec2) -> Vec2 {
    Vec2::new(-w * r.y, w * r.x)
}
