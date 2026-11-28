//! Device, queue and swapchain ownership.

use wgpu::SurfaceTarget;

use crate::frame_buffers::FrameBuffers;

mod adapter;
mod formats;

use adapter::{optional_features, present_mode, required_limits, srgb_format};
pub use formats::TargetSupport;

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
    /// What the 3D shot's targets may be: see [`TargetSupport`].
    targets: TargetSupport,
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
        // Said rather than left to the default config, which takes the
        // first mode the adapter lists: vsync on, the one mode every
        // surface has, so `vsync()` is true from the first frame.
        config.present_mode = present_mode(true);
        surface.configure(&device, &config);
        let downlevel = adapter.get_downlevel_capabilities().flags;
        let targets = TargetSupport::of(&adapter, device.features(), format);

        Self {
            surface: Some(surface),
            device,
            queue,
            config,
            format,
            downlevel,
            frame_buffers: FrameBuffers::default(),
            targets,
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
        let targets = TargetSupport::of(&adapter, device.features(), config.format);
        Some(Self {
            surface: None,
            device,
            queue,
            format: config.format,
            config,
            downlevel,
            frame_buffers: FrameBuffers::default(),
            targets,
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

    /// What the device can draw a 3D shot into: see [`TargetSupport`].
    pub fn target_support(&self) -> &TargetSupport {
        &self.targets
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

    /// Whether frames wait for the display's refresh: the surface's present
    /// mode. Off is wgpu's `AutoNoVsync` — immediate, else mailbox, else
    /// the vsync every surface has — so asking can never fail; it answers
    /// what was asked, not which of the three the driver gave. A browser
    /// paces frames itself whatever the mode.
    pub fn vsync(&self) -> bool {
        !matches!(
            self.config.present_mode,
            wgpu::PresentMode::AutoNoVsync
                | wgpu::PresentMode::Immediate
                | wgpu::PresentMode::Mailbox
        )
    }

    /// Turns vsync on or off by reconfiguring the surface with the matching
    /// present mode — see [`vsync`](Self::vsync). Asking for what it already
    /// is does nothing.
    pub fn set_vsync(&mut self, on: bool) {
        if self.vsync() == on {
            return;
        }
        self.config.present_mode = present_mode(on);
        self.reconfigure();
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
