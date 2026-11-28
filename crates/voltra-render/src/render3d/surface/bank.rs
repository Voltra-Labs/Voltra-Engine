//! The surface pipelines, one per variant a draw needs, made on first use.

use std::collections::HashMap;

use super::layouts::SurfaceLayouts;
use super::standard::AlphaMode;
use crate::render3d::depth::{DEPTH_COMPARE, DEPTH_FORMAT};
use crate::render3d::instance::Instance3D;
use crate::render3d::vertex::Vertex3D;
use crate::render3d::winding::Winding;

/// What a surface's alpha does to its pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum AlphaClass {
    /// Written, depth-tested and written.
    #[default]
    Opaque,
    /// Cut out: discarded under the cutoff, or alpha to coverage under
    /// MSAA.
    Mask,
    /// Blended premultiplied, depth-tested, not written.
    Blend,
}

impl AlphaClass {
    /// The class of `mode`.
    pub fn of(mode: AlphaMode) -> Self {
        match mode {
            AlphaMode::Opaque => Self::Opaque,
            AlphaMode::Mask(_) => Self::Mask,
            AlphaMode::Blend => Self::Blend,
        }
    }
}

/// Which pass a surface pipeline draws for. The main colour pass now; the
/// shadow (140) and picking (146) passes are its later variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SurfacePass {
    /// The camera's colour and depth.
    #[default]
    Main,
}

/// What picks a surface pipeline, within a bank made for one target
/// format and sample count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceKey {
    /// Opaque, masked or blended.
    pub alpha: AlphaClass,
    /// Both faces drawn.
    pub double_sided: bool,
    /// Which way the instances' fronts wind ([`Winding::of`]).
    pub winding: Winding,
    /// The pass drawn for.
    pub pass: SurfacePass,
}

/// The standard surface's pipelines into one colour format at one sample
/// count, by [`SurfaceKey`]: made the first time a draw needs one, kept
/// for the bank's life (the variants are few: 3 alpha classes × 2 sides ×
/// 3 windings).
#[derive(Debug)]
pub struct SurfaceBank {
    module: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    samples: u32,
    pipelines: HashMap<SurfaceKey, wgpu::RenderPipeline>,
}

impl SurfaceBank {
    /// A bank drawing into `format`, `samples` a pixel, against `layouts`.
    pub fn new(
        device: &wgpu::Device,
        layouts: &SurfaceLayouts,
        format: wgpu::TextureFormat,
        samples: u32,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("surface-3d"),
            source: wgpu::ShaderSource::Wgsl(crate::shader::SURFACE_3D.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("surface-3d-layout"),
            bind_group_layouts: &[Some(&layouts.frame), Some(&layouts.material)],
            immediate_size: 0,
        });
        Self {
            module,
            layout,
            format,
            samples: samples.max(1),
            pipelines: HashMap::new(),
        }
    }

    /// The colour format it draws into.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// The samples a pixel it draws.
    pub fn samples(&self) -> u32 {
        self.samples
    }

    /// How many pipelines it holds.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Whether it holds none yet.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    /// Makes the pipeline for `key` if it is not made yet.
    pub fn prepare(&mut self, device: &wgpu::Device, key: SurfaceKey) {
        if !self.pipelines.contains_key(&key) {
            let pipeline = self.build(device, key);
            self.pipelines.insert(key, pipeline);
        }
    }

    /// The pipeline for `key`, if [`prepare`](Self::prepare) made it.
    pub fn pipeline(&self, key: SurfaceKey) -> Option<&wgpu::RenderPipeline> {
        self.pipelines.get(&key)
    }

    fn build(&self, device: &wgpu::Device, key: SurfaceKey) -> wgpu::RenderPipeline {
        let SurfacePass::Main = key.pass;
        let coverage = self.samples > 1;
        let (entry, blend, write) = match key.alpha {
            AlphaClass::Opaque => ("fs_opaque", None, true),
            AlphaClass::Mask if coverage => ("fs_mask_coverage", None, true),
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
        let cull_mode =
            (!key.double_sided && key.winding != Winding::Flat).then_some(wgpu::Face::Back);
        let label = format!("surface-3d-{key:?}");
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&label),
            layout: Some(&self.layout),
            vertex: wgpu::VertexState {
                module: &self.module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(Vertex3D::layout()), Some(Instance3D::layout())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.format,
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
                count: self.samples,
                mask: !0,
                alpha_to_coverage_enabled: key.alpha == AlphaClass::Mask && coverage,
            },
            multiview_mask: None,
            cache: None,
        })
    }
}
