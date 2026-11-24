//! Event loop driver.

mod animation;
mod audio;
mod builder;
mod bus_layout;
mod camera_rig;
mod draw;
mod framing;
mod gamepads;
mod gpu_particles;
pub mod headless;
mod input_map;
mod loading;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod net_counters;
mod particles;
mod prefab_sync;
mod restyle;
mod scene_draws;
mod scenes;
mod simulation;
mod skinned_meshes;
mod sorting_layers;
mod spawning;
mod startup;
mod thumbnails;
mod tick;
mod tick_audio;
mod tick_controls;
pub mod tick_net;
mod tick_replication;
mod tick_theme;
mod tick_time;
mod ui_frame;
mod ui_input;
#[cfg(target_arch = "wasm32")]
mod web;
mod window_requests;
mod window_seam;

#[cfg(not(target_arch = "wasm32"))]
use native as platform;
#[cfg(target_arch = "wasm32")]
use web as platform;

use std::path::PathBuf;
use std::sync::Arc;

use voltra_assets::{AssetPath, AssetWatcher, Atlases, Clips, Fonts, Textures};
use voltra_audio::Audio;
use voltra_ecs::{Entity, World};
use voltra_physics::{CollisionEvent, Contact, PhysicsWorld};
use voltra_render::glam::Vec2;
use voltra_render::{LineBatch, RenderTarget, Renderer};
use voltra_scene::{ComponentRegistry, Prefabs};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::app::audio::SceneAudio;
use crate::app::framing::SceneFraming;
use crate::app::simulation::Simulation;
use crate::app::thumbnails::Thumbnails;
use crate::app::tick::TickFn;
use crate::input::{ActionMap, Input};
use crate::time::Clock;
use crate::ui::{EguiLayer, TextureId};
use crate::window::WindowConfig;
pub use bus_layout::{parse_bus_layout, DEFAULT_BUS_LAYOUT};
pub use input_map::DEFAULT_INPUT_MAP;
pub use loading::{LoadStats, SceneChangeStatus, StreamingStats, DEFAULT_LOAD_BUDGET};
pub use net_counters::NET_COUNTERS;
pub use scenes::SceneLoad;
pub use sorting_layers::{parse_sorting_layers, DEFAULT_SORTING_LAYERS};
pub use tick::Tick;
pub use tick_audio::TickAudio;
pub use tick_net::Net;
pub use ui_frame::UiFrame;

type UiFn = Box<dyn FnMut(&mut egui::Ui, &mut UiFrame<'_>)>;

/// The application: the window, the world and the frame loop that runs
/// the game's callbacks each frame.
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
    atlases: Option<Atlases>,
    /// The materials sprites and tile layers name.
    pub(crate) materials: Option<voltra_assets::Materials>,
    /// The pipelines and uniforms they draw with; made with the first one.
    material_bank: Option<voltra_render::material::MaterialBank>,
    /// The scene and picture targets of the screen's cameras, when a frame
    /// draws several: split screen, a pixel-perfect view.
    camera_pool: Vec<(voltra_render::RenderTarget, voltra_render::RenderTarget)>,
    /// How images are filtered: the project's choice, nearest for pixel art.
    texture_filter: voltra_render::Filter,
    /// Sprites' traced outlines, for shadow casters shaped by their sprite.
    /// `None` until `resumed`, for the root, like the stores around it.
    silhouettes: Option<voltra_assets::Silhouettes>,
    /// Faces and their glyph atlases. `None` until `resumed`, for the root,
    /// like `clips`.
    fonts: Option<Fonts>,
    /// Decoded sounds. `None` until `resumed`, like the stores above — not
    /// because a clip needs a device, but because it needs the asset root,
    /// and that is resolved when the window is.
    clips: Option<Clips>,
    watcher: Option<AssetWatcher>,
    egui: Option<EguiLayer>,
    scene_target: Option<RenderTarget>,
    /// The scene after the post pass, which is what the viewport panel
    /// shows; the editor overlay is drawn over it, ungraded.
    post_target: Option<RenderTarget>,
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
    /// The game's own code, run after the hooks above in both turns: one
    /// handed to `with_game`, or the one an editor's Play put in.
    game: Option<crate::game::Running>,
    /// Collision events the per-frame tick has not been handed yet.
    ///
    /// A frame can owe two steps, and `PhysicsWorld` only keeps the last
    /// step's: without this the per-frame tick would miss the pickup taken on
    /// the first of them. Emptied once the tick has had it.
    pending_events: Vec<CollisionEvent>,
    clock: Clock,
    /// Seconds since the loop started, simulating or not: the clock animated
    /// tiles are drawn by, so water moves in the editor too, as in Godot.
    elapsed: f64,
    input: Input,
    /// The project's sorting layers, back to front.
    sorting_layers: voltra_scene::SortingLayers,
    /// Whether `sorting_layers` came from `with_sorting_layers`, which the
    /// conventional file must not then overwrite.
    sorting_layers_from_code: bool,
    /// The game's controls: hover, press, focus, and this frame's clicks,
    /// changes and submissions.
    ui_controls: voltra_scene::UiControls,
    /// Whether the window takes IME input: while a text field is focused.
    ime_allowed: bool,
    /// Properties moving to values over time, started from the ticks.
    pub(crate) tweens: voltra_scene::Tweens,
    /// Moments to come, asked for from the ticks.
    pub(crate) timers: voltra_scene::Timers,
    /// The language the game starts in, before the locale is opened; empty
    /// for English.
    language: String,
    /// The current language and its strings.
    pub(crate) locale: voltra_scene::Locale,
    /// The screen a windowless run lays its UI out on, in points, if it
    /// has one: what lets a headless test press its controls.
    headless_screen: Option<voltra_render::glam::Vec2>,
    /// The game's save slots, in its per-user folder.
    saves: crate::saves::Saves,
    /// Scene changes and additive loads the ticks asked for, and what each
    /// additive load brought.
    scenes: scenes::Scenes,
    /// Gamepads, once `resumed` has connected to the platform's API.
    gamepads: Option<gamepads::Gamepads>,
    /// The game's named actions, re-evaluated into `input` every frame.
    actions: ActionMap,
    /// The actions as the game ships them — its map file, or the map handed
    /// to `with_actions` — before the player's own controls are laid over:
    /// what a rebound control is saved as a difference from.
    default_actions: ActionMap,
    /// The map file named by `with_input_map`; `None` reads the convention.
    input_map_path: Option<AssetPath>,
    /// Whether `actions` was handed in by `with_actions`, which the
    /// conventional file must not then overwrite.
    actions_from_code: bool,
    /// Whether frames simulate, and the one-off steps and resets a UI asks for.
    simulation: Simulation,
    /// The game's network session, received before the ticks and flushed
    /// after them.
    net: tick_net::Net,
    /// The output device, or a silent stand-in when there is none.
    ///
    /// Not an `Option`: [`Audio::silent`] already is the "no device" case, and
    /// wrapping it would mean every caller answering the same question twice.
    /// `App::default` therefore needs no sound card, which is what keeps every
    /// test below headless.
    audio: Audio,
    /// Which of the scene's own sources are sounding, and from where.
    scene_audio: SceneAudio,
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
    /// Tilemap chunks kept from one frame's sprite batch to the next.
    tile_meshes: voltra_scene::TileMeshCache,
    /// The world's sprite batch, kept so each frame refills its buffers
    /// instead of allocating them.
    frame_batch: voltra_scene::SpriteBatch,
    /// The GPU buffers the world's batch is written into, kept and grown
    /// rather than made afresh each frame (stage 108).
    batch_buffers: voltra_render::MeshBuffers,
    /// The renderer's half of every emitter simulated on the GPU.
    gpu_rings: gpu_particles::GpuParticleRings,
    /// The renderer's half of every skinned sprite.
    skinned_meshes: skinned_meshes::SkinnedMeshes,
    /// The scene. Populate it before calling [`App::run`].
    pub world: World,
    /// Which components a prefab can carry: the defaults, plus whatever a
    /// game registered through [`App::with_registry`].
    pub(crate) registry: ComponentRegistry,
    /// Prefabs, made on first use — see [`spawning::prefab_store`].
    prefabs: Option<Prefabs>,
    /// Property-animation clips, made on first use like the prefabs.
    animation_clips: Option<voltra_scene::AnimationClips>,
    /// Animation state machines, made on first use like the clips.
    state_machines: Option<voltra_scene::StateMachines>,
    /// Entities a tick spawned whose asset handles the loop has not resolved
    /// yet. Emptied right after every tick.
    spawned: Vec<Entity>,
    /// Actions a tool holds down for some frames — the editor bridge's input.
    held_actions: crate::input::HeldActions,
    /// The title and close the UI asked for this frame.
    window_requests: window_requests::WindowRequests,
    /// The window as the game's ticks see it, and what they asked of it.
    game_window: crate::window::GameWindow,
    /// The renderer a browser is still building, and the page's gesture;
    /// see `app/web.rs`.
    #[cfg(target_arch = "wasm32")]
    web: web::WebState,
}

/// Earth-ish gravity, in world units per second squared.
///
/// Negative y is down, matching the scene's axes. Not zero, because a physics
/// world that does nothing until a magic number is typed teaches nothing about
/// whether physics is working.
const DEFAULT_GRAVITY: Vec2 = Vec2::new(0.0, -9.81);

impl App {
    /// An app opening a window as `config` describes, saving under a
    /// directory named after its title until told otherwise.
    pub fn new(config: WindowConfig) -> Self {
        Self {
            // Named after the window until the game says otherwise: the
            // title is the one name every app already has.
            saves: crate::saves::Saves::for_game(&config.title),
            config,
            // Set here rather than in `Default`, which cannot express a
            // non-zero field without hand-writing the whole impl for a struct
            // that is otherwise all `None`.
            gravity: DEFAULT_GRAVITY,
            ..Default::default()
        }
    }

    /// Whether each frame runs the physics steps it owes.
    pub fn set_simulating(&mut self, simulating: bool) {
        self.simulation.set_running(simulating);
    }

    /// Whether the physics steps run each frame.
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

    /// What began and ended touching in the last physics step.
    pub fn events(&self) -> &[CollisionEvent] {
        self.physics_world.events()
    }

    /// The simulation, to tune or to reset after loading a scene.
    pub fn physics_mut(&mut self) -> &mut PhysicsWorld {
        &mut self.physics_world
    }

    /// Runs the game until its window closes.
    ///
    /// On the web this returns at once, the browser driving the loop from
    /// then on; natively it returns when the loop ends.
    pub fn run(self) {
        let event_loop = EventLoop::new().expect("failed to create event loop");
        platform::run(self, event_loop);
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
        // The frame before this one ends here, its draw and present included.
        voltra_profile::end_frame();
        voltra_profile::count("entities", self.world.entity_count() as f64);
        let _update = voltra_profile::scope("update");
        // Before the frame's actions are derived, so a pad press is seen the
        // frame it happened, like a key's.
        if let Some(pads) = self.gamepads.as_mut() {
            pads.poll(&mut self.input);
        }
        // With the devices: a held action reads as a key held this frame.
        self.held_actions.tick(&mut self.input, &self.actions);
        let delta = self.clock.tick();
        self.elapsed += delta.as_secs_f64();
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
    pub(crate) fn advance(&mut self, delta: f32) {
        // Once per frame, before anything reads an action: the fixed tick
        // below sees the same actions as the per-frame one.
        let input = voltra_profile::scope("input");
        // The size touch zones divide, before the actions read them.
        if let Some(window) = self.window.as_ref() {
            let size = window.inner_size();
            self.input.set_screen(voltra_render::glam::Vec2::new(
                size.width as f32,
                size.height as f32,
            ));
        } else if let Some(screen) = self.headless_screen {
            self.input.set_screen(screen);
        }
        self.input.update_actions(&self.actions);
        // After the actions, which navigate the focus, and before the tick,
        // which reads this frame's clicks.
        self.update_ui_controls(delta);
        // A theme the controls just applied may name skins and fonts.
        self.resolve_restyled();
        drop(input);
        // Before the tick, so it reads this frame's messages and events.
        self.receive_net(delta);
        // Before the tick too, which reads what the server sent: spawned,
        // despawned and changed.
        self.receive_replication();
        // Before the tick, so it reads this frame's finished tweens and
        // fired timers, as it reads this frame's clicks.
        self.step_tweens_and_timers(delta);
        {
            let _tick = voltra_profile::scope("tick");
            self.run_update(delta);
        }
        // After the tick, which may have switched the language or spawned a
        // keyed text, and before anything draws one.
        self.locale.localize(&mut self.world);
        if let Some(fonts) = self.fonts.as_mut() {
            self.locale.apply_fonts(fonts);
        }
        // Before anything else looks at what the tick made: a spawned sprite
        // is drawn, and a spawned source started, with its handles in place.
        self.resolve_spawned();
        // After the tick, so a clip the game just switched to is on screen this
        // frame rather than next.
        {
            let _animation = voltra_profile::scope("animation");
            self.advance_animations(delta);
            self.update_camera_rigs(delta);
        }
        {
            let _particles = voltra_profile::scope("particles");
            self.advance_particles(delta);
        }
        {
            // After the tick too, and for the matching reason: a source the
            // game just spawned or moved is heard from where it is now.
            let _audio = voltra_profile::scope("audio");
            self.update_audio();
        }
        // Spent, whether or not a game installed a tick to read them. Left to
        // pile up they would grow for the length of the session, and a game
        // that installed its hook later would be handed a backlog.
        self.pending_events.clear();
        // Every frame, simulating or not: the editor draws and picks against
        // a solid layer's colliders while authoring too, and a layer painted
        // by the tick above must be solid in the step that follows it.
        {
            let _physics = voltra_profile::scope("physics");
            voltra_scene::tilemap::sync_colliders(&mut self.world);
            self.step_physics(delta);
        }
        // The fixed tick spawns too, from inside the steps.
        self.resolve_spawned();
        // Last: a scene change despawns what the ticks and steps were holding,
        // so it waits until they have let go.
        self.apply_scene_requests();
        // After the requests, which may have started a change or a world:
        // what loads over frames gets this frame's budget.
        self.update_loading();
        // After every step: the snapshot is the world as the frame left it,
        // and a remote entity is drawn where the clock says, whatever the
        // physics did to it.
        self.send_replication(delta);
        // What every tick of the frame queued goes out together.
        self.flush_net();
    }

    /// Applies whatever the watcher saw since the last frame.
    ///
    /// Between the clock and the render, so a texture that changed this frame
    /// is on screen this frame rather than next. Costs one non-blocking
    /// `try_recv` when nothing changed, which is almost every frame.
    fn reload_changed_assets(&mut self) {
        let _reload = voltra_profile::scope("asset reload");
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        let changed = watcher.drain();
        if changed.is_empty() {
            return;
        }

        // Sounds first, and on their own: a clip needs no device, so a run
        // whose renderer is not up yet still hears the edit.
        let mut images = Vec::with_capacity(changed.len());
        for path in changed {
            // A sound reaches its own clip and every clip a settings file
            // makes of it.
            let reloaded = match self.clips.as_mut() {
                Some(clips) => clips.reload(&path),
                None => Vec::new(),
            };
            if !reloaded.is_empty() {
                for clip in reloaded {
                    self.scene_audio.restart(&self.world, clip, &mut self.audio);
                }
                continue;
            }
            // A clip is read by its players next frame, a machine by its
            // animators.
            if self.reload_animation_clip(&path) || self.reload_state_machine(&path) {
                continue;
            }
            // A prefab's instances take the edit, but for what they override.
            if self.reload_prefab(&path) {
                continue;
            }
            // A material, or a shader one names: compiled again next draw.
            if self
                .materials
                .as_mut()
                .is_some_and(|materials| materials.reload(&path))
            {
                continue;
            }
            // Strings land in their texts next frame.
            if self.locale.reload(&path) {
                continue;
            }
            // Nor do the bindings.
            if self.reload_input_map(&path) {
                continue;
            }
            if self.reload_sorting_layers(&path) {
                continue;
            }
            if self.reload_bus_layout(&path) {
                continue;
            }
            // A face needs no device either: its atlas starts over and is
            // re-rasterized and uploaded by the next draw.
            if self.fonts.as_mut().is_some_and(|fonts| fonts.reload(&path)) {
                continue;
            }
            images.push(path);
        }

        let (Some(textures), Some(atlases), Some(renderer)) = (
            self.textures.as_mut(),
            self.atlases.as_mut(),
            self.renderer.as_ref(),
        ) else {
            return;
        };

        let device = renderer.context().device();
        for path in images {
            // Traced outlines go stale with their pixels, whichever store
            // answers below; they are traced again when next asked for.
            if let Some(silhouettes) = self.silhouettes.as_mut() {
                silhouettes.reload(&path);
            }
            // A re-cut sheet changes no pixels, and a repainted one changes no
            // slicing, so whichever store owns the path answers and the other
            // ignores it.
            if atlases.reload(&path) {
                continue;
            }
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

        // Held from here, so a second `resumed` while the renderer is still
        // on its way (the web) does not build a second one.
        self.window = Some(window.clone());
        platform::start_renderer(self, window);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, (): ()) {
        // The one event sent to the loop: the web's renderer, built.
        #[cfg(target_arch = "wasm32")]
        if let Some(renderer) = self.take_renderer() {
            self.finish_resume(renderer);
        }
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
        // Before the renderer check: a key pressed while the device is still
        // on its way is the page's gesture all the same.
        #[cfg(target_arch = "wasm32")]
        self.on_web_event(&event);

        if self.renderer.is_none() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                if self.window_requests.on_close_requested(self.ui.is_some()) {
                    log::info!("close requested, shutting down");
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                // A resize is also how fullscreen arriving, or leaving by
                // the platform's own key, shows.
                self.refresh_game_window();
            }
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => {
                // Another monitor, other modes.
                self.refresh_game_window();
            }
            WindowEvent::RedrawRequested => {
                self.refresh_game_window_shown();
                self.update();

                if self.ui.is_some() {
                    self.redraw_with_ui();
                } else {
                    self.redraw_without_ui();
                }

                // Must come after everything that reads input this frame.
                self.input.end_frame();

                // Between this frame and the next, as the scene changes
                // were: fullscreen, size and vsync change on a window that
                // is not being drawn into.
                self.apply_game_window();

                // A close the UI agreed to, or asked for itself.
                if self.window_requests.should_exit() {
                    log::info!("closing, as the UI asked");
                    event_loop.exit();
                    return;
                }

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}
