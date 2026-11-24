//! The scene's light layers: the sprite batch and the GPU emitters drawn
//! again into a buffer the light pass reads, for lights that leave some
//! layers out.
//!
//! Godot's `light_mask` and a light's item cull mask, as bits: a surface is
//! lit by a light only where the two share one. Stored inverted — the
//! layers a texel is **not** on — so the zero a buffer is cleared to, and
//! the zeroed stand-in bound when the pass did not run, put everything on
//! every layer. Recorded only in a lit frame where some light leaves a
//! layer out or some surface is on none; see `sprite_layers.wgsl`.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use wgpu::util::DeviceExt;

use crate::mesh::Mesh;
use crate::particles::ParticlePipelines;
use crate::pipeline::{SpritePipeline, SpritePipelineDescriptor};
use crate::scene_draw::SceneDraw;
use crate::shader;

/// The buffer's format: one `u32` of excluded layers per texel.
pub const LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;

/// Draws the batch's light layers. One per renderer.
pub struct LayerPass {
    pipeline: SpritePipeline,
    /// GPU emitters' particles, through their own vertex stage; `None`
    /// without GPU particles, where every emitter is in the batch.
    particles: Option<wgpu::RenderPipeline>,
    layout: wgpu::BindGroupLayout,
    /// One small uniform per mask ever drawn, kept: a game uses a handful,
    /// and making them afresh each frame would allocate every frame — on the
    /// web, memory only the collector frees (`frame_buffers.rs`).
    groups: Mutex<HashMap<u32, wgpu::BindGroup>>,
}

impl LayerPass {
    /// A pass drawing through `camera_layout`, reading each range's colour
    /// through `texture_layout` for its coverage, and GPU emitters through
    /// `particles`' draw layout when there are any.
    pub fn new(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
        particles: Option<&ParticlePipelines>,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite-layers-range"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let module = shader::create_module(device, "sprite-layers", shader::SPRITE_LAYERS);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite-layers-layout"),
            bind_group_layouts: &[Some(camera_layout), Some(texture_layout), Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = SpritePipeline::new(
            device,
            &SpritePipelineDescriptor {
                label: "sprite-layers",
                layout: &pipeline_layout,
                module: &module,
                triangles_entry: "vs_layers",
                quads_entry: "vs_layers_quad",
                fragment_entry: "fs_layers",
                // Integer targets do not blend: the last surface wins.
                target: wgpu::ColorTargetState {
                    format: LAYER_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                },
            },
        );
        let particles = particles.map(|particles| {
            particle_pipeline(
                device,
                [
                    camera_layout,
                    texture_layout,
                    &particles.draw_layout,
                    &layout,
                ],
            )
        });
        Self {
            pipeline,
            particles,
            layout,
            groups: Mutex::default(),
        }
    }

    /// Whether `draws` hold a surface on no light layer at all: lit by no
    /// light, even one of every layer — which only the buffer can say.
    pub fn any_on_no_layer(draws: &[SceneDraw<'_>]) -> bool {
        draws.iter().any(|draw| layers_of(draw) == 0)
    }

    /// Clears `target` to "on every layer" and draws every mesh range and
    /// GPU emitter of `draws` into it, in order — emitters only when the
    /// pass was made with particle pipelines, as the scene pass draws them.
    pub fn record(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        camera: &wgpu::BindGroup,
        mesh: Option<&Mesh>,
        draws: &[SceneDraw<'_>],
    ) {
        let mut groups = self.groups.lock().unwrap_or_else(PoisonError::into_inner);
        for draw in draws {
            let layers = layers_of(draw);
            groups.entry(layers).or_insert_with(|| {
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sprite-layers-range"),
                    contents: bytemuck::cast_slice(&[layers, 0, 0, 0]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("sprite-layers-range"),
                    layout: &self.layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                })
            });
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sprite-layers-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        // Which pipeline is bound, so a run of ranges binds it once.
        #[derive(PartialEq)]
        enum Bound {
            Nothing,
            Triangles,
            Quads,
            Particles,
        }
        let mut bound = Bound::Nothing;
        for draw in draws {
            let group = &groups[&layers_of(draw)];
            match draw {
                SceneDraw::Mesh(range) => {
                    let Some(mesh) = mesh else {
                        continue;
                    };
                    let kind = if range.primitives.is_quads() {
                        Bound::Quads
                    } else {
                        Bound::Triangles
                    };
                    if bound != kind {
                        pass.set_pipeline(self.pipeline.for_primitives(&range.primitives));
                        pass.set_bind_group(0, camera, &[]);
                        bound = kind;
                    }
                    pass.set_bind_group(1, range.texture, &[]);
                    pass.set_bind_group(2, group, &[]);
                    mesh.draw_primitives(&mut pass, &range.primitives);
                }
                SceneDraw::Particles {
                    particles, texture, ..
                } => {
                    let Some(pipeline) = &self.particles else {
                        continue;
                    };
                    if bound != Bound::Particles {
                        pass.set_pipeline(pipeline);
                        pass.set_bind_group(0, camera, &[]);
                        bound = Bound::Particles;
                    }
                    pass.set_bind_group(1, *texture, &[]);
                    pass.set_bind_group(3, group, &[]);
                    particles.draw(&mut pass);
                }
                // Not drawn into the layer buffer yet: a skinned mesh is lit
                // as if on every layer (stage 103's note).
                SceneDraw::Skinned { .. } => {}
            }
        }
    }
}

/// The light layers a draw's surface is on.
fn layers_of(draw: &SceneDraw<'_>) -> u32 {
    match draw {
        SceneDraw::Mesh(range) => range.light_layers,
        SceneDraw::Particles { light_layers, .. } => *light_layers,
        SceneDraw::Skinned { light_layers, .. } => *light_layers,
    }
}

/// The GPU emitters' pipeline: `particles_draw.wgsl`'s vertex stage, the
/// layers fragment appended to it, through `layouts` (camera, texture,
/// particles, layers).
fn particle_pipeline(
    device: &wgpu::Device,
    layouts: [&wgpu::BindGroupLayout; 4],
) -> wgpu::RenderPipeline {
    let source = format!("{}\n{}", shader::PARTICLES_DRAW, shader::PARTICLE_LAYERS);
    let module = shader::create_module(device, "particle-layers", &source);
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("particle-layers-layout"),
        bind_group_layouts: &layouts.map(Some),
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("particle-layers"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_layers"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: LAYER_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
