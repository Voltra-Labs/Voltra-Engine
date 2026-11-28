//! Event loop driver.

mod animation;
mod audio;
mod builder;
mod bus_layout;
mod camera_rig;
pub(crate) mod draw;
mod framing;
mod gamepads;
mod gpu_particles;
mod handler;
pub mod headless;
mod input_map;
mod loading;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod net_counters;
mod particles;
mod pending_physics;
mod prefab_sync;
mod reload;
mod restyle;
mod scene_draws;
mod scenes;
mod simulation;
mod simulation_controls;
mod skinned_meshes;
mod sorting_layers;
mod spawning;
mod startup;
mod thumbnails;
mod tick;
mod tick_audio;
mod tick_controls;
mod tick_lag;
pub mod tick_net;
mod tick_replication;
mod tick_theme;
mod tick_time;
mod ui_frame;
mod ui_input;
mod update;
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
use voltra_physics::PhysicsWorld;
use voltra_render::glam::Vec2;
use voltra_render::{LineBatch, RenderTarget, Renderer};
use voltra_scene::{ComponentRegistry, Prefabs};
use winit::event_loop::EventLoop;
use winit::window::Window;

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
    /// Collision events and broken joints the per-frame tick has not been
    /// handed yet.
    pending_physics: pending_physics::PendingPhysics,
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
    /// The 3D world's meshes and draw list, kept between frames.
    world3d: draw::world3d::World3D,
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

    /// Runs the game until its window closes.
    ///
    /// On the web this returns at once, the browser driving the loop from
    /// then on; natively it returns when the loop ends.
    pub fn run(self) {
        let event_loop = EventLoop::new().expect("failed to create event loop");
        platform::run(self, event_loop);
    }
}
