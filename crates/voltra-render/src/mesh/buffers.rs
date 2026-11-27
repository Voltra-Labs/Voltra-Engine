//! A mesh whose buffers outlive the frame: rewritten in place with
//! `Queue::write_buffer` each frame, grown when the frame needs more.
//!
//! The sprite batch changes every frame, and making it fresh buffers every
//! frame (what [`Mesh2D::batch`] does) pays a buffer allocation, its zeroing
//! and a staging copy for each — milliseconds for a 100k-sprite batch, and
//! on the web memory only JavaScript's collector frees. Bevy keeps its
//! sprite instances in a `RawBufferVec` rewritten each frame and Godot its
//! canvas batches in buffers that grow and stay; this is the same.
//!
//! Capacity grows to the next power of two when a frame needs more, and is
//! cut back when a frame needs less than a quarter of it, so a level
//! that drew 100k sprites does not hold their memory through a menu of
//! ten, and a count wobbling around a power of two does not reallocate
//! every frame.

use bytemuck::Pod;

use super::{Mesh2D, QuadInstance};

/// The smallest buffer made: a few hundred quads.
const MIN_BYTES: u64 = 16 * 1024;

/// Under this share of its capacity in use, a buffer is made anew, smaller.
const SHRINK_BELOW: u64 = 4;

/// A [`Mesh2D`] kept from frame to frame. Empty until first written.
pub struct MeshBuffers {
    label: &'static str,
    mesh: Mesh2D,
}

/// Labelled for what kept buffers are, most often.
impl Default for MeshBuffers {
    fn default() -> Self {
        Self::new("sprite-batch")
    }
}

impl MeshBuffers {
    /// Buffers labelled `label`, none made yet.
    pub fn new(label: &'static str) -> Self {
        Self {
            label,
            mesh: Mesh2D::empty(),
        }
    }

    /// The mesh drawing `vertices` through `indices` and `quads`, written
    /// into the kept buffers — made or remade only when they are too small
    /// or far too large. `None` when there is nothing to draw, as
    /// [`Mesh2D::batch`] has nothing to upload.
    ///
    /// The writes are queued and land before the next submission's
    /// commands, after the previous one's: last frame's draws read last
    /// frame's data. A buffer replaced here was used by a submission
    /// already made, and `destroy` waits for it.
    pub fn write<V: Pod>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[V],
        indices: &[u32],
        quads: &[QuadInstance],
    ) -> Option<&Mesh2D> {
        let mesh = &mut self.mesh;
        let triangles = !indices.is_empty();
        if triangles {
            let usage = wgpu::BufferUsages::VERTEX;
            put(
                &mut mesh.vertices,
                device,
                queue,
                self.label,
                vertices,
                usage,
            );
            let usage = wgpu::BufferUsages::INDEX;
            put(&mut mesh.indices, device, queue, self.label, indices, usage);
        }
        mesh.count = indices.len() as u32;
        if !quads.is_empty() {
            let usage = wgpu::BufferUsages::VERTEX;
            put(&mut mesh.quads, device, queue, self.label, quads, usage);
        }
        mesh.quad_count = quads.len() as u32;
        (triangles || !quads.is_empty()).then_some(&self.mesh)
    }

    /// Bytes of GPU memory the kept buffers hold.
    pub fn capacity(&self) -> u64 {
        [&self.mesh.vertices, &self.mesh.indices, &self.mesh.quads]
            .into_iter()
            .flatten()
            .map(wgpu::Buffer::size)
            .sum()
    }
}

/// Writes `data` at the start of `slot`, first replacing the buffer there
/// when it is missing, too small, or more than [`SHRINK_BELOW`] times too
/// large.
fn put<T: Pod>(
    slot: &mut Option<wgpu::Buffer>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    data: &[T],
    usage: wgpu::BufferUsages,
) {
    let bytes: &[u8] = bytemuck::cast_slice(data);
    // `write_buffer` takes whole words; a vertex of odd size is padded.
    let padded: Vec<u8>;
    let bytes = if bytes
        .len()
        .is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT as usize)
    {
        bytes
    } else {
        let mut copy = bytes.to_vec();
        copy.resize(
            bytes
                .len()
                .next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT as usize),
            0,
        );
        padded = copy;
        &padded
    };
    let needed = bytes.len() as u64;
    let fits = slot.as_ref().is_some_and(|buffer| {
        let size = buffer.size();
        size >= needed && (size <= MIN_BYTES || needed * SHRINK_BELOW >= size)
    });
    if !fits {
        if let Some(old) = slot.take() {
            old.destroy();
        }
        *slot = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: needed.next_power_of_two().max(MIN_BYTES),
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
    }
    queue.write_buffer(slot.as_ref().expect("made just above"), 0, bytes);
}
