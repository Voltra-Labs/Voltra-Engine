//! Event loop driver.

mod draw;
mod framing;
mod simulation;
mod thumbnails;
mod tick;
mod ui_frame;

use std::path::PathBuf;
use std::sync::Arc;

use voltra_assets::{AssetWatcher, Textures};
use voltra_ecs::World;
use voltra_physics::{Contact, PhysicsWorld};
use voltra_render::glam::Vec2;
use voltra_render::{Filter, LineBatch, RenderTarget, Renderer};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::app::framing::SceneFraming;
use crate::app::simulation::Simulation;
use crate::app::thumbnails::Thumbnails;
use crate::app::tick::TickFn;
use crate::input::Input;
use crate::time::Clock;
use crate::ui::{EguiLayer, TextureId};
use crate::window::WindowConfig;
pub use tick::Tick;
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
    /// The egui id of every loaded texture, so a panel can draw one.
    ///
    /// Filled before each layout rather than on demand: the layout callback
    /// runs inside `EguiLayer::prepare`, which holds the layer mutably.
    thumbnails: Thumbnails,
    /// Size the UI last asked the scene to be rendered at.
    requested_size: (u32, u32),
    ui: Option<UiFn>,
    /// The game's per-frame tick, run before the frame's physics steps.
    update: Option<TickFn>,
    /// The game's fixed tick, run before each physics step.
    fixed_update: Option<TickFn>,
    clock: Clock,
    input: Input,
    /// Whether frames simulate, and the one-off steps and resets a UI asks for.
    simulation: Simulation,
    /// Points the frame at the scene's camera when there is no UI.
    ///
    /// Unused on the editor's path: there the viewport panel decides which
    /// camera the frame is drawn through, because it has an editor camera to
    /// choose between and a place to say so.
    framing: SceneFraming,
    /// The simulation: its fixed clock, its tuning, and the contact impulses
    /// that have to survive from one step to the next.
    physics_world: PhysicsWorld,
    /// World gravity, in world units per second squared.
    ///
    /// Negative y is down, matching the scene's axes. A game that wants
    /// top-down movement sets this to zero rather than reaching for a flag.
    pub gravity: Vec2,
    /// Overlay segments, emptied and rebuilt by the UI every frame.
    lines: LineBatch,
    /// The scene. Populate it before calling [`App::run`].
    pub world: World,
}

/// Earth-ish gravity, in world units per second squared.
///
/// Negative y is down, matching the scene's axes. Not zero, because a physics
/// world that does nothing until a magic number is typed teaches nothing about
/// whether physics is working.
const DEFAULT_GRAVITY: Vec2 = Vec2::new(0.0, -9.81);

impl App {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            // Set here rather than in `Default`, which cannot express a
            // non-zero field without hand-writing the whole impl for a struct
            // that is otherwise all `None`.
            gravity: DEFAULT_GRAVITY,
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

    /// Starts with the world live: physics steps, and the game's ticks run.
    ///
    /// Off by default: a scene with no `RigidBody` pays nothing either way, but
    /// an editor that started simulating the moment a body was added would move
    /// the thing being placed out from under the cursor, and one that ran game
    /// logic while authoring would edit the scene behind the author's back —
    /// which is why Unity runs scripts in play mode only.
    ///
    /// This is the *initial* value of a runtime switch, not a build-time
    /// choice. A game says it once and never thinks about it again; the editor
    /// says nothing and drives [`App::set_simulating`] from its play mode.
    ///
    /// Contacts are resolved: bodies rest on each other and stacks settle. Set
    /// [`App::gravity`] to change or disable the acceleration, and reach the
    /// solver's tuning through [`App::physics_mut`].
    pub fn with_simulation(mut self) -> Self {
        self.simulation.set_running(true);
        self
    }

    /// Installs the game's per-frame tick, run once before the frame's steps.
    ///
    /// This is where input belongs: it is read once per frame, so an edge — a
    /// key that went down *this* frame — is only counted once here. Runs only
    /// while the world is live, which for a game is always and for the editor
    /// is play mode.
    pub fn with_update(mut self, update: impl FnMut(&mut Tick<'_>) + 'static) -> Self {
        self.update = Some(Box::new(update));
        self
    }

    /// Installs the game's fixed tick, run before each physics step.
    ///
    /// Zero, one or several times per frame, with `delta` always the fixed
    /// step. This is where a force or a velocity belongs: applied per frame it
    /// would be scaled by the frame rate, which is the bug Unity's
    /// `FixedUpdate` and Godot's `_physics_process` exist to prevent.
    pub fn with_fixed_update(mut self, fixed: impl FnMut(&mut Tick<'_>) + 'static) -> Self {
        self.fixed_update = Some(Box::new(fixed));
        self
    }

    /// Whether each frame runs the physics steps it owes.
    pub fn set_simulating(&mut self, simulating: bool) {
        self.simulation.set_running(simulating);
    }

    pub fn is_simulating(&self) -> bool {
        self.simulation.is_running()
    }

    /// Runs `count` fixed steps on the next frame regardless of the switch.
    ///
    /// What a paused editor's Step button asks for. Additive, so two presses in
    /// one frame run two steps rather than one.
    pub fn request_steps(&mut self, count: u32) {
        self.simulation.request_steps(count);
    }

    /// The last physics step's contacts.
    pub fn contacts(&self) -> &[Contact] {
        self.physics_world.contacts()
    }

    /// The simulation, to tune or to reset after loading a scene.
    pub fn physics_mut(&mut self) -> &mut PhysicsWorld {
        &mut self.physics_world
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().expect("failed to create event loop");
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self).expect("event loop failed");
    }

    /// One simulation step, between the events arriving and `Input::end_frame`
    /// wiping the per-frame sets.
    ///
    /// Deliberately empty of camera *navigation*. How a scene is flown around
    /// is the editor's business, not the platform layer's; `voltra-editor`
    /// does it from the viewport panel. A game reads [`Input`] and moves its
    /// own camera — or authors a [`Camera`] into the scene, which the no-UI
    /// draw path frames through.
    ///
    /// [`Camera`]: voltra_scene::Camera
    fn update(&mut self) {
        let delta = self.clock.tick();
        self.advance(delta.as_secs_f32());
        self.reload_changed_assets();
    }

    /// One frame of world time: the game's tick, then the steps it owes.
    ///
    /// Separated from [`App::update`] so a test can hand it a delta instead of
    /// waiting for one.
    ///
    /// **The game's tick runs before the steps, not after.** Unity's order is
    /// the other way round — `FixedUpdate`, then `Update` — which costs a
    /// velocity set from input one step of latency before it is integrated.
    /// Nothing here has to inherit that: input is read at the top of the frame,
    /// so the frame that saw the key is the frame that moves.
    fn advance(&mut self, delta: f32) {
        self.run_update(delta);
        self.step_physics(delta);
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

        let device = renderer.context().device();
        for path in watcher.drain() {
            if !textures.reload(device, renderer.context().queue(), &path) {
                continue;
            }
            // The handle survived the swap; the view behind it did not. A
            // thumbnail still pointing at the old view draws a texture that has
            // been destroyed, which is a validation error rather than a stale
            // picture.
            let (Some(egui), Some(handle)) = (self.egui.as_mut(), textures.by_path_handle(&path))
            else {
                continue;
            };
            self.thumbnails.refresh(egui, device, textures, handle);
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
