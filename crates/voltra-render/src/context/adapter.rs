//! What a context asks the adapter for: limits, features, the frame's
//! sRGB format and the present mode.

/// The present mode for vsync `on`: FIFO, which every surface supports, or
/// the first of immediate, mailbox and FIFO the surface has.
pub(super) fn present_mode(on: bool) -> wgpu::PresentMode {
    if on {
        wgpu::PresentMode::Fifo
    } else {
        wgpu::PresentMode::AutoNoVsync
    }
}

/// The limits to ask for: wgpu's defaults wherever the adapter meets them —
/// every native adapter this engine targets, and WebGPU — else what the
/// adapter has, which on WebGL2 is far below them.
///
/// Asking for the defaults on WebGL2 fails device creation outright; asking
/// for the adapter's own is always granted, and the passes that need more
/// (GPU particles' storage buffers) already check before they build.
pub(super) fn required_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    let available = adapter.limits();
    let defaults = wgpu::Limits::default();
    if defaults.check_limits(&available) {
        defaults
    } else {
        log::info!("adapter below wgpu's default limits; asking for its own");
        available
    }
}

/// Picks the sRGB format frames are drawn in and, when the surface has none,
/// lets `config` be viewed as one.
///
/// The colour rules (ARCHITECTURE.md, "egui for the editor UI") assume the
/// frame encodes on write. Native surfaces offer an sRGB format and
/// `get_default_config` takes it; a WebGPU canvas offers only linear ones
/// (`bgra8unorm`, `rgba8unorm`), and drawing into those unconverted would
/// lighten every mid-tone. The canvas keeps its linear format and is drawn
/// through an sRGB view of it — WebGPU's own answer, `viewFormats`.
pub(super) fn srgb_format(
    offered: &[wgpu::TextureFormat],
    config: &mut wgpu::SurfaceConfiguration,
) -> wgpu::TextureFormat {
    if config.format.is_srgb() {
        return config.format;
    }
    if let Some(srgb) = offered.iter().copied().find(|f| f.is_srgb()) {
        config.format = srgb;
        return srgb;
    }
    let srgb = config.format.add_srgb_suffix();
    if srgb != config.format {
        config.view_formats.push(srgb);
    }
    srgb
}

/// The features asked for when the adapter has them, and never required:
/// GPU timing, both of its features or neither.
pub(super) fn optional_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    let timing = crate::gpu_timer::timing_features();
    if adapter.features().contains(timing) {
        timing
    } else {
        wgpu::Features::empty()
    }
}
