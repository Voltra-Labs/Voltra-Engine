//! Render pipeline construction.

use crate::shader;

/// Builds the built-in flat-colour pipeline.
///
/// `format` must match the surface configuration the pipeline will render
/// into; a mismatch is a validation error at draw time, not at creation.
pub fn create_flat_color(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let module = shader::create_module(device, "flat-color", shader::FLAT_COLOR);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("flat-color-pipeline"),
        // `None` derives the layout from the shader. The triangle has no
        // bindings yet; an explicit layout lands with the first uniform.
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            // No vertex buffers: the shader generates positions from
            // @builtin(vertex_index).
            buffers: &[],
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
