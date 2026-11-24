//! The pipelines a frame's materials draw with, and its uniform blocks.

use std::collections::HashMap;

use super::pipeline::{create_material_pipeline, uniform_layout};
use super::uniforms::{MaterialUniforms, BLOCK_SIZE};
use crate::pass::MaterialDraw;

/// One material use a frame draws: the material's compiled-from revision,
/// its composed source, and this use's uniform block.
pub struct MaterialRequest<'a> {
    /// Unique to one read of one material: a changed material is a new
    /// revision, and so a new pipeline.
    pub revision: u64,
    /// The material's composed WGSL source.
    pub source: &'a str,
    /// This use's uniform block, as the shader reads it.
    pub block: [u8; BLOCK_SIZE],
}

/// The material pipelines made so far, by revision and target format, and
/// the frame's uniform blocks.
///
/// Prepared once a frame, before the draw list is made: the pipelines a
/// request needs are compiled the first time — a material is compiled
/// once, not once a frame — and every block is uploaded into one buffer.
/// Pipelines no frame has drawn with for a while are let go, so a material
/// edited a hundred times does not keep a hundred.
pub struct MaterialBank {
    layout: wgpu::BindGroupLayout,
    uniforms: MaterialUniforms,
    pipelines: HashMap<(u64, wgpu::TextureFormat), Cached>,
    /// For each request of the last prepare: its pipeline's key and block
    /// offset, or `None` if it was not drawable.
    prepared: Vec<Option<((u64, wgpu::TextureFormat), u32)>>,
    frame: u64,
}

struct Cached {
    pipeline: wgpu::RenderPipeline,
    last_used: u64,
}

/// Frames a pipeline is kept without being drawn with.
const KEEP_FRAMES: u64 = 600;

impl MaterialBank {
    /// An empty bank on `device`.
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = uniform_layout(device);
        let uniforms = MaterialUniforms::new(device, &layout);
        Self {
            layout,
            uniforms,
            pipelines: HashMap::new(),
            prepared: Vec::new(),
            frame: 0,
        }
    }

    /// Compiles what `requests` need for `format` and uploads their blocks.
    /// `layouts` are the sprite pipeline's groups 0 and 1: the camera and
    /// the texture. A request that is `None` draws plainly.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        layouts: [&wgpu::BindGroupLayout; 2],
        requests: &[Option<MaterialRequest<'_>>],
    ) {
        self.frame += 1;
        let blocks: Vec<[u8; BLOCK_SIZE]> = requests.iter().flatten().map(|r| r.block).collect();
        let offsets = self.uniforms.upload(device, queue, &self.layout, &blocks);
        let mut offsets = offsets.into_iter();
        self.prepared.clear();
        for request in requests {
            let Some(request) = request else {
                self.prepared.push(None);
                continue;
            };
            let offset = offsets.next().expect("one offset per block uploaded");
            let key = (request.revision, format);
            let frame = self.frame;
            let [camera, texture] = layouts;
            let cached = self.pipelines.entry(key).or_insert_with(|| Cached {
                pipeline: create_material_pipeline(
                    device,
                    format,
                    "material",
                    request.source,
                    [camera, texture, &self.layout],
                ),
                last_used: frame,
            });
            cached.last_used = frame;
            self.prepared.push(Some((key, offset)));
        }
        let now = self.frame;
        self.pipelines
            .retain(|_, cached| now - cached.last_used <= KEEP_FRAMES);
    }

    /// What request `index` of the last [`prepare`](Self::prepare) draws
    /// with, or `None` to draw it plainly.
    pub fn draw(&self, index: u32) -> Option<MaterialDraw<'_>> {
        let (key, offset) = (*self.prepared.get(index as usize)?)?;
        let cached = self.pipelines.get(&key)?;
        Some(MaterialDraw {
            pipeline: &cached.pipeline,
            uniforms: self.uniforms.bind_group(),
            offset,
        })
    }

    /// Pipelines held now.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Whether no pipeline is held.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }
}
