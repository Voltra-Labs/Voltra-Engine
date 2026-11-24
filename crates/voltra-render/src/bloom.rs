//! Bloom: light past a threshold spread into a glow, as passes of the
//! frame's [`RenderGraph`].
//!
//! A prefilter keeps what is bright and halves the size; downsamples halve
//! it again level by level; upsamples walk back up, each added onto the
//! level above. The result, at half the frame's size, is what the post pass
//! adds to the scene. Off — intensity zero — the graph culls every pass.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::graph::{RenderGraph, Slot, TextureDesc};
use crate::shader;

/// The bloom levels' format: linear, and room above one for light that adds
/// up.
pub const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Levels below the prefilter at most; a level is never smaller than
/// [`MIN_SIZE`] texels on its short side.
const MAX_LEVELS: usize = 6;
const MIN_SIZE: u32 = 8;

/// How much glow, from what.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BloomSettings {
    /// How strongly the glow is added; `0` is off.
    pub intensity: f32,
    /// Linear brightness where bloom begins.
    pub threshold: f32,
    /// How gradually it begins, `0` (a hard edge) to `1`.
    pub softness: f32,
    /// How far each upsample reaches, in texels of its source; `1` is the
    /// tight default, larger is a wider, softer glow.
    pub radius: f32,
}

impl BloomSettings {
    /// Bloom switched off: zero intensity, with the other settings at their defaults.
    pub const OFF: Self = Self {
        intensity: 0.0,
        threshold: 0.8,
        softness: 0.5,
        radius: 1.0,
    };

    /// Whether any glow is drawn.
    pub fn is_on(&self) -> bool {
        self.intensity.is_finite() && self.intensity > 0.0
    }
}

impl Default for BloomSettings {
    fn default() -> Self {
        Self::OFF
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BloomUniform {
    params: [f32; 4],
}

impl BloomUniform {
    fn new(settings: &BloomSettings) -> Self {
        let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        let threshold = finite(settings.threshold, 0.8).max(0.0);
        let softness = finite(settings.softness, 0.5).clamp(0.0, 1.0);
        Self {
            params: [
                threshold,
                // A knee of zero would divide by zero in the shader's ease.
                (threshold * softness).max(1e-4),
                finite(settings.radius, 1.0).clamp(0.25, 4.0),
                0.0,
            ],
        }
    }
}

/// The level sizes of a chain under a `width` × `height` frame: the
/// prefilter's first, each half the last.
fn levels(width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut sizes = Vec::new();
    let (mut w, mut h) = (width / 2, height / 2);
    while w.min(h) >= MIN_SIZE && sizes.len() <= MAX_LEVELS {
        sizes.push((w, h));
        (w, h) = (w / 2, h / 2);
    }
    sizes
}

/// The three pipelines and what they bind. One per renderer.
pub struct BloomPass {
    prefilter: wgpu::RenderPipeline,
    downsample: wgpu::RenderPipeline,
    upsample: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
}

impl BloomPass {
    /// Builds the prefilter, downsample and upsample pipelines on `device`.
    pub fn new(device: &wgpu::Device) -> Self {
        let module = shader::create_module(device, "bloom", shader::BLOOM);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom-layout"),
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
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
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
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: BLOOM_FORMAT,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        let prefilter = pipeline("fs_prefilter", None);
        let downsample = pipeline("fs_downsample", None);
        let upsample = pipeline("fs_upsample", Some(additive));
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bloom-uniform"),
            contents: bytemuck::bytes_of(&BloomUniform::new(&BloomSettings::OFF)),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("bloom-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            prefilter,
            downsample,
            upsample,
            layout,
            uniform,
            sampler,
        }
    }

    /// Adds the chain over `source` — the drawn scene, read linear, at
    /// `width` × `height` — to `graph`, returning the slot holding the glow.
    ///
    /// `None` when bloom is off or the frame is too small for one level; the
    /// caller then adds nothing.
    #[allow(clippy::too_many_arguments)]
    pub fn add<'a>(
        &'a self,
        graph: &mut RenderGraph<'a>,
        device: &'a wgpu::Device,
        queue: &wgpu::Queue,
        source: Slot,
        width: u32,
        height: u32,
        settings: &BloomSettings,
    ) -> Option<Slot> {
        if !settings.is_on() {
            return None;
        }
        let sizes = levels(width, height);
        if sizes.is_empty() {
            return None;
        }
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&BloomUniform::new(settings)),
        );
        let slots: Vec<Slot> = sizes
            .iter()
            .map(|&(width, height)| {
                graph.create(TextureDesc {
                    label: "bloom-level",
                    width,
                    height,
                    format: BLOOM_FORMAT,
                })
            })
            .collect();

        let mut from = source;
        for (level, &to) in slots.iter().enumerate() {
            let pipeline = if level == 0 {
                &self.prefilter
            } else {
                &self.downsample
            };
            graph.add_pass("bloom-down", &[from], &[to], move |encoder, views| {
                self.draw(
                    device,
                    encoder,
                    pipeline,
                    views.get(from),
                    views.get(to),
                    true,
                );
            });
            from = to;
        }
        for pair in slots.windows(2).rev() {
            let (to, from) = (pair[0], pair[1]);
            graph.add_pass("bloom-up", &[from, to], &[to], move |encoder, views| {
                self.draw(
                    device,
                    encoder,
                    &self.upsample,
                    views.get(from),
                    views.get(to),
                    false,
                );
            });
        }
        Some(slots[0])
    }

    fn draw(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
        clear: bool,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom-bind-group"),
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
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bloom-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear {
                        wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_halve_down_to_the_smallest_useful_size() {
        assert_eq!(levels(256, 128), [(128, 64), (64, 32), (32, 16), (16, 8)]);
        assert!(levels(10, 10).is_empty(), "too small for even one level");
        assert_eq!(levels(8192, 8192).len(), MAX_LEVELS + 1);
    }

    #[test]
    fn off_is_off_and_nonsense_is_safe() {
        assert!(!BloomSettings::OFF.is_on());
        assert!(!BloomSettings {
            intensity: f32::NAN,
            ..BloomSettings::OFF
        }
        .is_on());
        let u = BloomUniform::new(&BloomSettings {
            intensity: 1.0,
            threshold: -1.0,
            softness: 0.0,
            radius: f32::INFINITY,
        });
        assert_eq!(u.params[0], 0.0);
        assert!(u.params[1] > 0.0, "the knee never divides by zero");
        assert_eq!(u.params[2], 1.0);
    }
}
