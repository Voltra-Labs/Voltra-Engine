//! The world's draw list: the batch's texture runs, with each GPU emitter's
//! draw between them where the batch's sort put it.

use voltra_assets::{Materials, Textures};
use voltra_render::material::{MaterialBank, MaterialRequest};
use voltra_render::{wgpu, MeshDraw, Renderer, SceneDraw};
use voltra_scene::SpriteBatch;

use super::gpu_particles::GpuParticleRings;
use super::skinned_meshes::SkinnedMeshes;

/// `batch`'s ranges, GPU emitters and skinned meshes, in painter's order.
///
/// An emitter without a ring — made on the next step, or never on an
/// adapter without compute — is left out rather than drawn from nothing;
/// so is a skinned sprite with no mesh to draw.
pub(super) fn scene_draws<'a>(
    batch: &SpriteBatch,
    textures: &'a Textures,
    white: &'a wgpu::BindGroup,
    (rings, skins): (&'a GpuParticleRings, &'a SkinnedMeshes),
    bank: Option<&'a MaterialBank>,
    skip: Option<voltra_assets::Handle<voltra_render::Texture>>,
) -> Vec<SceneDraw<'a>> {
    let bind = |texture| match texture {
        Some(handle) => textures.bind_group(handle),
        None => white,
    };
    let mut emitters = batch.gpu_emitters.iter().enumerate().peekable();
    let mut skinned = batch.skinned.iter().peekable();
    let mut draws =
        Vec::with_capacity(batch.ranges.len() + batch.gpu_emitters.len() + batch.skinned.len());
    for index in 0..=batch.ranges.len() {
        // The emitters and skinned meshes due here, in the order the batch
        // met them: a skinned mesh after as many emitters as it says.
        loop {
            let emitter_due = emitters
                .peek()
                .filter(|(_, e)| e.before <= index)
                .map(|(n, _)| *n);
            let skin_due = skinned
                .peek()
                .filter(|s| s.before <= index)
                .map(|s| s.after_emitters);
            match (emitter_due, skin_due) {
                (Some(n), Some(after)) if after <= n => {}
                (Some(_), _) => {
                    if let Some((_, emitter)) = emitters.next() {
                        if let Some(particles) = rings.get(emitter.entity) {
                            draws.push(SceneDraw::Particles {
                                particles,
                                texture: bind(emitter.texture),
                                light_layers: emitter.light_layers,
                            });
                        }
                    }
                    continue;
                }
                (None, Some(_)) => {}
                (None, None) => break,
            }
            if let Some(draw) = skinned.next() {
                if let Some((mesh, pipeline)) = skins.get(draw.entity) {
                    draws.push(SceneDraw::Skinned {
                        mesh,
                        pipeline,
                        texture: bind(draw.texture),
                        light_layers: draw.light_layers,
                    });
                }
            }
        }
        if let Some(range) = batch
            .ranges
            .get(index)
            .filter(|r| skip.is_none() || r.texture != skip)
        {
            draws.push(SceneDraw::Mesh(MeshDraw {
                texture: bind(range.texture),
                // A normal map that would not load is drawn flat, not as
                // the checker's colours read as normals.
                normal: range
                    .normal
                    .filter(|&handle| !textures.is_broken(handle))
                    .map(|handle| textures.raw_bind_group(handle)),
                light_layers: range.light_layers,
                primitives: range.primitives.clone(),
                material: range
                    .material
                    .and_then(|index| bank.and_then(|bank| bank.draw(index))),
            }));
        }
    }
    draws
}

/// Compiles what `batch`'s materials need for a target of `format` and
/// uploads their uniforms, into `bank` — made on first use. A material the
/// store cannot draw with leaves its ranges plain.
pub(super) fn prepare_materials<'b>(
    bank: &'b mut Option<MaterialBank>,
    renderer: &Renderer,
    format: wgpu::TextureFormat,
    batch: &SpriteBatch,
    materials: Option<&Materials>,
) -> Option<&'b MaterialBank> {
    if batch.materials.is_empty() {
        return bank.as_ref();
    }
    let materials = materials?;
    let device = renderer.context().device();
    let bank = bank.get_or_insert_with(|| MaterialBank::new(device));
    let requests: Vec<Option<MaterialRequest<'_>>> = batch
        .materials
        .iter()
        .map(|used| {
            let material = materials.get(used.material)?;
            material.is_usable().then(|| MaterialRequest {
                revision: used.revision,
                source: &material.source,
                block: used.block,
            })
        })
        .collect();
    bank.prepare(
        device,
        renderer.context().queue(),
        format,
        [renderer.camera_layout(), renderer.texture_layout()],
        &requests,
    );
    Some(bank)
}
