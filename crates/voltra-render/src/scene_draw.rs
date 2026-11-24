//! The world's pass: sprite ranges and GPU particle emitters in one painter's
//! order.

use crate::mesh::Mesh;
use crate::particles::{GpuParticles, ParticlePipelines};
use crate::pass::{Binder, MeshDraw};
use crate::skinning::{SkinPipeline, SkinnedMesh};

/// One draw of the world, in the order it is drawn.
///
/// Sprites, tiles, text and CPU particles are ranges of the one sprite mesh;
/// a GPU emitter is its own instanced draw between them, wherever its
/// sorting layer and `sort_order` put it — painter's order does not care
/// which pipeline a draw takes.
pub enum SceneDraw<'a> {
    /// A range of the sprite mesh.
    Mesh(MeshDraw<'a>),
    /// A GPU particle emitter, drawn instanced.
    Particles {
        /// The emitter's particle buffer.
        particles: &'a GpuParticles,
        /// The texture its particles are drawn with.
        texture: &'a wgpu::BindGroup,
        /// The light layers its particles are on, as a mesh range's are.
        light_layers: u32,
    },
    /// A sprite mesh deformed by bones.
    Skinned {
        /// The mesh and its pose.
        mesh: &'a SkinnedMesh,
        /// The pipeline it is drawn through.
        pipeline: &'a SkinPipeline,
        /// The texture it is drawn with.
        texture: &'a wgpu::BindGroup,
        /// The light layers it is on, as a mesh range's are.
        light_layers: u32,
    },
}

/// Records a clear of `view`, then every one of `draws` in order: mesh
/// ranges through `sprites`, emitters through `particles`.
///
/// `camera` is bound to group 0 for both pipelines; a draw's texture to
/// group 1. Emitters are skipped when `particles` is `None` — an adapter
/// without compute, whose caller simulates them on the CPU instead — and
/// mesh ranges when there is no mesh. The clear is recorded either way, as
/// [`draw_mesh_batches`](crate::pass::draw_mesh_batches) does.
#[allow(clippy::too_many_arguments)]
pub fn draw_scene(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    sprites: &crate::pipeline::SpritePipeline,
    particles: Option<&ParticlePipelines>,
    camera: &wgpu::BindGroup,
    mesh: Option<&Mesh>,
    draws: &[SceneDraw<'_>],
    clear: wgpu::Color,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("scene-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    // Which pipeline is bound, so a run of ranges binds it once.
    let mut binder = Binder::new(sprites, camera);
    let mut particles_bound = false;
    for draw in draws {
        match draw {
            SceneDraw::Mesh(range) => {
                let Some(mesh) = mesh else {
                    continue;
                };
                binder.bind(&mut pass, range);
                particles_bound = false;
                mesh.draw_primitives(&mut pass, &range.primitives);
            }
            SceneDraw::Particles {
                particles: emitter,
                texture,
                ..
            } => {
                let Some(pipelines) = particles else {
                    continue;
                };
                if !particles_bound {
                    pass.set_pipeline(&pipelines.draw);
                    pass.set_bind_group(0, camera, &[]);
                    particles_bound = true;
                    binder.forget();
                }
                pass.set_bind_group(1, *texture, &[]);
                emitter.draw(&mut pass);
            }
            SceneDraw::Skinned {
                mesh: skinned,
                pipeline,
                texture,
                ..
            } => {
                pass.set_pipeline(&pipeline.pipeline);
                pass.set_bind_group(0, camera, &[]);
                pass.set_bind_group(1, *texture, &[]);
                particles_bound = false;
                binder.forget();
                skinned.draw(&mut pass);
            }
        }
    }
}
