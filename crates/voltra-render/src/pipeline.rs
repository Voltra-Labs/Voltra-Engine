//! Render pipeline construction.

use crate::mesh::Vertex;
use crate::shader;

/// Builds the built-in flat-colour pipeline.
///
/// `format` must match the surface configuration the pipeline will render
/// into; a mismatch is a validation error at draw time, not at creation.
/// `camera_layout` becomes bind group 0.
pub fn create_flat_color(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let module = shader::create_module(device, "flat-color", shader::FLAT_COLOR);

    // Declared rather than inferred from the shader: an explicit layout keeps
    // the binding contract ours instead of something naga reconstructs, and
    // makes a mismatch fail here rather than at the first draw.
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("flat-color-layout"),
        bind_group_layouts: &[Some(camera_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("flat-color-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(Vertex::LAYOUT)],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            // 2D geometry is always viewed from one side, so culling would
            // only cost a winding rule to get wrong. The 3D pipeline turns it
            // back on alongside the depth buffer.
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
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
