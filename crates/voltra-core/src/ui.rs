//! The egui layer: window events in, triangles out.
//!
//! egui needs a half in the platform layer and a half in the render layer.
//! `egui-winit` translates events and drives the cursor and clipboard, so it
//! belongs here next to `winit`; drawing the result belongs in `voltra-render`
//! next to `wgpu`. This module is the seam, and it is the only place the two
//! halves meet.

use voltra_render::egui_backend::{EguiBackend, ScreenDescriptor};
use voltra_render::{wgpu, Filter};
use winit::window::Window;

pub use egui::{Context, TextureId};

/// Owns the egui context, its winit translation and its wgpu backend.
pub struct EguiLayer {
    ctx: Context,
    state: egui_winit::State,
    backend: EguiBackend,
}

impl EguiLayer {
    pub fn new(window: &Window, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let ctx = Context::default();
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            // `None` means "ask the window", which is what we want: forcing a
            // scale factor makes the UI the wrong size on a HiDPI display.
            None,
            window.theme(),
            // The backend has no texture-size limit of its own beyond the
            // device's, which egui reads from the window.
            None,
        );

        Self {
            ctx,
            state,
            backend: EguiBackend::new(device, format),
        }
    }

    /// Feeds an event to egui and reports whether egui wants it.
    ///
    /// A consumed event must not also reach the engine: typing `wasd` into a
    /// text field would otherwise fly the camera across the scene.
    pub fn on_window_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    /// Runs `build` to lay the UI out, then uploads the result.
    ///
    /// Call inside the redraw, before [`Self::render`] and before the frame is
    /// acquired — this writes buffers and textures the pass will read.
    pub fn prepare(
        &mut self,
        window: &Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: ScreenDescriptor,
        build: impl FnMut(&mut egui::Ui),
    ) {
        let input = self.state.take_egui_input(window);
        // Panels dock inside a `Ui`, not a `Context`, so the callback is handed
        // the root one rather than the context.
        let output = self.ctx.run_ui(input, build);

        // Cursor icon, clipboard writes and opened links. Skipping this leaves
        // the pointer stuck as an arrow over every resize handle.
        self.state
            .handle_platform_output(window, output.platform_output);

        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        self.backend
            .prepare(device, queue, &primitives, &output.textures_delta, screen);
    }

    /// Records the UI. The pass must cover the whole `ScreenDescriptor` given
    /// to [`Self::prepare`].
    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.backend.render(pass);
    }

    /// Hands egui a texture the engine owns, such as the viewport target.
    pub fn register_view(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        filter: Filter,
    ) -> TextureId {
        self.backend.register_view(device, view, filter)
    }

    /// Points an existing id at a new view, after the target was reallocated.
    pub fn update_view(
        &mut self,
        device: &wgpu::Device,
        id: TextureId,
        view: &wgpu::TextureView,
        filter: Filter,
    ) {
        self.backend.update_view(device, id, view, filter);
    }

    pub fn context(&self) -> &Context {
        &self.ctx
    }
}
