//! The surface pipelines, one per variant a draw needs, made on first use.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

use super::check::GpuCheck;
use super::key::SurfaceKey;
use super::layouts::SurfaceLayouts;
use super::pipeline::build;
use super::shader::SurfaceShader;

/// The surface pipelines into one colour format at one sample count, by
/// [`SurfaceKey`]: made the first time a draw needs one.
///
/// **A custom shader is drawn only once the GPU has taken it.** Its module
/// and each pipeline are made in a validation scope ([`GpuCheck`]); until
/// the scope answers — at once on wgpu-core, a promise later on WebGPU —
/// and if it answers with an error, its draws use the standard shader's
/// pipeline of the same alpha, sides and winding, so its materials show
/// their standard block, never a black frame. A refused shader is said
/// once with the GPU's own message (Tint's, in a browser).
///
/// **A custom shader is kept while some material holds it**: the bank keeps
/// a `Weak` to it, and lets its module and pipelines go once that no longer
/// upgrades — an edited material's old shader, a closed preview's — not
/// when it has merely not been drawn (culled, off screen, in another
/// shot). The standard shader's variants are few (3 alpha classes × 2
/// sides × 3 windings) and kept for the bank's life.
#[derive(Debug)]
pub struct SurfaceBank {
    layout: wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    samples: u32,
    standard: Arc<SurfaceShader>,
    modules: HashMap<u64, Module>,
    pipelines: HashMap<SurfaceKey, Built>,
    /// Custom shaders the GPU refused, kept while they live.
    broken: HashSet<u64>,
}

#[derive(Debug)]
struct Module {
    module: wgpu::ShaderModule,
    shader: Weak<SurfaceShader>,
    check: GpuCheck,
}

#[derive(Debug)]
struct Built {
    pipeline: wgpu::RenderPipeline,
    check: GpuCheck,
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
            standard: SurfaceShader::standard(),
            modules: HashMap::new(),
            pipelines: HashMap::new(),
            broken: HashSet::new(),
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
    /// shader a material still holds.
    pub fn shaders(&self) -> usize {
        self.modules.len()
    }

    /// Whether the GPU refused the shader of `revision`: its materials draw
    /// their standard block.
    pub fn is_broken(&self, revision: u64) -> bool {
        self.broken.contains(&revision)
    }

    /// Starts a shot: reads the validation scopes that have answered, and
    /// lets go of every custom shader no material holds any more, its
    /// pipelines with it.
    pub fn begin(&mut self) {
        let standard = self.standard.revision();
        self.modules
            .retain(|revision, module| *revision == standard || module.shader.strong_count() > 0);
        let modules = &self.modules;
        self.pipelines
            .retain(|key, _| modules.contains_key(&key.shader));
        self.broken
            .retain(|revision| modules.contains_key(revision));
        let revisions: Vec<u64> = self.modules.keys().copied().collect();
        for revision in revisions {
            self.poll_module(revision);
        }
        let keys: Vec<SurfaceKey> = self.pipelines.keys().copied().collect();
        for key in keys {
            self.poll_pipeline(key);
        }
    }

    /// Makes the pipeline for `key`, drawn through `shader` (whose
    /// revision `key` carries), if it is not made yet — and, for a custom
    /// shader, the standard pipeline its draws fall back on.
    pub fn prepare(&mut self, device: &wgpu::Device, key: SurfaceKey, shader: &Arc<SurfaceShader>) {
        if key.shader != shader.revision() {
            log::error!(
                "a surface pipeline keyed by shader {:x} asked for with shader {:x}; skipped \
                 (a renderer bug)",
                key.shader,
                shader.revision()
            );
            return;
        }
        let standard = self.standard.revision();
        if key.shader != standard {
            let fallback = SurfaceKey {
                shader: standard,
                ..key
            };
            let shader = Arc::clone(&self.standard);
            self.make(device, fallback, &shader);
        }
        self.make(device, key, shader);
    }

    fn make(&mut self, device: &wgpu::Device, key: SurfaceKey, shader: &Arc<SurfaceShader>) {
        if self.pipelines.contains_key(&key) || self.broken.contains(&key.shader) {
            return;
        }
        let custom = key.shader != self.standard.revision();
        if let Entry::Vacant(slot) = self.modules.entry(key.shader) {
            let create = || {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("surface-3d"),
                    source: wgpu::ShaderSource::Wgsl(shader.source().into()),
                })
            };
            // The engine's own shader is pinned by its tests; a scope
            // around it would only delay its first frame on WebGPU.
            let (module, check) = if custom {
                GpuCheck::scoped(device, create)
            } else {
                (create(), GpuCheck::Passed)
            };
            slot.insert(Module {
                module,
                shader: Arc::downgrade(shader),
                check,
            });
            self.poll_module(key.shader);
            if self.broken.contains(&key.shader) {
                return;
            }
        }
        let Some(module) = self.modules.get(&key.shader) else {
            return;
        };
        let create = || {
            build(
                device,
                &self.layout,
                &module.module,
                self.format,
                self.samples,
                key,
            )
        };
        let (pipeline, check) = if custom {
            GpuCheck::scoped(device, create)
        } else {
            (create(), GpuCheck::Passed)
        };
        self.pipelines.insert(key, Built { pipeline, check });
        self.poll_pipeline(key);
    }

    /// The pipeline `key`'s draws are made with: its own once the GPU has
    /// taken it, the standard one of the same alpha, sides and winding
    /// meanwhile and for good once it has refused it. `None` before
    /// [`prepare`](Self::prepare).
    pub fn pipeline(&self, key: SurfaceKey) -> Option<&wgpu::RenderPipeline> {
        let ready = !self.broken.contains(&key.shader)
            && self
                .modules
                .get(&key.shader)
                .is_some_and(|module| module.check.passed())
            && self
                .pipelines
                .get(&key)
                .is_some_and(|built| built.check.passed());
        let key = if ready {
            key
        } else {
            SurfaceKey {
                shader: self.standard.revision(),
                ..key
            }
        };
        self.pipelines.get(&key).map(|built| &built.pipeline)
    }

    fn poll_module(&mut self, revision: u64) {
        let Some(module) = self.modules.get_mut(&revision) else {
            return;
        };
        if let Some(why) = module.check.poll() {
            let why = why.to_owned();
            self.refuse(revision, &why);
        }
    }

    fn poll_pipeline(&mut self, key: SurfaceKey) {
        let Some(built) = self.pipelines.get_mut(&key) else {
            return;
        };
        if let Some(why) = built.check.poll() {
            let why = why.to_owned();
            self.refuse(key.shader, &why);
        }
    }

    /// Marks the shader of `revision` refused — once, with the GPU's word
    /// — and drops its pipelines: its materials draw their standard block.
    fn refuse(&mut self, revision: u64, why: &str) {
        if self.broken.insert(revision) {
            log::warn!(
                "the GPU refused a custom surface shader ({revision:016x}); its materials draw \
                 their standard block:\n{why}\n(on WebGPU, often a texture sampled or a \
                 derivative taken after a branch: sample first, then `select`)"
            );
        }
        self.pipelines.retain(|key, _| key.shader != revision);
    }
}
