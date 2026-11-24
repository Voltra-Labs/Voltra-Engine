//! Render pipeline construction.

use crate::lines::LineVertex;
use crate::shader;

pub use sprite::{SpritePipeline, SpritePipelineDescriptor};

mod sprite;

/// Builds the built-in flat-colour pipeline, for triangles and quads.
///
/// `format` must match the surface configuration the pipeline will render
/// into; a mismatch is a validation error at draw time, not at creation.
/// `camera_layout` becomes bind group 0, `texture_layout` group 1.
pub fn create_flat_color(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
) -> SpritePipeline {
    let module = shader::create_module(device, "flat-color", shader::FLAT_COLOR);

    // Declared rather than inferred from the shader: an explicit layout keeps
    // the binding contract ours instead of something naga reconstructs, and
    // makes a mismatch fail here rather than at the first draw.
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("flat-color-layout"),
        bind_group_layouts: &[Some(camera_layout), Some(texture_layout)],
        immediate_size: 0,
    });

    SpritePipeline::new(
        device,
        &SpritePipelineDescriptor {
            label: "flat-color-pipeline",
            layout: &layout,
            module: &module,
            triangles_entry: "vs_main",
            quads_entry: "vs_quad",
            fragment_entry: "fs_main",
            target: wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            },
        },
    )
}

/// Builds the line pipeline.
///
/// Same shape as [`create_flat_color`] — `TriangleList`, no culling, no depth,
/// alpha blending — because a widened line *is* triangles. What differs is the
/// vertex layout and the shader that reads it.
pub fn create_lines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_layout: &wgpu::BindGroupLayout,
    viewport_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let module = shader::create_module(device, "lines", shader::LINES);

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("lines-layout"),
        bind_group_layouts: &[Some(camera_layout), Some(viewport_layout)],
        immediate_size: 0,
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("lines-pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(LineVertex::LAYOUT)],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            // A widened quad's winding flips with the direction its segment
            // runs in, so culling would drop every line that happened to point
            // the other way.
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
