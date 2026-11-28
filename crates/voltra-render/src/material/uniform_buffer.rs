//! The buffer one frame's sprite-material uniform blocks go in.

use super::uniforms::BLOCK_SIZE;

/// One frame's uniform blocks, in one buffer, bound with a dynamic offset
/// per draw: a material's draws differ only in where their block starts.
pub struct MaterialUniforms {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Blocks it holds room for.
    capacity: usize,
    /// Bytes from one block to the next: the device's offset alignment, at
    /// least a block.
    stride: usize,
}

impl MaterialUniforms {
    /// A uniform buffer on `device` and its bind group against `layout`.
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let align = device.limits().min_uniform_buffer_offset_alignment as usize;
        let stride = BLOCK_SIZE.next_multiple_of(align.max(1));
        Self::with_capacity(device, layout, stride, 16)
    }

    fn with_capacity(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        stride: usize,
        capacity: usize,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("material-uniforms"),
            size: (stride * capacity) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material-uniforms"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(BLOCK_SIZE as u64),
                }),
            }],
        });
        Self {
            buffer,
            bind_group,
            capacity,
            stride,
        }
    }

    /// Uploads this frame's `blocks`, growing the buffer if they do not fit,
    /// and answers each block's dynamic offset, in order.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        blocks: &[[u8; BLOCK_SIZE]],
    ) -> Vec<u32> {
        if blocks.len() > self.capacity {
            let capacity = blocks.len().next_power_of_two();
            *self = Self::with_capacity(device, layout, self.stride, capacity);
        }
        let mut bytes = vec![0u8; self.stride * blocks.len()];
        for (i, block) in blocks.iter().enumerate() {
            bytes[i * self.stride..i * self.stride + BLOCK_SIZE].copy_from_slice(block);
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, &bytes);
        }
        (0..blocks.len())
            .map(|i| (i * self.stride) as u32)
            .collect()
    }

    /// The bind group over the frame's uniform blocks.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
