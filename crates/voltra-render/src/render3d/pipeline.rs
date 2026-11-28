//! The unlit instanced pipeline: flat-tinted meshes, depth-tested.

use super::depth::{DEPTH_COMPARE, DEPTH_FORMAT};
use super::instance::Instance3D;
use super::vertex::Vertex3D;
use super::winding::Winding;

/// The pipeline the 3D pass draws every mesh with until surface materials
/// (stage 137): a flat tint, depth written and tested reversed, back faces
/// culled. Built for one colour format, in a variant per [`Winding`]:
/// counter-clockwise fronts, clockwise for a mirrored instance (a negative
/// determinant turns its triangles over on screen), and no culling for a
/// model flattened to a plane. Every later 3D pipeline — material, shadow,
/// picking — keys on the same three.
#[derive(Debug)]
pub struct UnlitPipeline3D {
    /// One per [`Winding`], in [`Winding::ALL`]'s order.
    pipelines: [wgpu::RenderPipeline; 3],
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
        let build = |winding: Winding| {
            let (front_face, cull_mode, label) = match winding {
                Winding::CounterClockwise => {
                    (wgpu::FrontFace::Ccw, Some(wgpu::Face::Back), "unlit-3d")
                }
                Winding::Clockwise => (
                    wgpu::FrontFace::Cw,
                    Some(wgpu::Face::Back),
                    "unlit-3d-mirrored",
                ),
                Winding::Flat => (wgpu::FrontFace::Ccw, None, "unlit-3d-flat"),
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
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
                    front_face,
                    cull_mode,
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
            })
        };
        Self {
            pipelines: Winding::ALL.map(build),
            view_layout,
            format,
        }
    }

    /// The pipeline for instances of `winding` ([`Winding::of`]).
    pub fn pipeline(&self, winding: Winding) -> &wgpu::RenderPipeline {
        &self.pipelines[winding as usize]
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
