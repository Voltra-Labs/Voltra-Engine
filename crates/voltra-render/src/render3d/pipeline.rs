//! The unlit instanced pipeline: flat-tinted meshes, depth-tested.

use super::depth::{DEPTH_COMPARE, DEPTH_FORMAT};
use super::instance::Instance3D;
use super::vertex::Vertex3D;

/// The pipeline the 3D pass draws every mesh with until surface materials
/// (stage 137): a flat tint, depth written and tested reversed, back faces
/// culled. Built for one colour format.
#[derive(Debug)]
pub struct UnlitPipeline3D {
    pipeline: wgpu::RenderPipeline,
    view_layout: wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
}

impl UnlitPipeline3D {
    /// The pipeline drawing into `format`.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("view-3d-layout"),
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
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("unlit-3d"),
            source: wgpu::ShaderSource::Wgsl(crate::shader::UNLIT_3D.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("unlit-3d-layout"),
            bind_group_layouts: &[Some(&view_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("unlit-3d"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex3D::layout()), Some(Instance3D::layout())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(DEPTH_COMPARE),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            view_layout,
            format,
        }
    }

    /// The pipeline.
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// The layout of group 0: the view's uniform.
    pub fn view_layout(&self) -> &wgpu::BindGroupLayout {
        &self.view_layout
    }

    /// The colour format it draws into.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}
