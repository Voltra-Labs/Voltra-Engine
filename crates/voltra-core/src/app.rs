//! Event loop driver.

use std::path::PathBuf;
use std::sync::Arc;

use voltra_assets::Textures;
use voltra_ecs::World;
use voltra_render::egui_backend::ScreenDescriptor;
use voltra_render::{wgpu, Camera2D, Filter, MeshDraw, RenderTarget, Renderer};
use voltra_scene::{Sprite, SpriteBatch};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::input::Input;
use crate::time::Clock;
use crate::ui::{EguiLayer, TextureId};
use crate::window::WindowConfig;

type UiFn = Box<dyn FnMut(&mut egui::Ui, &mut UiFrame<'_>)>;

/// What the UI may reach while it is being laid out.
///
/// Passing this rather than the whole `App` is what stops a panel reaching the
/// event loop or the swapchain — a UI callback that could acquire a frame would
/// deadlock the one already in flight.
pub struct UiFrame<'a> {
    /// The scene. Panels add, remove and edit entities through it.
    pub world: &'a mut World,
    /// The camera the scene was drawn with, so a viewport panel can pan it.
    pub camera: &'a mut Camera2D,
    /// Loaded textures, shared with the renderer's draws for this frame.
    ///
    /// A panel that edits a sprite's texture path resolves through this —
    /// never a `Textures` of its own — so the handle it produces is valid
    /// against the bind groups the very same frame is about to draw with.
    pub textures: &'a mut Textures,
    /// Needed to load a texture a panel just named.
    pub device: &'a wgpu::Device,
    /// Needed to upload a texture a panel just named.
    pub queue: &'a wgpu::Queue,
    /// The rendered scene, ready for `egui::Image::new`.
    viewport: TextureId,
    viewport_size: (u32, u32),
    requested_size: &'a mut (u32, u32),
}

impl UiFrame<'_> {
    /// Handle for the scene image.
    pub fn viewport(&self) -> TextureId {
        self.viewport
    }

    /// Physical size the scene was rendered at this frame.
    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport_size
    }

    /// Asks for a different scene resolution, honoured on the *next* frame.
    ///
    /// It cannot be this frame's: the scene has to be drawn before egui can
    /// sample it, and a panel only learns how much room it has once egui is
    /// already laying out. One frame of lag while dragging a splitter is the
    /// price, and it is not visible.
    pub fn request_viewport_size(&mut self, width: u32, height: u32) {
        *self.requested_size = (width.max(1), height.max(1));
    }

    /// Re-resolves every sprite's texture handle from its path.
    ///
    /// For after a world-replacing edit — Open is the only caller today.
    /// Handles from whatever was in the world before mean nothing once the
    /// entities they pointed at are gone, so this reloads every `Sprite`
    /// unconditionally rather than trying to detect which ones changed. Not
    /// for per-frame use: a path whose handle is already correct still pays
    /// for a `Textures::load` cache lookup.
    pub fn resolve_sprite_textures(&mut self) {
        resolve_world_textures(self.world, self.textures, self.device, self.queue);
    }
}

/// Re-resolves every [`Sprite`]'s texture handle from its path.
///
/// Collected before mutating: `World` has no query that yields `&mut Sprite`
/// while also handing out unrelated mutable access to itself, and
/// `set_texture` cannot run inside the iterator borrowing the world it would
/// need to call `get_mut` on.
fn resolve_world_textures(
    world: &mut World,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    let entities: Vec<_> = world
        .query::<Sprite>()
        .map(|(e, sprite)| (e, sprite.texture.clone()))
        .collect();
    for (entity, path) in entities {
        if let Some(sprite) = world.get_mut::<Sprite>(entity) {
            sprite.set_texture(path, textures, device, queue);
        }
    }
}

/// Turns a batch's texture-run ranges into the renderer's draw list.
///
/// `white` stands in for `None`: an untextured range still needs a bind
/// group, and the renderer's own white one is what every flat-coloured
/// sprite drew against before textures existed. Both `textures` and `white`
/// share `'a` so the returned draws can borrow either.
fn mesh_draws<'a>(
    batch: &SpriteBatch,
    textures: &'a Textures,
    white: &'a wgpu::BindGroup,
) -> Vec<MeshDraw<'a>> {
    batch
        .ranges
        .iter()
        .map(|range| MeshDraw {
            texture: match range.texture {
                Some(handle) => textures.bind_group(handle),
                None => white,
            },
            indices: range.indices.clone(),
        })
        .collect()
}

#[derive(Default)]
pub struct App {
    config: WindowConfig,
    // All of these stay `None` until the event loop resumes and hands us a
    // window; none of them can be built without a surface.
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    textures: Option<Textures>,
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
    }

    /// Draws the world straight to the window. No UI, no intermediate texture.
    fn redraw_without_ui(&mut self) {
        let (Some(renderer), Some(textures)) = (self.renderer.as_mut(), self.textures.as_ref())
        else {
            return;
        };
        // Rebuilt every frame. Caching it means tracking every Transform and
        // Sprite write, which is not worth it until the batch actually shows up
        // in a profile.
        let batch = SpriteBatch::from_world(&self.world);
        let mesh = batch.upload(renderer.context().device());
        // Cloned (a cheap ref-count bump — `wgpu::BindGroup` is `Clone`)
        // rather than borrowed, so the borrow on `renderer` ends here instead
        // of surviving into the `&mut self` call below.
        let white = renderer.white_bind_group().clone();
        let draws = mesh_draws(&batch, textures, &white);
        renderer.render_mesh(mesh.as_ref(), &draws);
    }

    /// Scene to an offscreen target, then the UI over the top of the window.
    fn redraw_with_ui(&mut self) {
        let (
            Some(renderer),
            Some(window),
            Some(egui),
            Some(target),
            Some(viewport),
            Some(ui),
            Some(textures),
        ) = (
            self.renderer.as_mut(),
            self.window.as_ref(),
            self.egui.as_mut(),
            self.scene_target.as_mut(),
            self.viewport,
            self.ui.as_mut(),
            self.textures.as_mut(),
        )
        else {
            return;
        };

        // Cloned handles rather than borrows: `renderer.camera` goes into the
        // UI callback mutably, which rules out holding a borrow of the renderer
        // across it. Both are reference-counted, so this costs a bump.
        let device = renderer.context().device().clone();
        let queue = renderer.context().queue().clone();

        let (width, height) = self.requested_size;
        if target.resize(&device, width, height) {
            // The old view points at a texture that no longer exists, and the
            // scene has to be projected for the panel's shape, not the window's.
            egui.update_view(&device, viewport, target.raw_view(), Filter::Linear);
            renderer.camera.aspect = target.aspect();
        }

        let batch = SpriteBatch::from_world(&self.world);
        let mesh = batch.upload(&device);
        // Cloned (a cheap ref-count bump — `wgpu::BindGroup` is `Clone`)
        // rather than borrowed, so the borrow on `renderer` ends here instead
        // of surviving into the `&mut self` call below.
        let white = renderer.white_bind_group().clone();
        let draws = mesh_draws(&batch, textures, &white);
        renderer.render_scene(target, mesh.as_ref(), &draws);

        let size = window.inner_size();
        let screen = ScreenDescriptor {
            width: size.width.max(1),
            height: size.height.max(1),
            pixels_per_point: window.scale_factor() as f32,
        };

        let mut frame = UiFrame {
            world: &mut self.world,
            camera: &mut renderer.camera,
            textures,
            device: &device,
            queue: &queue,
            viewport,
            viewport_size: (target.width(), target.height()),
            requested_size: &mut self.requested_size,
        };
        egui.prepare(window, &device, &queue, screen, |root| ui(root, &mut frame));

        renderer.present_with(|pass| egui.render(pass));
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

        // The same layout object the sprite pipeline was built with —
        // `Renderer` owns both, so `texture_layout()` is guaranteed to be it.
        let textures = Textures::new(
            renderer.context().device(),
            renderer.context().queue(),
            renderer.texture_layout(),
            PathBuf::from("assets"),
        );

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
