//! One surface pipeline, built for a key from a shader module.

use super::key::{AlphaClass, SurfaceKey, SurfacePass, SurfaceVertex};
use crate::render3d::depth::{DEPTH_COMPARE, DEPTH_FORMAT};
use crate::render3d::instance::Instance3D;
use crate::render3d::vertex::Vertex3D;
use crate::render3d::winding::Winding;

/// Alpha to coverage's blend: the colour replaces, the alpha keeps the
/// larger of the coverage written and what the sample held — so a covered
/// sample over an opaque clear stays opaque. Over a transparent overlay a
/// covered sample holds the coverage, and the resolved edge lets a little
/// of what is behind through (`coverage − coverage²` of it at most, a
/// quarter at half coverage): WebGL2 has no sample-mask output, so there is
/// no exact answer for both.
const MASK_COVERAGE_BLEND: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent::REPLACE,
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Max,
    },
};

/// The pipeline for `key` through `module`, into `format` at `samples`.
pub(super) fn build(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
    samples: u32,
    key: SurfaceKey,
) -> wgpu::RenderPipeline {
    let SurfacePass::Main = key.pass;
    let SurfaceVertex::Plain = key.vertex;
    let coverage = samples > 1;
    let (entry, blend, write) = match key.alpha {
        AlphaClass::Opaque => ("fs_opaque", None, true),
        AlphaClass::Mask if coverage => ("fs_mask_coverage", Some(MASK_COVERAGE_BLEND), true),
        AlphaClass::Mask => ("fs_mask", None, true),
        AlphaClass::Blend => (
            "fs_blend",
            Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            false,
        ),
    };
    let front_face = match key.winding {
        Winding::Clockwise => wgpu::FrontFace::Cw,
        Winding::CounterClockwise | Winding::Flat => wgpu::FrontFace::Ccw,
    };
    // A flat model's fronts cannot be trusted, and a double-sided
    // surface shows both: neither culls.
    let cull_mode = (!key.double_sided && key.winding != Winding::Flat).then_some(wgpu::Face::Back);
    let label = format!("surface-3d-{key:?}");
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(Vertex3D::layout()), Some(Instance3D::layout())],
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
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
            depth_write_enabled: Some(write),
            depth_compare: Some(DEPTH_COMPARE),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: samples,
            mask: !0,
            alpha_to_coverage_enabled: key.alpha == AlphaClass::Mask && coverage,
        },
        multiview_mask: None,
        cache: None,
    })
}
