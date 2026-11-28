//! The surface pipelines, one per variant a draw needs, made on first use.

use std::collections::HashMap;

use super::key::SurfaceKey;
use super::layouts::SurfaceLayouts;
use super::pipeline::build;
use super::shader::SurfaceShader;

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
