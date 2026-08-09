//! Event loop driver.

mod draw;
mod ui_frame;

use std::path::PathBuf;
use std::sync::Arc;

use voltra_assets::{AssetWatcher, Textures};
use voltra_ecs::World;
use voltra_render::{Filter, RenderTarget, Renderer};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::input::Input;
use crate::time::Clock;
use crate::ui::{EguiLayer, TextureId};
use crate::window::WindowConfig;
pub use ui_frame::UiFrame;

type UiFn = Box<dyn FnMut(&mut egui::Ui, &mut UiFrame<'_>)>;

#[derive(Default)]
pub struct App {
    config: WindowConfig,
    /// Where [`AssetPath`]s resolve from, when the caller has an opinion.
    ///
    /// `None` means [`voltra_assets::default_root`] decides at `resumed` time.
    /// A game that ships its assets somewhere unusual sets this; the editor
    /// does not need to.
    ///
    /// [`AssetPath`]: voltra_assets::AssetPath
    asset_root: Option<PathBuf>,
    /// Whether to watch the asset root and reload textures as files change.
    ///
    /// Off by default. A shipped game has no reason to watch its own assets,
    /// and none of Unity, Unreal, Godot or Bevy leaves this on in a build.
    hot_reload: bool,
    // All of these stay `None` until the event loop resumes and hands us a
    // window; none of them can be built without a surface.
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    textures: Option<Textures>,
    watcher: Option<AssetWatcher>,
    egui: Option<EguiLayer>,
    scene_target: Option<RenderTarget>,
    viewport: Option<TextureId>,
    /// Size the UI last asked the scene to be rendered at.
    requested_size: (u32, u32),
    ui: Option<UiFn>,
    clock: Clock,
    input: Input,
    /// The scene. Populate it before calling [`App::run`].
    pub world: World,
}

impl App {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    /// Installs the UI callback, run once per frame to lay the editor out.
    ///
    /// Without one the scene is drawn straight to the window and no UI exists,
    /// which is what a shipped game wants.
    pub fn with_ui(mut self, ui: impl FnMut(&mut egui::Ui, &mut UiFrame<'_>) + 'static) -> Self {
        self.ui = Some(Box::new(ui));
        self
    }

    /// Sets the directory every texture path resolves against.
    ///
    /// Without this, [`voltra_assets::default_root`] resolves one at startup.
    pub fn with_asset_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.asset_root = Some(root.into());
        self
    }

    /// Watches the asset root and reloads textures as their files change.
    ///
    /// The editor wants this; a shipped game does not, which is why it is not
    /// the default. A failure to start the watch is logged and otherwise
    /// ignored — the app runs, it just does not notice file changes.
    pub fn with_hot_reload(mut self) -> Self {
        self.hot_reload = true;
        self
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().expect("failed to create event loop");
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self).expect("event loop failed");
    }

    /// One simulation step, between the events arriving and `Input::end_frame`
    /// wiping the per-frame sets.
    ///
    /// Deliberately empty of camera work. How a scene is navigated is the
    /// editor's business, not the platform layer's; `voltra-editor` does it
    /// from the viewport panel. A game reads [`Input`] and moves its own
    /// camera.
    fn update(&mut self) {
        self.clock.tick();
        self.reload_changed_assets();
    }

    /// Applies whatever the watcher saw since the last frame.
    ///
    /// Between the clock and the render, so a texture that changed this frame
    /// is on screen this frame rather than next. Costs one non-blocking
    /// `try_recv` when nothing changed, which is almost every frame.
    fn reload_changed_assets(&mut self) {
        let (Some(watcher), Some(textures), Some(renderer)) = (
            self.watcher.as_mut(),
            self.textures.as_mut(),
            self.renderer.as_ref(),
        ) else {
            return;
        };

        for path in watcher.drain() {
            textures.reload(
                renderer.context().device(),
                renderer.context().queue(),
                &path,
            );
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Android and iOS resume more than once; only the first pass builds.
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(self.config.to_attributes())
                .expect("failed to create window"),
        );

        let size = window.inner_size();
        let renderer = Renderer::new(window.clone(), size.width, size.height);

        let asset_root = self
            .asset_root
            .clone()
            .unwrap_or_else(voltra_assets::default_root);
        log::info!("asset root: {}", asset_root.display());

        // The same layout object the sprite pipeline was built with —
        // `Renderer` owns both, so `texture_layout()` is guaranteed to be it.
        let mut textures = Textures::new(
            renderer.context().device(),
            renderer.context().queue(),
            renderer.texture_layout(),
            asset_root.clone(),
        );

        // `App::world` is public and documented to be populated before `run`,
        // so by the time there is a device to upload with, the world may
        // already hold sprites naming a texture they have never resolved.
        // Without this they draw untextured forever: the only other callers of
        // `set_texture` are the inspector and Open, and a game has neither.
        ui_frame::resolve_world_textures(
            &mut self.world,
            &mut textures,
            renderer.context().device(),
            renderer.context().queue(),
        );

        if self.hot_reload {
            match AssetWatcher::new(&asset_root) {
                Ok(watcher) => self.watcher = Some(watcher),
                // Not fatal. Refusing to open the editor because a watch handle
                // could not be had would trade a working session for a feature
                // nobody has used yet this run.
                Err(e) => log::error!("hot reload disabled: {e}"),
            }
        }

        if self.ui.is_some() {
            let device = renderer.context().device().clone();
            let format = renderer.context().config().format;

            // Same format as the window so the UI compositing step has nothing
            // to convert, and full window size until a panel says otherwise.
            let target = RenderTarget::new(
                &device,
                "scene-viewport",
                format,
                size.width,
                size.height,
                Filter::Linear,
            );
            let mut egui = EguiLayer::new(&window, &device, format);
            self.viewport = Some(egui.register_view(&device, target.raw_view(), Filter::Linear));
            self.requested_size = (target.width(), target.height());
            self.scene_target = Some(target);
            self.egui = Some(egui);
        }

        self.renderer = Some(renderer);
        self.textures = Some(textures);
        self.window = Some(window);

        // Device creation takes long enough to produce a huge first delta;
        // restarting the clock here keeps frame one honest.
        self.clock = Clock::new();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // egui gets first refusal. A keystroke aimed at a text field must not
        // also fly the camera, and a click on a panel must not select in the
        // scene behind it.
        let consumed = match (self.egui.as_mut(), self.window.as_ref()) {
            (Some(egui), Some(window)) => egui.on_window_event(window, &event),
            _ => false,
        };
        if !consumed {
            self.input.process_window_event(&event);
        }

        if self.renderer.is_none() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                log::info!("close requested, shutting down");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.update();

                if self.ui.is_some() {
                    self.redraw_with_ui();
                } else {
                    self.redraw_without_ui();
                }

                // Must come after everything that reads input this frame.
                self.input.end_frame();

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
