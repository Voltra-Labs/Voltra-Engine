//! An emitter's particles as quads in the sprite batch.

use voltra_assets::Atlases;
use voltra_render::glam::{Mat3, Vec2};

use super::look::{color_at, size_at};
use super::ParticleEmitter;
use crate::batch::{SpriteBatch, Surface};
use crate::sprite::quad::WHOLE;

/// Appends one quad per live particle, sized, tinted, turned and framed by
/// how far through its life it is — what `particles_draw.wgsl` does for a
/// GPU emitter. `matrix` is the emitter's world matrix, used only for a
/// local-space emitter, since a world-space one already stores world
/// positions. `atlases` resolves a flipbook's frames; without them, or
/// without a flipbook, every particle draws the whole texture.
pub fn push(
    batch: &mut SpriteBatch,
    matrix: Mat3,
    emitter: &ParticleEmitter,
    atlases: Option<&Atlases>,
) {
    if emitter.particles().is_empty() {
        return;
    }
    let place = if emitter.world_space {
        Mat3::IDENTITY
    } else {
        matrix
    };
    let whole = [WHOLE.0[0], WHOLE.0[1], WHOLE.1[0], WHOLE.1[1]];
    let frames = emitter
        .flipbook
        .as_ref()
        .map(|flipbook| (flipbook.uvs(atlases), flipbook.timing));
    let surface = Surface::textured(emitter.texture_handle).on(emitter.light_layers);
    for particle in emitter.particles() {
        let t = particle.progress();
        let half = size_at(emitter, t).max(0.0) * 0.5;
        if half.is_nan() || half <= 0.0 {
            continue;
        }
        let color = color_at(emitter, t);
        let rect = match &frames {
            Some((uvs, timing)) if !uvs.is_empty() => uvs[timing.index(particle.age, t, uvs.len())],
            _ => whole,
        };
        let [u0, v0, u1, v1] = rect;
        let uvs = [[u0, v0], [u0, v1], [u1, v1], [u1, v0]];
        let turn = Vec2::from_angle(particle.rotation);
        let p = particle.position;
        let corners = [
            Vec2::new(-half, half),
            Vec2::new(-half, -half),
            Vec2::new(half, -half),
            Vec2::new(half, half),
        ]
        .map(|corner| p + turn.rotate(corner));
        batch.push_quad_on(place, corners, uvs, color, surface);
    }
}
