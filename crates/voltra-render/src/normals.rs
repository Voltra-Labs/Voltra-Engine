//! The scene's normals: the sprite batch drawn a second time, into a normal
//! buffer the light pass reads for lights with a height.
//!
//! A second pass rather than a second colour attachment on the scene pass:
//! the scene pass is the same with or without normal maps, and a frame with
//! no light that needs them pays nothing — the pass is not recorded. See
//! `sprite_normals.wgsl` for the encoding and the tangent frame.

use crate::mesh::{Mesh, Vertex};
use crate::scene_draw::SceneDraw;
use crate::texture::Texture;
use crate::{shader, Filter};

/// The normal buffer's format: `n * 0.5 + 0.5` in rgb, alpha 1 where
/// something was drawn. Unorm: the bytes are data, not colour.
pub const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// What the normal buffer is cleared to: facing the screen, nothing drawn.
pub const FLAT: wgpu::Color = wgpu::Color {
    r: 0.5,
    g: 0.5,
    b: 1.0,
    a: 0.0,
};

/// Draws the batch's normals. One per renderer.
pub struct NormalPass {
    pipeline: wgpu::RenderPipeline,
    /// Bound as the normal map of ranges that have none: facing the screen,
    /// so they cover what is under them as a plain sprite should.
    flat: wgpu::BindGroup,
}

impl NormalPass {
    /// A pass drawing through `camera_layout`, sampling colour and normal
    /// maps through `texture_layout` ([`crate::texture::bind_group_layout`]).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let module = shader::create_module(device, "sprite-normals", shader::SPRITE_NORMALS);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sprite-normals-layout"),
            bind_group_layouts: &[
                Some(camera_layout),
                Some(texture_layout),
                Some(texture_layout),
            ],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite-normals"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_normals"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex::LAYOUT)],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_normals"),
                compilation_options: Default::default(),
                // Replaced, not blended: the last thing drawn over a texel
                // is the surface the light sees there.
                targets: &[Some(wgpu::ColorTargetState {
                    format: NORMAL_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        // Facing the screen: (0, 0, 1).
        let flat = Texture::from_rgba8(
            device,
            queue,
            "flat-normal",
            &[128, 128, 255, 255],
            1,
            1,
            Filter::Nearest,
        )
        .expect("a 1x1 texture has a valid size")
        .create_raw_bind_group(device, texture_layout);
        Self { pipeline, flat }
    }

    /// Whether any of `draws` carries a normal map: without one the buffer
    /// would be flat everywhere and the pass is not worth recording.
    pub fn needed(draws: &[SceneDraw<'_>]) -> bool {
        draws
            .iter()
            .any(|draw| matches!(draw, SceneDraw::Mesh(range) if range.normal.is_some()))
    }

    /// Clears `target` to [`FLAT`] and draws every mesh range of `draws` into
    /// it, in order. GPU emitters are not drawn: particles have no normals,
    /// and the pixels they cover keep the normals of what is under them.
    pub fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        camera: &wgpu::BindGroup,
        mesh: Option<&Mesh>,
        draws: &[SceneDraw<'_>],
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sprite-normals-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(FLAT),
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
                pass.set_bind_group(2, range.normal.unwrap_or(&self.flat), &[]);
                mesh.draw_range(&mut pass, range.indices.clone());
            }
        }
    }
}
