//! The 3D pass: one depth-tested, instanced draw of a [`Scene3D`] into a
//! shot's [`Targets3D`].

use super::depth::DEPTH_CLEAR;
use super::hdr::Hdr3D;
use super::instance::Instance3D;
use super::lighting::FrameUniform3D;
use super::scene::Scene3D;
use super::surface::{SurfaceBank, SurfaceKey, SurfaceLayouts, SurfacePass, SurfaceVertex};
use super::targets::Targets3D;
use super::MeshDraw3D;

/// Records a [`Scene3D`]: clears colour and depth, uploads the view, the
/// light and the instances, then one instanced draw per [`MeshDraw3D`],
/// each through its material's pipeline variant.
///
/// Owns what persists between frames — the surface pipelines, the frame's
/// uniform and the instance buffer (grown, never shrunk); the targets are
/// the shot's, the materials the caller's. Every call writes its buffers
/// before its own submit, so one pass serves one shot per submit
/// (`Queue::write_buffer` lands at the next submit, for every draw of it).
#[derive(Debug)]
pub struct Pass3D {
    bank: SurfaceBank,
    frame_buffer: wgpu::Buffer,
    frame_group: wgpu::BindGroup,
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
        let frame_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame-3d"),
            layout: &layouts.frame,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });
        Self {
            bank,
            frame_buffer,
            frame_group,
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
        queue.write_buffer(
            &self.frame_buffer,
            0,
            bytemuck::bytes_of(&FrameUniform3D::new(
                &scene.view,
                &scene.lighting,
                scene.time,
            )),
        );
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
        let Some(instances) = instances else {
            return;
        };
        pass.set_bind_group(0, &self.frame_group, &[]);
        let mut bound_pipeline = None;
        let mut bound_material: Option<&super::SurfaceMaterial3D> = None;
        let stride = size_of::<Instance3D>() as wgpu::BufferAddress;
        let count = scene.instances.len() as u32;
        for draw in scene.draws {
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
