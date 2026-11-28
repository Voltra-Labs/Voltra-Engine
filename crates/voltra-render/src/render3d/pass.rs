//! The 3D pass: one depth-tested, instanced draw of a [`Scene3D`] into a
//! shot's [`Targets3D`].

use super::depth::DEPTH_CLEAR;
use super::hdr::Hdr3D;
use super::instance::Instance3D;
use super::pipeline::UnlitPipeline3D;
use super::scene::Scene3D;
use super::targets::Targets3D;

/// Records a [`Scene3D`]: clears colour and depth, uploads the view and
/// the instances, then one instanced draw per [`MeshDraw3D`].
///
/// Owns what persists between frames — the pipeline, the view's uniform
/// and the instance buffer (grown, never shrunk); the targets are the
/// shot's. Every call writes its buffers before its own submit, so one
/// pass serves one shot per submit (`Queue::write_buffer` lands at the next
/// submit, for every draw of it).
///
/// [`MeshDraw3D`]: super::MeshDraw3D
#[derive(Debug)]
pub struct Pass3D {
    pipeline: UnlitPipeline3D,
    view_buffer: wgpu::Buffer,
    view_group: wgpu::BindGroup,
    instances: Option<wgpu::Buffer>,
}

impl Pass3D {
    /// A pass drawing into targets of `hdr`'s format and samples.
    pub fn new(device: &wgpu::Device, hdr: Hdr3D) -> Self {
        let pipeline = UnlitPipeline3D::new(device, hdr.format, hdr.samples);
        let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("view-3d"),
            size: size_of::<[[f32; 4]; 4]>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("view-3d"),
            layout: pipeline.view_layout(),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view_buffer.as_entire_binding(),
            }],
        });
        Self {
            pipeline,
            view_buffer,
            view_group,
            instances: None,
        }
    }

    /// The format and samples it draws.
    pub fn hdr(&self) -> Hdr3D {
        Hdr3D {
            format: self.pipeline.format(),
            samples: self.pipeline.samples(),
        }
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
            &self.view_buffer,
            0,
            bytemuck::cast_slice(&scene.view.view_projection().to_cols_array_2d()),
        );
        let instances = upload_instances(device, queue, &mut self.instances, scene.instances);

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
        pass.set_bind_group(0, &self.view_group, &[]);
        let mut bound = None;
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
            if bound != Some(draw.winding) {
                pass.set_pipeline(self.pipeline.pipeline(draw.winding));
                bound = Some(draw.winding);
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
