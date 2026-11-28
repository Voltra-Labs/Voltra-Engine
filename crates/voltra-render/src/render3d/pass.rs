//! The 3D pass: one depth-tested, instanced draw of a [`Scene3D`] into a
//! shot's [`Targets3D`].

use super::depth::DEPTH_CLEAR;
use super::hdr::Hdr3D;
use super::instance::Instance3D;
use super::lighting::{FrameUniform3D, LightsUniform3D};
use super::scene::Scene3D;
use super::sky::SkyPipeline;
use super::surface::{
    AlphaClass, SurfaceBank, SurfaceKey, SurfaceLayouts, SurfacePass, SurfaceVertex,
};
use super::targets::Targets3D;
use super::MeshDraw3D;

/// Records a [`Scene3D`]: clears colour and depth, uploads the view, the
/// lights and the instances, then one instanced draw per [`MeshDraw3D`],
/// each through its material's pipeline variant — the sky, when there is
/// one, after the opaque and masked and before the blended.
///
/// Owns what persists between frames — the surface pipelines, the sky's,
/// the frame's uniforms and the instance buffer (grown, never shrunk); the targets are
/// the shot's, the materials the caller's. Every call writes its buffers
/// before its own submit, so one pass serves one shot per submit
/// (`Queue::write_buffer` lands at the next submit, for every draw of it).
#[derive(Debug)]
pub struct Pass3D {
    bank: SurfaceBank,
    frame_buffer: wgpu::Buffer,
    lights_buffer: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
    /// The frame's group 0 layout, which the sky is built against.
    frame_layout: wgpu::BindGroupLayout,
    /// Made the first time a scene has a sky.
    sky: Option<SkyPipeline>,
    instances: Option<wgpu::Buffer>,
}

impl Pass3D {
    /// A pass drawing into targets of `hdr`'s format and samples, its
    /// materials bound against `layouts`.
    pub fn new(device: &wgpu::Device, layouts: &SurfaceLayouts, hdr: Hdr3D) -> Self {
        let bank = SurfaceBank::new(device, layouts, hdr.format, hdr.samples);
        let frame_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame-3d"),
            size: size_of::<FrameUniform3D>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lights_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lights-3d"),
            size: size_of::<LightsUniform3D>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame-3d"),
            layout: &layouts.frame,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights_buffer.as_entire_binding(),
                },
            ],
        });
        Self {
            bank,
            frame_buffer,
            lights_buffer,
            frame_group,
            frame_layout: layouts.frame.clone(),
            sky: None,
            instances: None,
        }
    }

    /// The format and samples it draws.
    pub fn hdr(&self) -> Hdr3D {
        Hdr3D {
            format: self.bank.format(),
            samples: self.bank.samples(),
        }
    }

    /// How many pipeline variants it holds.
    pub fn pipelines(&self) -> usize {
        self.bank.len()
    }

    /// How many surface shaders it holds modules of.
    pub fn shaders(&self) -> usize {
        self.bank.shaders()
    }

    /// Records `scene` into `targets`, made for [`hdr`](Self::hdr): colour
    /// cleared to `scene.clear` and resolved when multisampled, depth
    /// cleared to the far plane.
    pub fn record(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        targets: &Targets3D,
        scene: &Scene3D<'_>,
    ) {
        if targets.hdr() != self.hdr() {
            log::error!(
                "3D targets of {:?} drawn by a pass for {:?}; skipped (a renderer bug)",
                targets.hdr(),
                self.hdr()
            );
            return;
        }
        let clear = [scene.clear.r, scene.clear.g, scene.clear.b, scene.clear.a].map(|c| c as f32);
        queue.write_buffer(
            &self.frame_buffer,
            0,
            bytemuck::bytes_of(&FrameUniform3D::new(
                &scene.view,
                &scene.lighting,
                clear,
                scene.time,
            )),
        );
        queue.write_buffer(
            &self.lights_buffer,
            0,
            bytemuck::bytes_of(&LightsUniform3D::new(&scene.lighting)),
        );
        if scene.lighting.sky.is_some() && self.sky.is_none() {
            let hdr = self.hdr();
            self.sky = Some(SkyPipeline::new(
                device,
                &self.frame_layout,
                hdr.format,
                hdr.samples,
            ));
        }
        let instances = upload_instances(device, queue, &mut self.instances, scene.instances);
        self.bank.begin();
        for draw in scene.draws {
            self.bank.prepare(device, key(draw), draw.material.shader());
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("pass-3d"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: targets.color(),
                depth_slice: None,
                resolve_target: targets.resolve(),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(scene.clear),
                    // Resolved, the samples are read by nothing.
                    store: if targets.resolve().is_some() {
                        wgpu::StoreOp::Discard
                    } else {
                        wgpu::StoreOp::Store
                    },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: targets.depth(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(DEPTH_CLEAR),
                    // Nothing after the pass reads it yet.
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.frame_group, &[]);
        let sky = scene.lighting.sky.and(self.sky.as_ref());
        // The sky goes in before the first blended draw: the draws are in
        // order, the blended last.
        let blended_from = scene
            .draws
            .iter()
            .position(|draw| draw.material.alpha() == AlphaClass::Blend)
            .unwrap_or(scene.draws.len());
        let Some(instances) = instances else {
            if let Some(sky) = sky {
                sky.draw(&mut pass);
            }
            return;
        };
        let mut bound_pipeline = None;
        let mut bound_material: Option<&super::SurfaceMaterial3D> = None;
        let stride = size_of::<Instance3D>() as wgpu::BufferAddress;
        let count = scene.instances.len() as u32;
        for (at, draw) in scene.draws.iter().enumerate() {
            if at == blended_from {
                if let Some(sky) = sky {
                    sky.draw(&mut pass);
                    bound_pipeline = None;
                    bound_material = None;
                }
            }
            let range = &draw.instances;
            if range.start >= range.end {
                continue;
            }
            if range.end > count {
                log::error!(
                    "a 3D draw names instances {range:?} of {count}; skipped (a draw list bug)"
                );
                continue;
            }
            let key = key(draw);
            if bound_pipeline != Some(key) {
                let Some(pipeline) = self.bank.pipeline(key) else {
                    continue;
                };
                pass.set_pipeline(pipeline);
                bound_pipeline = Some(key);
            }
            if !bound_material.is_some_and(|bound| std::ptr::eq(bound, draw.material)) {
                pass.set_bind_group(1, draw.material.bind_group(), &[]);
                bound_material = Some(draw.material);
            }
            pass.set_vertex_buffer(0, draw.mesh.vertices().slice(..));
            // Bound at the run's first instance rather than drawn with a
            // first instance: the same on every backend, WebGL2 included.
            pass.set_vertex_buffer(
                1,
                instances.slice(u64::from(range.start) * stride..u64::from(range.end) * stride),
            );
            pass.set_index_buffer(draw.mesh.indices().slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..draw.mesh.index_count(), 0, 0..range.end - range.start);
        }
        if blended_from == scene.draws.len() {
            if let Some(sky) = sky {
                sky.draw(&mut pass);
            }
        }
    }
}

/// The pipeline variant `draw` is drawn with.
fn key(draw: &MeshDraw3D<'_>) -> SurfaceKey {
    SurfaceKey {
        shader: draw.material.shader().revision(),
        vertex: SurfaceVertex::Plain,
        alpha: draw.material.alpha(),
        double_sided: draw.material.double_sided(),
        winding: draw.winding,
        pass: SurfacePass::Main,
    }
}

/// Writes `instances` into `buffer`, growing it to the next power of two
/// when they do not fit; the buffer, or `None` for no instances.
fn upload_instances<'b>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &'b mut Option<wgpu::Buffer>,
    instances: &[Instance3D],
) -> Option<&'b wgpu::Buffer> {
    if instances.is_empty() {
        return None;
    }
    let bytes: &[u8] = bytemuck::cast_slice(instances);
    let needed = bytes.len() as wgpu::BufferAddress;
    if buffer.as_ref().is_none_or(|b| b.size() < needed) {
        *buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances-3d"),
            size: needed.next_power_of_two(),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    let buffer = buffer.as_ref().expect("made just above when missing");
    queue.write_buffer(buffer, 0, bytes);
    Some(buffer)
}
