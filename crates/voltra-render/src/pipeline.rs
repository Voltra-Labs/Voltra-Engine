//! Render pipeline construction.

use crate::lines::LineVertex;
use crate::mesh::Vertex;
use crate::shader;

/// Builds the built-in flat-colour pipeline.
///
/// `format` must match the surface configuration the pipeline will render
/// into; a mismatch is a validation error at draw time, not at creation.
/// `camera_layout` becomes bind group 0, `texture_layout` group 1.
pub fn create_flat_color(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    camera_layout: &wgpu::BindGroupLayout,
    texture_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let module = shader::create_module(device, "flat-color", shader::FLAT_COLOR);

    // Declared rather than inferred from the shader: an explicit layout keeps
    // the binding contract ours instead of something naga reconstructs, and
    // makes a mismatch fail here rather than at the first draw.
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("flat-color-layout"),
        bind_group_layouts: &[Some(camera_layout), Some(texture_layout)],
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

/// Layout of the viewport-size uniform the line shader widens with.
///
/// Group 1 — the slot the sprite pipeline gives its texture — so that group 0
/// is the camera in every pipeline this engine has.
pub fn viewport_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viewport-layout"),
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
    })
}

/// The viewport size, uploaded, with its bind group.
///
/// A `vec4` rather than a `vec2`: a uniform buffer binding is aligned to 16
/// bytes, and two unused floats are cheaper than a padding field nobody
/// remembers the reason for.
///
/// Both dimensions are clamped to at least one. A minimised window reports zero
/// and the shader divides by half of this, which would put a NaN through every
/// vertex of every line.
pub fn viewport_binding(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
) -> wgpu::BindGroup {
    let size = [width.max(1) as f32, height.max(1) as f32, 0.0, 0.0];
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("viewport-uniform"),
        size: size_of::<[f32; 4]>() as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&size));

    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viewport-bind-group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
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
