//! The compute pipeline that steps particles and the render pipeline that
//! draws them, with the layouts both are bound through. One per renderer.

use crate::shader;

/// Both pipelines and their per-emitter layouts.
pub struct ParticlePipelines {
    pub(super) simulate: wgpu::ComputePipeline,
    pub(super) simulate_layout: wgpu::BindGroupLayout,
    pub(crate) draw: wgpu::RenderPipeline,
    pub(super) draw_layout: wgpu::BindGroupLayout,
}

impl ParticlePipelines {
    /// Pipelines drawing into `format`, through the sprite pipeline's
    /// camera and texture layouts — a particle is a sprite as far as
    /// groups 0 and 1 are concerned.
    ///
    /// Needs compute shaders and storage buffers in the vertex stage: see
    /// [`ParticlePipelines::supported`].
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        camera_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let simulate_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particles-simulate-layout"),
            entries: &[
                storage(0, wgpu::ShaderStages::COMPUTE, false),
                uniform(1, wgpu::ShaderStages::COMPUTE),
            ],
        });
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particles-draw-layout"),
            entries: &[
                storage(0, wgpu::ShaderStages::VERTEX, true),
                uniform(1, wgpu::ShaderStages::VERTEX),
            ],
        });

        let module = shader::create_module(device, "particles-simulate", shader::PARTICLES_SIM);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particles-simulate-pipeline-layout"),
            bind_group_layouts: &[Some(&simulate_layout)],
            immediate_size: 0,
        });
        let simulate = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("particles-simulate"),
            layout: Some(&layout),
            module: &module,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let module = shader::create_module(device, "particles-draw", shader::PARTICLES_DRAW);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particles-draw-pipeline-layout"),
            bind_group_layouts: &[
                Some(camera_layout),
                Some(texture_layout),
                Some(&draw_layout),
            ],
            immediate_size: 0,
        });
        let draw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("particles-draw"),
            layout: Some(&layout),
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
                    // The sprite pipeline's blend, so a particle looks the
                    // same simulated either way.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            simulate,
            simulate_layout,
            draw,
            draw_layout,
        }
    }

    /// Whether an adapter can run these: compute shaders, and storage
    /// buffers read from the vertex stage. WebGL2 and some GL drivers have
    /// neither; an emitter there falls back to the CPU.
    pub fn supported(flags: wgpu::DownlevelFlags) -> bool {
        flags.contains(wgpu::DownlevelFlags::COMPUTE_SHADERS | wgpu::DownlevelFlags::VERTEX_STORAGE)
    }
}

fn storage(
    binding: u32,
    visibility: wgpu::ShaderStages,
    read_only: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
