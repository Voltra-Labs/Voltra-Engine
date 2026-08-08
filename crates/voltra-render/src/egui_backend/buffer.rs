//! GPU-side buffers for the geometry egui submits each frame.
//!
//! `write_growable` backs the vertex and index buffers, which are resized only
//! when the frame's geometry no longer fits rather than every frame. `Locals`
//! is the uniform those draws read the screen size from.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(super) struct Locals {
    pub(super) screen_size: [f32; 2],
    /// The shader-side struct is padded to 16 bytes; this keeps the Rust side
    /// the same size so the write is not short.
    pub(super) _padding: [f32; 2],
}

pub(super) fn empty_buffer(
    device: &wgpu::Device,
    label: &str,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: 0,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Writes `data`, reallocating only when it no longer fits.
///
/// The UI's triangle count changes every frame as panels open and text scrolls,
/// so a fixed buffer would either overflow or be sized for the worst case.
pub(super) fn write_growable(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &mut wgpu::Buffer,
    label: &str,
    usage: wgpu::BufferUsages,
    data: &[u8],
) {
    if data.is_empty() {
        return;
    }

    if (data.len() as wgpu::BufferAddress) > buffer.size() {
        // Doubling keeps a steadily growing UI from reallocating every frame.
        let size = (data.len() as wgpu::BufferAddress).next_power_of_two();
        *buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    queue.write_buffer(buffer, 0, data);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locals_match_the_padded_shader_struct() {
        assert_eq!(size_of::<Locals>(), 16);
    }
}
