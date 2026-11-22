//! The world's draw list: the batch's texture runs, with each GPU emitter's
//! draw between them where the batch's sort put it.

use voltra_assets::Textures;
use voltra_render::{wgpu, MeshDraw, SceneDraw};
use voltra_scene::SpriteBatch;

use super::gpu_particles::GpuParticleRings;

/// `batch`'s ranges and GPU emitters, in painter's order.
///
/// An emitter without a ring — made on the next step, or never on an
/// adapter without compute — is left out rather than drawn from nothing.
pub(super) fn scene_draws<'a>(
    batch: &SpriteBatch,
    textures: &'a Textures,
    white: &'a wgpu::BindGroup,
    rings: &'a GpuParticleRings,
) -> Vec<SceneDraw<'a>> {
    let bind = |texture| match texture {
        Some(handle) => textures.bind_group(handle),
        None => white,
    };
    let mut emitters = batch.gpu_emitters.iter().peekable();
    let mut draws = Vec::with_capacity(batch.ranges.len() + batch.gpu_emitters.len());
    for index in 0..=batch.ranges.len() {
        while let Some(emitter) = emitters.next_if(|emitter| emitter.before <= index) {
            if let Some(particles) = rings.get(emitter.entity) {
                draws.push(SceneDraw::Particles {
                    particles,
                    texture: bind(emitter.texture),
                    light_layers: emitter.light_layers,
                });
            }
        }
        if let Some(range) = batch.ranges.get(index) {
            draws.push(SceneDraw::Mesh(MeshDraw {
                texture: bind(range.texture),
                // A normal map that would not load is drawn flat, not as
                // the checker's colours read as normals.
                normal: range
                    .normal
                    .filter(|&handle| !textures.is_broken(handle))
                    .map(|handle| textures.raw_bind_group(handle)),
                light_layers: range.light_layers,
                indices: range.indices.clone(),
            }));
        }
    }
    draws
}
