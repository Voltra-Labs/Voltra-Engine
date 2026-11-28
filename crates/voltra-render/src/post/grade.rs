//! Which of the post pass's shaders grades a picture, and the pipelines
//! made for the ones a frame asks for.

use std::sync::OnceLock;

use crate::render3d::Tonemap;
use crate::shader;

/// How a picture is graded by [`PostPass`](super::PostPass).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Grade {
    /// A 2D picture: exposed, tinted and graded as it is.
    #[default]
    Picture,
    /// A 2D overlay, drawn over another dimension's picture into a
    /// transparent target: its colour is premultiplied, so it is
    /// unpremultiplied, graded and premultiplied again — graded as it is,
    /// a soft edge would halo.
    Overlay,
    /// A 3D picture: linear HDR light, exposed, brought into range by the
    /// curve, then tinted and graded. Premultiplied the same way, alpha
    /// passed through, so a 3D overlay stays transparent where nothing was
    /// drawn.
    Tonemap(Tonemap),
}

/// The pipelines for [`Grade::Overlay`] and [`Grade::Tonemap`] (one for
/// every curve: the curve is a uniform), each made on first use.
pub(super) struct Variants {
    layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    overlay: OnceLock<wgpu::RenderPipeline>,
    tonemap: OnceLock<wgpu::RenderPipeline>,
}

impl Variants {
    pub(super) fn new(layout: wgpu::PipelineLayout, format: wgpu::TextureFormat) -> Self {
        Self {
            layout,
            format,
            overlay: OnceLock::new(),
            tonemap: OnceLock::new(),
        }
    }

    /// The pipeline for `grade`, which is not [`Grade::Picture`] (the pass
    /// holds that one itself).
    pub(super) fn get(&self, device: &wgpu::Device, grade: Grade) -> &wgpu::RenderPipeline {
        match grade {
            Grade::Tonemap(_) => self.tonemap.get_or_init(|| {
                let module = shader::create_module(device, "tonemap", shader::TONEMAP);
                pipeline(device, &self.layout, &module, "fs_tonemap", self.format)
            }),
            Grade::Picture | Grade::Overlay => self.overlay.get_or_init(|| {
                let module = shader::create_module(device, "post-overlay", shader::POST);
                pipeline(device, &self.layout, &module, "fs_overlay", self.format)
            }),
        }
    }
}

/// A full-screen pipeline drawing `module`'s `entry` into `format`,
/// replacing every pixel.
pub(super) fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    entry: &str,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
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
            module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                // Replaces, not blends: the pass writes every pixel.
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
