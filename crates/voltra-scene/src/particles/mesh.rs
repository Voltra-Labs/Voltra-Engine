//! An emitter's particles as quads in the sprite batch.

use voltra_render::glam::{Mat3, Vec2};

use super::ParticleEmitter;
use crate::batch::SpriteBatch;
use crate::sprite::quad::WHOLE;

/// Appends one quad per live particle, sized and tinted by how far through
/// its life it is. `matrix` is the emitter's world matrix — used only for a
/// local-space emitter, since a world-space one already stores world
/// positions.
pub fn push(batch: &mut SpriteBatch, matrix: Mat3, emitter: &ParticleEmitter) {
    let place = if emitter.world_space {
        Mat3::IDENTITY
    } else {
        matrix
    };
    let (u0, v0) = (WHOLE.0[0], WHOLE.0[1]);
    let (u1, v1) = (WHOLE.1[0], WHOLE.1[1]);
    let uvs = [[u0, v0], [u0, v1], [u1, v1], [u1, v0]];
    for particle in emitter.particles() {
        let t = particle.progress();
        let half = lerp(emitter.size.0, emitter.size.1, t).max(0.0) * 0.5;
        if half <= 0.0 {
            continue;
        }
        let color = std::array::from_fn(|i| lerp(emitter.color_start[i], emitter.color_end[i], t));
        let p = particle.position;
        let corners = [
            p + Vec2::new(-half, half),
            p + Vec2::new(-half, -half),
            p + Vec2::new(half, -half),
            p + Vec2::new(half, half),
        ];
        batch.push_quad(place, corners, uvs, color, emitter.texture_handle);
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
