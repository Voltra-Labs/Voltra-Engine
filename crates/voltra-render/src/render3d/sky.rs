//! The sky's pipeline: one triangle over a 3D target at the far plane,
//! drawn where nothing else was.

use super::depth::{DEPTH_COMPARE, DEPTH_FORMAT};

/// Draws the frame's sky ([`Lighting3D::sky`](super::Lighting3D::sky)):
/// a full-screen triangle at depth 0 — the far plane under reversed Z,
/// what depth is cleared to — tested `GreaterEqual` without writing, so it
/// lands only on pixels (and samples) nothing was drawn on. Drawn after the
/// opaque and masked draws, so the depth test spares the sky's pixels
/// under them, and before the blended, which blend over it.
#[derive(Debug)]
pub(super) struct SkyPipeline {
    pipeline: wgpu::RenderPipeline,
}

impl SkyPipeline {
    /// The pipeline into `format` at `samples`, its group 0 `frame`.
    pub fn new(
        device: &wgpu::Device,
        frame: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        samples: u32,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky-3d"),
            source: wgpu::ShaderSource::Wgsl(crate::shader::SKY_3D.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky-3d-layout"),
            bind_group_layouts: &[Some(frame)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky-3d"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_sky"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(DEPTH_COMPARE),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline }
    }

    /// Records the sky into `pass`, its group 0 already bound.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use wgpu::naga;

    #[test]
    fn the_sky_shader_validates_and_writes_as_webgl2_s_glsl() {
        let source = crate::shader::SKY_3D;
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
        crate::shader_check::webgl2(&module, &info).expect("WebGL2 runs it");
    }
}
