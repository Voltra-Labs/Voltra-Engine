//! A full-screen pass over the drawn scene: colour grading and a vignette.
//!
//! The last pass of the frame's [`RenderGraph`](crate::graph::RenderGraph):
//! it reads the drawn scene and, when bloom is on, the glow, and writes the
//! frame. What must not be graded — the HUD, the editor's overlay — is drawn
//! after it.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::bloom::BloomSettings;
use crate::shader;

/// How the frame is graded. [`PostSettings::NEUTRAL`] changes nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostSettings {
    /// Multiplies every colour; `1` leaves it.
    pub exposure: f32,
    /// Around a linear middle grey; `1` leaves it, `0` is flat grey.
    pub contrast: f32,
    /// `1` leaves colour, `0` is greyscale, above `1` is more vivid.
    pub saturation: f32,
    /// Multiplies each channel; white leaves it.
    pub tint: [f32; 3],
    /// How far the corners darken, `0` to `1`.
    pub vignette: f32,
    /// Glow around what is bright, added before the grade.
    pub bloom: BloomSettings,
}

impl PostSettings {
    /// Settings that leave the picture unchanged.
    pub const NEUTRAL: Self = Self {
        exposure: 1.0,
        contrast: 1.0,
        saturation: 1.0,
        tint: [1.0, 1.0, 1.0],
        vignette: 0.0,
        bloom: BloomSettings::OFF,
    };
}

impl Default for PostSettings {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct PostUniform {
    tint_exposure: [f32; 4],
    grade: [f32; 4],
    /// x: bloom intensity; the rest unused.
    bloom: [f32; 4],
}

impl PostUniform {
    fn new(settings: &PostSettings, aspect: f32) -> Self {
        let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        Self {
            tint_exposure: [
                finite(settings.tint[0], 1.0).max(0.0),
                finite(settings.tint[1], 1.0).max(0.0),
                finite(settings.tint[2], 1.0).max(0.0),
                finite(settings.exposure, 1.0).max(0.0),
            ],
            grade: [
                finite(settings.contrast, 1.0).max(0.0),
                finite(settings.saturation, 1.0).max(0.0),
                finite(settings.vignette, 0.0).clamp(0.0, 1.0),
                finite(aspect, 1.0).max(1e-3),
            ],
            bloom: [
                finite(settings.bloom.intensity, 0.0).max(0.0),
                0.0,
                0.0,
                0.0,
            ],
        }
    }
}

/// The pipeline, its uniform and its sampler. One per renderer.
pub struct PostPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    /// Bound in the glow's place when there is none: black adds nothing.
    no_bloom: wgpu::TextureView,
}

impl PostPass {
    /// A pass writing `format`, which must be the target's.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = shader::create_module(device, "post", shader::POST);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Replaces, not blends: the pass writes every pixel.
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("post-uniform"),
            contents: bytemuck::bytes_of(&PostUniform::new(&PostSettings::NEUTRAL, 1.0)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("post-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let no_bloom = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("post-no-bloom"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Zeroed by wgpu on creation: black.
                format: crate::bloom::BLOOM_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            pipeline,
            layout,
            uniform,
            sampler,
            no_bloom,
        }
    }

    /// Grades `source` — an sRGB view, so the shader reads linear values —
    /// into `target`, every pixel replaced.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::TextureView,
        bloom: Option<&wgpu::TextureView>,
        target: &wgpu::TextureView,
        settings: &PostSettings,
        aspect: f32,
    ) {
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&PostUniform::new(settings, aspect)),
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post-bind-group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(bloom.unwrap_or(&self.no_bloom)),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("post-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonsense_settings_become_safe_ones() {
        let wild = PostSettings {
            exposure: f32::NAN,
            contrast: -3.0,
            saturation: f32::INFINITY,
            tint: [-1.0, f32::NAN, 2.0],
            vignette: 7.0,
            bloom: BloomSettings {
                intensity: -2.0,
                ..BloomSettings::OFF
            },
        };
        let u = PostUniform::new(&wild, f32::NAN);
        assert_eq!(u.tint_exposure, [0.0, 1.0, 2.0, 1.0]);
        assert_eq!(u.grade, [0.0, 1.0, 1.0, 1.0]);
        assert_eq!(u.bloom[0], 0.0);
    }
}
