//! The surface pipelines, one per variant a draw needs, made on first use.

use std::collections::HashMap;

use super::layouts::SurfaceLayouts;
use super::shader::SurfaceShader;
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

/// What a surface pipeline's vertex stage reads. A mesh's own vertices
/// and the instance's placement now; skinned meshes (155) are its later
/// variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SurfaceVertex {
    /// [`Vertex3D`] and [`Instance3D`].
    #[default]
    Plain,
}

/// What picks a surface pipeline, within a bank made for one target
/// format and sample count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceKey {
    /// The shader's revision ([`SurfaceShader::revision`]): the standard
    /// one, or a custom material's.
    pub shader: u64,
    /// What the vertex stage reads.
    pub vertex: SurfaceVertex,
    /// Opaque, masked or blended.
    pub alpha: AlphaClass,
    /// Both faces drawn.
    pub double_sided: bool,
    /// Which way the instances' fronts wind ([`Winding::of`]).
    pub winding: Winding,
    /// The pass drawn for.
    pub pass: SurfacePass,
}

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

/// Uses ([`SurfaceBank::begin`] calls — one a shot) a custom shader's
/// module and pipelines are kept without being drawn with: a material
/// edited a hundred times does not keep a hundred. The standard shader's
/// are kept for the bank's life.
pub const KEEP_USES: u64 = 600;

/// The surface pipelines into one colour format at one sample count, by
/// [`SurfaceKey`]: made the first time a draw needs one. The standard
/// shader's variants are few (3 alpha classes × 2 sides × 3 windings) and
/// kept; a custom shader's are let go [`KEEP_USES`] shots after its last.
#[derive(Debug)]
pub struct SurfaceBank {
    layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    samples: u32,
    standard: u64,
    modules: HashMap<u64, Used<wgpu::ShaderModule>>,
    pipelines: HashMap<SurfaceKey, Used<wgpu::RenderPipeline>>,
    uses: u64,
}

#[derive(Debug)]
struct Used<T> {
    value: T,
    last: u64,
}

impl SurfaceBank {
    /// A bank drawing into `format`, `samples` a pixel, against `layouts`.
    pub fn new(
        device: &wgpu::Device,
        layouts: &SurfaceLayouts,
        format: wgpu::TextureFormat,
        samples: u32,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("surface-3d-layout"),
            bind_group_layouts: &[Some(&layouts.frame), Some(&layouts.material)],
            immediate_size: 0,
        });
        Self {
            layout,
            format,
            samples: samples.max(1),
            standard: SurfaceShader::standard().revision(),
            modules: HashMap::new(),
            pipelines: HashMap::new(),
            uses: 0,
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

    /// How many shader modules it holds: the standard one and each custom
    /// shader drawn lately.
    pub fn shaders(&self) -> usize {
        self.modules.len()
    }

    /// Starts a shot: lets go of every custom shader's module and
    /// pipelines not drawn with in the last [`KEEP_USES`] shots.
    pub fn begin(&mut self) {
        self.uses += 1;
        let (now, standard) = (self.uses, self.standard);
        let stale = |shader: u64, last: u64| shader != standard && last + KEEP_USES < now;
        self.modules
            .retain(|shader, used| !stale(*shader, used.last));
        self.pipelines
            .retain(|key, used| !stale(key.shader, used.last));
    }

    /// Makes the pipeline for `key`, drawn through `shader` (whose
    /// revision `key` carries), if it is not made yet.
    pub fn prepare(&mut self, device: &wgpu::Device, key: SurfaceKey, shader: &SurfaceShader) {
        let now = self.uses;
        if let Some(used) = self.pipelines.get_mut(&key) {
            used.last = now;
            if let Some(module) = self.modules.get_mut(&key.shader) {
                module.last = now;
            }
            return;
        }
        if key.shader != shader.revision() {
            log::error!(
                "a surface pipeline keyed by shader {:x} asked for with shader {:x}; skipped \
                 (a renderer bug)",
                key.shader,
                shader.revision()
            );
            return;
        }
        let module = self.modules.entry(key.shader).or_insert_with(|| Used {
            value: device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("surface-3d"),
                source: wgpu::ShaderSource::Wgsl(shader.source().into()),
            }),
            last: now,
        });
        module.last = now;
        let pipeline = build(
            device,
            &self.layout,
            &module.value,
            self.format,
            self.samples,
            key,
        );
        self.pipelines.insert(
            key,
            Used {
                value: pipeline,
                last: now,
            },
        );
    }

    /// The pipeline for `key`, if [`prepare`](Self::prepare) made it.
    pub fn pipeline(&self, key: SurfaceKey) -> Option<&wgpu::RenderPipeline> {
        self.pipelines.get(&key).map(|used| &used.value)
    }
}

/// The pipeline for `key` through `module`, into `format` at `samples`.
fn build(
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
