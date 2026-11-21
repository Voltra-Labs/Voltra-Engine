//! The scene's light layers: the sprite batch drawn again into a buffer the
//! light pass reads, for lights that leave some layers out.
//!
//! Godot's `light_mask` and a light's item cull mask, as bits: a surface is
//! lit by a light only where the two share one. Stored inverted — the
//! layers a texel is **not** on — so the zero a buffer is cleared to, and
//! the zeroed stand-in bound when the pass did not run, put everything on
//! every layer. Recorded only in a lit frame where some light leaves a
//! layer out or some surface is on none; see `sprite_layers.wgsl`.

use std::collections::HashMap;

use wgpu::util::DeviceExt;

use crate::mesh::{Mesh, Vertex};
use crate::scene_draw::SceneDraw;
use crate::shader;

/// The buffer's format: one `u32` of excluded layers per texel.
pub const LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;

/// Draws the batch's light layers. One per renderer.
pub struct LayerPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl LayerPass {
    /// A pass drawing through `camera_layout`, reading each range's colour
    /// through `texture_layout` for its coverage.
    pub fn new(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
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
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite-layers"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_layers"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::LAYOUT)],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_layers"),
                compilation_options: Default::default(),
                // Integer targets do not blend: the last surface wins.
                targets: &[Some(wgpu::ColorTargetState {
                    format: LAYER_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, layout }
    }

    /// Whether `draws` hold a surface on no light layer at all: lit by no
    /// light, even one of every layer — which only the buffer can say.
    pub fn any_on_no_layer(draws: &[SceneDraw<'_>]) -> bool {
        draws
            .iter()
            .any(|draw| matches!(draw, SceneDraw::Mesh(range) if range.light_layers == 0))
    }

    /// Clears `target` to "on every layer" and draws every mesh range of
    /// `draws` into it, in order. GPU emitters are not drawn: the pixels
    /// they cover keep the layers of what is under them.
    pub fn record(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        camera: &wgpu::BindGroup,
        mesh: Option<&Mesh>,
        draws: &[SceneDraw<'_>],
    ) {
        // One small uniform per distinct mask this frame; a scene has few.
        let mut groups: HashMap<u32, wgpu::BindGroup> = HashMap::new();
        for draw in draws {
            if let SceneDraw::Mesh(range) = draw {
                groups.entry(range.light_layers).or_insert_with(|| {
                    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("sprite-layers-range"),
                        contents: bytemuck::cast_slice(&[range.light_layers, 0, 0, 0]),
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
        let Some(mesh) = mesh else {
            return;
        };
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera, &[]);
        for draw in draws {
            if let SceneDraw::Mesh(range) = draw {
                pass.set_bind_group(1, range.texture, &[]);
                pass.set_bind_group(2, &groups[&range.light_layers], &[]);
                mesh.draw_range(&mut pass, range.indices.clone());
            }
        }
    }
}
