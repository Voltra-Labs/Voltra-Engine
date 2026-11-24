//! Device, queue and swapchain ownership.

use wgpu::SurfaceTarget;

use crate::frame_buffers::FrameBuffers;

/// Owns the GPU device and, for a window, the swapchain surface.
///
/// Without a surface — [`GpuContext::headless`] — the configuration still
/// carries the format and size a window's would, so everything built from
/// it is built the same; there is just nothing to acquire or present.
pub struct GpuContext {
    surface: Option<wgpu::Surface<'static>>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// The format frames are drawn in: the surface's own when it is sRGB,
    /// else its sRGB view — see [`srgb_format`].
    format: wgpu::TextureFormat,
    /// What the adapter can do below full WebGPU: whether compute and
    /// vertex-stage storage, which GPU particles need, are there.
    downlevel: wgpu::DownlevelFlags,
    /// Buffers made for the frame being recorded; see [`FrameBuffers`].
    frame_buffers: FrameBuffers,
}

impl GpuContext {
    /// `target` is anything `wgpu` can build a surface from — typically an
    /// `Arc<winit::window::Window>`. Blocks on adapter/device acquisition,
    /// which happens once at startup.
    ///
    /// Native only: a browser cannot block, so the web builds its context
    /// through [`GpuContext::new_async`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(target: impl Into<SurfaceTarget<'static>>, width: u32, height: u32) -> Self {
        pollster::block_on(Self::new_async(target, width, height))
    }

    /// [`GpuContext::new`], awaited rather than blocked on: what a browser
    /// requires, where the adapter and device arrive through promises.
    ///
    /// On the web, WebGPU where the browser has it and WebGL2 where it does
    /// not — decided before the instance exists, since wgpu cannot fall back
    /// afterwards.
    pub async fn new_async(
        target: impl Into<SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        // `_from_env` honours WGPU_BACKEND / WGPU_POWER_PREF for debugging.
        let descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        #[cfg(target_arch = "wasm32")]
        let instance = wgpu::util::new_instance_with_webgpu_detection(descriptor).await;
        #[cfg(not(target_arch = "wasm32"))]
        let instance = wgpu::Instance::new(descriptor);

        let surface = instance
            .create_surface(target)
            .expect("failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("no compatible GPU adapter found");

        log::info!("adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("voltra-device"),
                required_features: optional_features(&adapter),
                required_limits: required_limits(&adapter),
                ..Default::default()
            })
            .await
            .expect("failed to request device");
        // Said once, plainly: everything after a lost device fails on its
        // own terms (a browser's "createBuffer failed"), far from the cause.
        device.set_device_lost_callback(|reason, message| {
            log::error!("the GPU device was lost ({reason:?}): {message}");
        });

        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .expect("surface not supported by adapter");
        let format = srgb_format(&surface.get_capabilities(&adapter).formats, &mut config);
        surface.configure(&device, &config);
        let downlevel = adapter.get_downlevel_capabilities().flags;

        Self {
            surface: Some(surface),
            device,
            queue,
            config,
            format,
            downlevel,
            frame_buffers: FrameBuffers::default(),
        }
    }

    /// A device with no window, for drawing offscreen: a test, a screenshot,
    /// a headless run. `None` when the machine has no usable adapter — the
    /// caller's cue to go without pictures rather than fail.
    ///
    /// The format is the sRGB one a window most often gets, so a frame drawn
    /// here matches a window's.
    pub fn headless(width: u32, height: u32) -> Option<Self> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        log::info!("headless adapter: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("voltra-headless-device"),
            required_features: optional_features(&adapter),
            ..Default::default()
        }))
        .ok()?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::new(),
        };
        let downlevel = adapter.get_downlevel_capabilities().flags;
        Some(Self {
            surface: None,
            device,
            queue,
            format: config.format,
            config,
            downlevel,
            frame_buffers: FrameBuffers::default(),
        })
    }

    /// Whether there is a window to present to.
    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }

    /// The GPU device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The adapter's downlevel capability flags.
    pub fn downlevel(&self) -> wgpu::DownlevelFlags {
        self.downlevel
    }

    /// The queue commands are submitted to.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Where a pass makes a buffer that only this frame reads.
    pub fn frame_buffers(&self) -> &FrameBuffers {
        &self.frame_buffers
    }

    /// Submits `commands`, then destroys the frame's own buffers, which
    /// nothing recorded after this can use.
    pub fn submit(&self, commands: wgpu::CommandBuffer) {
        self.queue.submit(Some(commands));
        self.frame_buffers.retire();
    }

    /// The surface configuration: size, format and present mode.
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    /// The format everything drawn into the frame is drawn in, and so what
    /// every pipeline targeting it is built for. Always sRGB when the
    /// adapter allows it; not always `config().format`, which on the web is
    /// the canvas's linear format viewed as sRGB.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// A view of an acquired frame in [`format`](Self::format).
    pub fn frame_view(&self, frame: &wgpu::SurfaceTexture) -> wgpu::TextureView {
        frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.format),
            ..Default::default()
        })
    }

    /// A zero-sized surface is invalid, so a minimised window is a no-op.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.reconfigure();
    }

    /// Applies the current configuration to the surface; does nothing without one.
    pub fn reconfigure(&mut self) {
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
    }

    /// Returns `None` when the frame should be skipped — always, without a
    /// surface. Recoverable states (`Outdated`, `Lost`) reconfigure the
    /// surface so the next call succeeds.
    pub fn acquire(&mut self) -> Option<wgpu::SurfaceTexture> {
        use wgpu::CurrentSurfaceTexture as Cst;

        let surface = self.surface.as_ref()?;
        match surface.get_current_texture() {
            Cst::Success(frame) => Some(frame),
            Cst::Suboptimal(frame) => {
                self.reconfigure();
                Some(frame)
            }
            Cst::Timeout | Cst::Occluded => None,
            Cst::Outdated | Cst::Lost => {
                self.reconfigure();
                None
            }
            other => {
                log::warn!("surface acquire failed: {other:?}");
                None
            }
        }
    }

    /// Presents a frame from [`Self::acquire`] to the window.
    pub fn present(&self, frame: wgpu::SurfaceTexture) {
        self.queue.present(frame);
    }
}

/// The limits to ask for: wgpu's defaults wherever the adapter meets them —
/// every native adapter this engine targets, and WebGPU — else what the
/// adapter has, which on WebGL2 is far below them.
///
/// Asking for the defaults on WebGL2 fails device creation outright; asking
/// for the adapter's own is always granted, and the passes that need more
/// (GPU particles' storage buffers) already check before they build.
fn required_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
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
fn srgb_format(
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
fn optional_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    let timing = crate::gpu_timer::timing_features();
    if adapter.features().contains(timing) {
        timing
    } else {
        wgpu::Features::empty()
    }
}
