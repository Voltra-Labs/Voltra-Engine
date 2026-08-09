//! Headless GPU scaffolding shared by every crate's integration tests.
//!
//! A dev-dependency, never shipped: `publish = false`, and no member crate
//! depends on it outside `[dev-dependencies]`. It exists because each
//! integration test is its own binary and each crate its own `tests/` tree, so
//! the alternative is the same adapter-acquisition and readback code copied
//! once per crate — which is what it was until three copies were due.
//!
//! It depends on `voltra-render` for `wgpu`, like every other non-render
//! crate, and declares no `wgpu` of its own.

use std::path::{Path, PathBuf};

use voltra_render::wgpu;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    /// Whether this pixel is close enough to the test clear colour to be it.
    ///
    /// 0.1 linear encodes to roughly 89 in an sRGB texture, so the thresholds
    /// sit comfortably above that and well below anything the shaders emit.
    pub fn is_clear_ish(&self) -> bool {
        self.r < 120 && self.g < 120 && self.b < 130
    }
}

pub const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.1,
    g: 0.1,
    b: 0.12,
    a: 1.0,
};

/// Returns `None` when the machine has no usable adapter, so a CI runner
/// without a GPU skips rather than fails.
pub fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;

    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("headless-test-device"),
        ..Default::default()
    }))
    .ok()
}

/// Copies a rendered texture back to the CPU, row by row.
///
/// The texture must carry [`wgpu::TextureUsages::COPY_SRC`] and hold four bytes
/// per pixel.
pub fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> Vec<Rgba> {
    // `copy_texture_to_buffer` demands bytes_per_row be a multiple of 256, so
    // every row but the last carries padding the CPU side has to step over.
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = (width * 4).div_ceil(align) * align;

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("headless-readback"),
        size: (padded * height) as wgpu::BufferAddress,
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
            buffer: &readback,
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

    readback.map_async(wgpu::MapMode::Read, .., |result| {
        result.expect("readback buffer failed to map");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll failed");

    let pixels = {
        let mapped = readback
            .get_mapped_range(..)
            .expect("readback buffer range not mapped");
        let mut out = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            let row = (y * padded) as usize;
            for x in 0..width {
                let i = row + (x * 4) as usize;
                out.push(Rgba {
                    r: mapped[i],
                    g: mapped[i + 1],
                    b: mapped[i + 2],
                    a: mapped[i + 3],
                });
            }
        }
        out
    };
    readback.unmap();

    pixels
}

/// A fresh directory under the system temp dir, unique per call.
pub fn scratch_root() -> PathBuf {
    // Unique per call so tests running in parallel cannot see each other's
    // files. Nothing cleans these up; they are a few hundred bytes each.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "voltra-assets-{nanos}-{:?}",
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Writes a real PNG of `width` x `height` in one flat colour at `root/name`.
///
/// Tests that tell two textures apart by what comes back need their own
/// colours; [`write_png`] is the red-by-default case.
///
/// Encodes through `PngEncoder` rather than `DynamicImage::save_with_format`.
/// The workspace pins `image` with `default-features = false, features =
/// ["png"]`, and the convenience `save*` helpers sit behind feature gates that
/// set does not necessarily turn on; the encoder is exactly what the `png`
/// feature provides.
pub fn write_png_rgba(root: &Path, name: &str, width: u32, height: u32, rgba: [u8; 4]) {
    use image::ImageEncoder;

    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("asset subdirectory");
    }

    let pixels: Vec<u8> = (0..width * height).flat_map(|_| rgba).collect();

    let file = std::fs::File::create(&path).expect("creating the test PNG");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .expect("encoding the test PNG");
}

/// Writes a real PNG of `width` x `height` opaque red at `root/name`.
pub fn write_png(root: &Path, name: &str, width: u32, height: u32) {
    write_png_rgba(root, name, width, height, [255, 0, 0, 255]);
}
