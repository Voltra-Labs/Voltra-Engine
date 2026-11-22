//! Copying a rendered texture back to the CPU.

/// The pixels of `texture`, `width` by `height`, as tightly packed RGBA
/// bytes, top row first.
///
/// For a texture of four bytes per pixel carrying `COPY_SRC` — every
/// [`RenderTarget`](crate::RenderTarget) does. A BGRA texture's channels
/// are swapped into RGBA order; the bytes are otherwise as stored, so an
/// sRGB target reads back as the sRGB values a screen would show. Blocks
/// until the copy is done.
pub fn read_rgba8(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<u8> {
    // Rows of a copy are padded to 256 bytes; the padding is stepped over.
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = (width * 4).div_ceil(align) * align;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback-encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let (sent, mapped) = std::sync::mpsc::channel();
    buffer.map_async(wgpu::MapMode::Read, .., move |result| {
        let _ = sent.send(result);
    });
    if let Err(e) = device.poll(wgpu::PollType::wait_indefinitely()) {
        log::error!("waiting for a readback failed: {e}");
        return vec![0; (width * height * 4) as usize];
    }
    match mapped.recv() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            log::error!("a readback buffer would not map: {e}");
            return vec![0; (width * height * 4) as usize];
        }
        Err(_) => {
            log::error!("a readback was dropped before it mapped");
            return vec![0; (width * height * 4) as usize];
        }
    }

    let bgra = matches!(
        texture.format(),
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    );
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    {
        let Ok(view) = buffer.get_mapped_range(..) else {
            log::error!("a readback buffer's range was not mapped");
            return vec![0; (width * height * 4) as usize];
        };
        for y in 0..height {
            let row = &view[(y * padded) as usize..][..(width * 4) as usize];
            for pixel in row.chunks_exact(4) {
                if bgra {
                    out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                } else {
                    out.extend_from_slice(pixel);
                }
            }
        }
    }
    buffer.unmap();
    out
}
