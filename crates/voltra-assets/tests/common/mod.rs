//! Shared scaffolding for the headless GPU tests.
//!
//! Each integration test is its own binary, so anything used by only one of
//! them looks dead to the others. That is what the blanket allow is for.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use voltra_render::wgpu;

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

/// Writes a real PNG of `width` x `height` opaque red at `root/name`.
///
/// Encodes through `PngEncoder` rather than `DynamicImage::save_with_format`.
/// The workspace pins `image` with `default-features = false, features =
/// ["png"]`, and the convenience `save*` helpers sit behind feature gates that
/// set does not necessarily turn on; the encoder is exactly what the `png`
/// feature provides.
pub fn write_png(root: &Path, name: &str, width: u32, height: u32) {
    use image::ImageEncoder;

    let path = root.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("asset subdirectory");
    }

    let pixels: Vec<u8> = (0..width * height)
        .flat_map(|_| [255u8, 0, 0, 255])
        .collect();

    let file = std::fs::File::create(&path).expect("creating the test PNG");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .expect("encoding the test PNG");
}
