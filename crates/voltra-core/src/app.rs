//! Event loop driver.

mod animation;
mod audio;
mod camera_rig;
mod draw;
mod framing;
mod gamepads;
mod gpu_particles;
mod input_map;
mod particles;
mod prefab_sync;
mod scene_draws;
mod scenes;
mod simulation;
mod sorting_layers;
mod spawning;
mod thumbnails;
mod tick;
mod tick_audio;
mod ui_frame;
mod ui_input;

use std::path::PathBuf;
use std::sync::Arc;

use voltra_assets::{AssetPath, AssetWatcher, Atlases, Clips, Fonts, Textures};
use voltra_audio::Audio;
use voltra_ecs::{Entity, World};
use voltra_physics::{CollisionEvent, Contact, PhysicsWorld};
use voltra_render::glam::Vec2;
use voltra_render::{Filter, LineBatch, RenderTarget, Renderer};
use voltra_scene::{ComponentRegistry, Prefabs};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
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
pub use input_map::DEFAULT_INPUT_MAP;
pub use scenes::SceneLoad;
pub use sorting_layers::DEFAULT_SORTING_LAYERS;
pub use tick::Tick;
pub use tick_audio::TickAudio;
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
    atlases: Option<Atlases>,
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
    /// The game's buttons: hover, press, focus and this frame's clicks.
    ui_buttons: voltra_scene::UiButtons,
    /// The game's save slots, in its per-user folder.
    saves: crate::saves::Saves,
    /// Scene changes and additive loads the ticks asked for, and what each
    /// additive load brought.
    scenes: scenes::Scenes,
    /// Gamepads, once `resumed` has connected to the platform's API.
    gamepads: Option<gamepads::Gamepads>,
    /// The game's named actions, re-evaluated into `input` every frame.
    actions: ActionMap,
    /// The map file named by `with_input_map`; `None` reads the convention.
    input_map_path: Option<AssetPath>,
    /// Whether `actions` was handed in by `with_actions`, which the
    /// conventional file must not then overwrite.
    actions_from_code: bool,
    /// Whether frames simulate, and the one-off steps and resets a UI asks for.
    simulation: Simulation,
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
    /// The renderer's half of every emitter simulated on the GPU.
    gpu_rings: gpu_particles::GpuParticleRings,
    /// The scene. Populate it before calling [`App::run`].
    pub world: World,
    /// Which components a prefab can carry: the defaults, plus whatever a
    /// game registered through [`App::with_registry`].
    registry: ComponentRegistry,
    /// Prefabs, made on first use — see [`spawning::prefab_store`].
    prefabs: Option<Prefabs>,
    /// Property-animation clips, made on first use like the prefabs.
    animation_clips: Option<voltra_scene::AnimationClips>,
    /// Entities a tick spawned whose asset handles the loop has not resolved
    /// yet. Emptied right after every tick.
    spawned: Vec<Entity>,
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

    /// Names the game, which names its per-user saves folder —
    /// `%APPDATA%/<name>/saves` on Windows; see
    /// [`saves::dir`](crate::saves::dir). Without this, the window title is
    /// the name.
    pub fn with_game_name(mut self, name: &str) -> Self {
        self.saves = crate::saves::Saves::for_game(name);
        self
    }

    /// Sorts drawables by `names`, back to front, instead of Background,
    /// Default, Foreground. `Default` is kept — every drawable naming no
    /// layer is on it.
    pub fn with_sorting_layers<S: Into<String>>(
        mut self,
        names: impl IntoIterator<Item = S>,
    ) -> Self {
        self.sorting_layers = voltra_scene::SortingLayers::new(names);
        self.sorting_layers_from_code = true;
        self
    }

    /// Keeps the save slots in `dir` instead of the per-user folder: a
    /// portable build, or a test.
    pub fn with_save_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.saves = crate::saves::Saves::in_dir(dir);
        self
    }

    /// The game's save slots, for setup code that reads one before the first
    /// tick.
    pub fn saves(&self) -> &crate::saves::Saves {
        &self.saves
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

    /// Reads prefabs through `registry` instead of the default one.
    ///
    /// What a game with components of its own needs: a prefab carrying a
    /// `Coin` marker spawns it only if the registry knows the name, and keeps
    /// it unread otherwise. Start from [`ComponentRegistry::with_defaults`]
    /// and register on top, or the engine's own components stop loading.
    pub fn with_registry(mut self, registry: ComponentRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Mixes through `buses` instead of the default master, music, sfx and
    /// ui.
    ///
    /// A builder rather than a settings file, because there is no settings
    /// file: the table is code the game owns. Names a scene uses that are not
    /// in it still work — see [`Buses::id`](voltra_audio::Buses::id).
    pub fn with_buses(mut self, buses: voltra_audio::Buses) -> Self {
        self.audio = std::mem::take(&mut self.audio).with_buses(buses);
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

    /// What began and ended touching in the last physics step.
    pub fn events(&self) -> &[CollisionEvent] {
        self.physics_world.events()
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
        // Before the frame's actions are derived, so a pad press is seen the
        // frame it happened, like a key's.
        if let Some(pads) = self.gamepads.as_mut() {
            pads.poll(&mut self.input);
        }
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
    fn advance(&mut self, delta: f32) {
        // Once per frame, before anything reads an action: the fixed tick
        // below sees the same actions as the per-frame one.
        self.input.update_actions(&self.actions);
        // After the actions, which navigate the focus, and before the tick,
        // which reads this frame's clicks.
        self.update_ui_buttons();
        self.run_update(delta);
        // Before anything else looks at what the tick made: a spawned sprite
        // is drawn, and a spawned source started, with its handles in place.
        self.resolve_spawned();
        // After the tick, so a clip the game just switched to is on screen this
        // frame rather than next.
        self.advance_animations(delta);
        self.update_camera_rigs(delta);
        self.advance_particles(delta);
        // After the tick too, and for the matching reason: a source the game
        // just spawned or moved is heard from where it is now.
        self.update_audio();
        // Spent, whether or not a game installed a tick to read them. Left to
        // pile up they would grow for the length of the session, and a game
        // that installed its hook later would be handed a backlog.
        self.pending_events.clear();
        // Every frame, simulating or not: the editor draws and picks against
        // a solid layer's colliders while authoring too, and a layer painted
        // by the tick above must be solid in the step that follows it.
        voltra_scene::tilemap::sync_colliders(&mut self.world);
        self.step_physics(delta);
        // The fixed tick spawns too, from inside the steps.
        self.resolve_spawned();
        // Last: a scene change despawns what the ticks and steps were holding,
        // so it waits until they have let go.
        self.apply_scene_requests();
    }

    /// Applies whatever the watcher saw since the last frame.
    ///
    /// Between the clock and the render, so a texture that changed this frame
    /// is on screen this frame rather than next. Costs one non-blocking
    /// `try_recv` when nothing changed, which is almost every frame.
    fn reload_changed_assets(&mut self) {
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
            let reloaded = match self.clips.as_mut() {
                Some(clips) => match clips.reload(&path) {
                    true => clips.by_path_handle(&path),
                    false => None,
                },
                None => None,
            };
            if let Some(clip) = reloaded {
                self.scene_audio.restart(&self.world, clip, &mut self.audio);
                continue;
            }
            // A clip is read by its players next frame.
            if self.reload_animation_clip(&path) {
                continue;
            }
            // A prefab's instances take the edit, but for what they override.
            if self.reload_prefab(&path) {
                continue;
            }
            // Nor do the bindings.
            if self.reload_input_map(&path) {
                continue;
            }
            if self.reload_sorting_layers(&path) {
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
        // Instances first, so what they are brought in step with is resolved
        // with the rest.
        self.sync_all_instances();
        ui_frame::resolve_world_textures(
            &mut self.world,
            &mut textures,
            renderer.context().device(),
            renderer.context().queue(),
        );

        // The slicing half of the same problem, and for the same reason: a
        // sprite naming an atlas has never met an `Atlases` either.
        let mut atlases = Atlases::new(asset_root.clone());
        ui_frame::resolve_world_atlases(&mut self.world, &mut atlases);
        self.atlases = Some(atlases);
        self.silhouettes = Some(voltra_assets::Silhouettes::new(asset_root.clone()));

        self.load_input_map(&asset_root);
        self.load_sorting_layers(&asset_root);
        self.gamepads = gamepads::Gamepads::connect();

        // And the legible part: a text in a world populated before `run`
        // has never met a `Fonts`.
        let mut fonts = Fonts::new(asset_root.clone());
        voltra_scene::text::resolve_fonts(&mut self.world, &mut fonts);
        self.fonts = Some(fonts);

        // The audible third of the same problem: a source in a world populated
        // before `run` has never met a `Clips` either.
        let mut clips = Clips::new(asset_root.clone());
        ui_frame::resolve_world_clips(&mut self.world, &mut clips);
        self.clips = Some(clips);

        // Last of the three, and the only one that can fail without stopping
        // anything: with no device this is the silent `Audio` and the game
        // runs on.
        self.audio.connect();

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
            let post = RenderTarget::new(
                &device,
                "scene-viewport-graded",
                format,
                size.width,
                size.height,
                Filter::Linear,
            );
            let mut egui = EguiLayer::new(&window, &device, format);
            self.viewport = Some(egui.register_view(&device, post.raw_view(), Filter::Linear));
            self.requested_size = (target.width(), target.height());
            self.scene_target = Some(target);
            self.post_target = Some(post);
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
