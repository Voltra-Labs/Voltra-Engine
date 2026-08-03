//! Device, queue and swapchain ownership.

use wgpu::SurfaceTarget;

/// Owns the GPU device and the swapchain surface for a single render target.
pub struct GpuContext {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
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

        Self {
            surface,
            device,
            queue,
            config,
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
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
        self.surface.configure(&self.device, &self.config);
    }

    /// Returns `None` when the frame should be skipped. Recoverable states
    /// (`Outdated`, `Lost`) reconfigure the surface so the next call succeeds.
    pub fn acquire(&mut self) -> Option<wgpu::SurfaceTexture> {
        use wgpu::CurrentSurfaceTexture as Cst;

        match self.surface.get_current_texture() {
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
