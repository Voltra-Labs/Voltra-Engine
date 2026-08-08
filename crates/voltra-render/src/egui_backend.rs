//! Draws egui's tessellated output.
//!
//! `egui-wgpu` exists and does this, but it is pinned to wgpu 29 and would drag
//! a second, incompatible copy of wgpu into the build — its `Device` would be a
//! different type from ours. So the backend is ours.
//!
//! Only `epaint` is needed here, never `egui` itself: this module deals in
//! triangles and texture deltas, and knows nothing about widgets or layout.
//! That keeps the UI's shape a concern of the crate that builds it.

mod buffer;
mod draw;
mod texture;

use std::collections::HashMap;

use epaint::textures::{TextureOptions, TexturesDelta};
use epaint::{ClippedPrimitive, Primitive, TextureId};

use crate::shader;
use buffer::{empty_buffer, write_growable, Locals};
use draw::{scissor_rect, DrawCommand};
use texture::BoundTexture;

/// Where egui is being drawn, in physical pixels plus the scale that maps
/// egui's logical points onto them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenDescriptor {
    pub width: u32,
    pub height: u32,
    /// Physical pixels per logical point — the display's scale factor.
    pub pixels_per_point: f32,
}

impl ScreenDescriptor {
    /// The size egui lays out against, which is the physical size divided by
    /// the scale factor.
    pub fn size_in_points(&self) -> [f32; 2] {
        [
            self.width as f32 / self.pixels_per_point,
            self.height as f32 / self.pixels_per_point,
        ]
    }
}

/// Renders egui. Feed it `prepare`, then `render` inside a pass.
pub struct EguiBackend {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,

    textures: HashMap<TextureId, BoundTexture>,
    samplers: HashMap<TextureOptions, wgpu::Sampler>,
    /// Freed one frame late — see [`EguiBackend::prepare`].
    pending_free: Vec<TextureId>,
    next_user_id: u64,

    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    draws: Vec<DrawCommand>,
}

impl EguiBackend {
    /// `format` must match the attachment this will draw into.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = shader::create_module(device, "egui", shader::EGUI);

        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("egui-locals"),
            size: size_of::<Locals>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("egui-locals-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui-locals-bind-group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        // Deliberately the same layout the sprite pipeline uses: a texture at 0
        // and its sampler at 1. That is what lets a `RenderTarget`'s bind group
        // be handed straight to egui as the viewport image.
        let texture_layout = crate::texture::bind_group_layout(device);

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("egui-layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("egui-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<epaint::Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    // pos, uv, then the colour as four packed sRGB bytes.
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // egui emits both windings, so culling would eat half the UI.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(if format.is_srgb() {
                    "fs_main_srgb_target"
                } else {
                    "fs_main_unorm_target"
                }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Not `ALPHA_BLENDING`: egui's colours arrive with alpha
                    // already multiplied in, so the source must not be scaled
                    // by it a second time. The alpha channel accumulates
                    // coverage instead, which matters when egui draws into a
                    // transparent target.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            uniform,
            uniform_bind_group,
            texture_layout,
            textures: HashMap::new(),
            samplers: HashMap::new(),
            pending_free: Vec::new(),
            next_user_id: 0,
            vertices: empty_buffer(device, "egui-vertices", wgpu::BufferUsages::VERTEX),
            indices: empty_buffer(device, "egui-indices", wgpu::BufferUsages::INDEX),
            draws: Vec::new(),
        }
    }

    /// Uploads this frame's textures and geometry. Call before [`Self::render`].
    ///
    /// `delta.free` is not applied now but on the *next* call. egui's contract
    /// is that a freed texture is dead only once the frame that reported it has
    /// been drawn, and `primitives` here may still reference one.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        primitives: &[ClippedPrimitive],
        delta: &TexturesDelta,
        screen: ScreenDescriptor,
    ) {
        for id in self.pending_free.drain(..) {
            self.textures.remove(&id);
        }
        self.pending_free.extend_from_slice(&delta.free);

        for (id, image_delta) in &delta.set {
            self.set_texture(device, queue, *id, image_delta);
        }

        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Locals {
                screen_size: screen.size_in_points(),
                _padding: [0.0; 2],
            }),
        );

        self.draws.clear();
        let mut vertices: Vec<epaint::Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for ClippedPrimitive {
            clip_rect,
            primitive,
        } in primitives
        {
            let Primitive::Mesh(mesh) = primitive else {
                // Paint callbacks let a widget issue its own draw calls. The
                // editor shows the viewport as a registered texture instead, so
                // there is nothing here to honour yet.
                log::warn!("egui paint callbacks are not supported; primitive skipped");
                continue;
            };

            let scissor = scissor_rect(clip_rect, screen);
            // A fully clipped mesh still costs a draw call, and a zero-sized
            // scissor is a validation error on some backends.
            if scissor[2] == 0 || scissor[3] == 0 || mesh.indices.is_empty() {
                continue;
            }

            let start = indices.len() as u32;
            indices.extend_from_slice(&mesh.indices);
            self.draws.push(DrawCommand {
                texture: mesh.texture_id,
                indices: start..indices.len() as u32,
                base_vertex: vertices.len() as i32,
                scissor,
            });
            vertices.extend_from_slice(&mesh.vertices);
        }

        write_growable(
            device,
            queue,
            &mut self.vertices,
            "egui-vertices",
            wgpu::BufferUsages::VERTEX,
            bytemuck::cast_slice(&vertices),
        );
        write_growable(
            device,
            queue,
            &mut self.indices,
            "egui-indices",
            wgpu::BufferUsages::INDEX,
            bytemuck::cast_slice(&indices),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: ScreenDescriptor = ScreenDescriptor {
        width: 800,
        height: 600,
        pixels_per_point: 2.0,
    };

    #[test]
    fn points_convert_to_pixels_by_the_scale_factor() {
        assert_eq!(SCREEN.size_in_points(), [400.0, 300.0]);
    }

    #[test]
    fn vertex_layout_matches_epaint() {
        // pos and uv are two f32s each, colour is four packed bytes.
        assert_eq!(size_of::<epaint::Vertex>(), 20);
    }
}
