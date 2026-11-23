//! The shadow map's pipeline: draws casting edges into it, and binds it —
//! or an empty stand-in — for the accumulate pass.

use super::plan::{ShadowInstance, ShadowPlan};
use super::{QUARTERS, SHADOW_FORMAT};
use crate::frame_buffers::FrameBuffers;
use crate::shader;

/// Vertices per casting edge: a quad (two triangles) in each quarter.
const VERTICES: u32 = 6 * QUARTERS;

/// Draws casting edges into the shadow map, and binds it — or an empty
/// stand-in when there is none — for the accumulate pass.
pub(in crate::lights) struct ShadowPass {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    /// Bound when the frame has no shadow map. No light reads it: each is
    /// drawn with no row.
    empty: wgpu::BindGroup,
}

impl ShadowPass {
    pub(in crate::lights) fn new(device: &wgpu::Device) -> Self {
        let module = shader::create_module(device, "light-shadows", shader::LIGHT_SHADOWS);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("light-shadows-pipeline-layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        // `Min` takes no factors but one: the nearest edge is the shadow.
        let nearest = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Min,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("light-shadows"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_edge"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ShadowInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32,
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_edge"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SHADOW_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: nearest,
                        alpha: nearest,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("light-shadow-map-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let stand_in = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("light-shadow-map-empty"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SHADOW_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let empty = Self::bind(
            device,
            &layout,
            &stand_in.create_view(&wgpu::TextureViewDescriptor::default()),
        );
        Self {
            pipeline,
            layout,
            empty,
        }
    }

    /// The layout the accumulate pass reads the map through.
    pub(in crate::lights) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The stand-in bound when a frame has no shadow map.
    pub(in crate::lights) fn empty(&self) -> &wgpu::BindGroup {
        &self.empty
    }

    /// `map`, bound for the accumulate pass.
    pub(in crate::lights) fn bind_map(
        &self,
        device: &wgpu::Device,
        map: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        Self::bind(device, &self.layout, map)
    }

    fn bind(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light-shadow-map"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            }],
        })
    }

    /// The plan's instances, uploaded; `None` when there are none.
    pub(in crate::lights) fn upload(
        &self,
        device: &wgpu::Device,
        buffers: &FrameBuffers,
        plan: &ShadowPlan,
    ) -> Option<wgpu::Buffer> {
        (!plan.instances.is_empty()).then(|| {
            buffers.create(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some("light-shadow-edges"),
                    contents: bytemuck::cast_slice(&plan.instances),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            )
        })
    }

    /// Clears the map at `target` to "nothing in reach" and draws `count`
    /// casting edges from `buffer` into it.
    pub(in crate::lights) fn record(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        buffer: &wgpu::Buffer,
        count: u32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("light-shadows-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..VERTICES, 0..count);
    }
}
