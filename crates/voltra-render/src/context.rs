//! Device, queue and swapchain ownership.

use wgpu::SurfaceTarget;

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
    /// What the adapter can do below full WebGPU: whether compute and
    /// vertex-stage storage, which GPU particles need, are there.
    downlevel: wgpu::DownlevelFlags,
}

impl GpuContext {
    /// `target` is anything `wgpu` can build a surface from — typically an
    /// `Arc<winit::window::Window>`. Blocks on adapter/device acquisition,
    /// which happens once at startup.
    pub fn new(target: impl Into<SurfaceTarget<'static>>, width: u32, height: u32) -> Self {
        // `_from_env` honours WGPU_BACKEND / WGPU_POWER_PREF for debugging.
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());

        let surface = instance
            .create_surface(target)
            .expect("failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("no compatible GPU adapter found");

        log::info!("adapter: {:?}", adapter.get_info());

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("voltra-device"),
            ..Default::default()
        }))
        .expect("failed to request device");

        let config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .expect("surface not supported by adapter");
        surface.configure(&device, &config);
        let downlevel = adapter.get_downlevel_capabilities().flags;

        Self {
            surface: Some(surface),
            device,
            queue,
            config,
            downlevel,
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
            config,
            downlevel,
        })
    }

    /// Whether there is a window to present to.
    pub fn has_surface(&self) -> bool {
        self.surface.is_some()
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn downlevel(&self) -> wgpu::DownlevelFlags {
        self.downlevel
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
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

    pub fn present(&self, frame: wgpu::SurfaceTexture) {
        self.queue.present(frame);
    }
}
