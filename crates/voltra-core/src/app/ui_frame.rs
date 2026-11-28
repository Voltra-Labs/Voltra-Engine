//! The seam a UI callback sees for one frame, and the plumbing behind it.
//!
//! `UiFrame` hands a panel the world, the camera, and just enough of the GPU
//! to resolve a texture — never the renderer or the swapchain, so a panel
//! cannot reach the frame already in flight. `resolve_world_textures` exists
//! only to back `UiFrame::resolve_sprite_textures`.

use std::collections::HashMap;

use voltra_assets::{Atlases, Clips, Fonts, Handle, Textures};
use voltra_audio::Audio;
use voltra_ecs::World;
use voltra_physics::Contact;
use voltra_render::{wgpu, LineBatch, Texture, View2D};
use voltra_scene::Prefabs;

use super::simulation::Simulation;
use super::window_requests::WindowRequests;
use crate::ui::TextureId;

mod assets;
mod previews;
mod resolve;
mod simulation;
mod window;

pub(super) use resolve::{resolve_world_atlases, resolve_world_clips, resolve_world_textures};

/// What the UI may reach while it is being laid out.
///
/// Passing this rather than the whole `App` is what stops a panel reaching the
/// event loop or the swapchain — a UI callback that could acquire a frame would
/// deadlock the one already in flight.
pub struct UiFrame<'a> {
    /// The scene. Panels add, remove and edit entities through it.
    pub world: &'a mut World,
    /// The camera the scene was drawn with, so a viewport panel can pan it.
    pub camera: &'a mut View2D,
    /// Loaded textures, shared with the renderer's draws for this frame.
    ///
    /// A panel that edits a sprite's texture path resolves through this —
    /// never a `Textures` of its own — so the handle it produces is valid
    /// against the bind groups the very same frame is about to draw with.
    pub textures: &'a mut Textures,
    /// Loaded slicings, shared with this frame's draws for the same reason
    /// `textures` is: a panel that assigns an atlas must produce a handle the
    /// batch about to run can resolve.
    pub atlases: &'a mut Atlases,
    /// Sprites' traced outlines, for a panel drawing a caster shaped by its
    /// sprite or fitting a collider to one. `None` before the asset root is
    /// known.
    pub silhouettes: Option<&'a mut voltra_assets::Silhouettes>,
    /// Faces and glyph atlases, for a panel resolving a text's font.
    pub fonts: &'a mut Fonts,
    /// Sprite materials, for a panel naming one and editing its uniforms.
    /// `None` before the asset root is known.
    pub materials: Option<&'a mut voltra_assets::Materials>,
    /// The project's sorting layers, for the inspector's choice and the pick.
    pub sorting_layers: &'a voltra_scene::SortingLayers,
    /// Loaded sounds, shared with the loop's own playback for the same reason
    /// the two stores above are shared: a panel that names a clip must produce
    /// a handle the frame's audio can already resolve.
    pub clips: &'a mut Clips,
    /// Loaded prefabs, the same store a game's tick spawns from: a panel that
    /// drops one into the scene and a hot reload of its file must agree on
    /// what the file currently holds.
    pub prefabs: &'a mut Prefabs,
    /// Loaded animation clips, the store the players read: a timeline's
    /// edit is played by every player of the clip the next frame.
    pub animation_clips: &'a mut voltra_scene::AnimationClips,
    /// Loaded state machines, the store the animators read.
    pub state_machines: &'a mut voltra_scene::StateMachines,
    /// The game's own output, for a panel that auditions a sound.
    ///
    /// The same `Audio` the scene plays through rather than a second device:
    /// a preview should go through the buses and the limiter the game will,
    /// and two streams open on one device is a configuration some hosts
    /// refuse.
    pub audio: &'a mut Audio,
    /// Needed to load a texture a panel just named.
    pub device: &'a wgpu::Device,
    /// Needed to upload a texture a panel just named.
    pub queue: &'a wgpu::Queue,
    /// Segments to draw over the scene this frame.
    ///
    /// A panel pushes into this and never sees a device: `App` uploads it and
    /// records the pass after the scene and before egui samples the target, so
    /// the overlay lands in the same frame as the scene it annotates. Emptied
    /// before every layout, so a panel that stops pushing stops drawing.
    pub(super) lines: &'a mut LineBatch,
    /// What the last physics step found overlapping.
    ///
    /// Read-only: a panel draws these, it does not author them. Empty when
    /// physics is off, which is the default — see [`App::with_simulation`].
    ///
    /// [`App::with_simulation`]: crate::app::App::with_simulation
    pub(super) contacts: &'a [Contact],
    /// The simulation switch, so a play-mode panel can turn stepping on and
    /// off, ask for a single step, or clear the solver after a restore.
    ///
    /// Reached through the four methods below rather than exposed, so a panel
    /// cannot consume a pending step itself and leave `App` nothing to run.
    pub(super) simulation: &'a mut Simulation,
    /// The rendered scene, ready for `egui::Image::new`.
    pub(super) viewport: TextureId,
    pub(super) viewport_size: (u32, u32),
    pub(super) requested_size: &'a mut (u32, u32),
    /// Set by [`show_game_frame`](Self::show_game_frame): the next frame
    /// draws the viewport as the game draws its window.
    pub(super) game_frame: &'a mut bool,
    /// Loaded textures as egui knows them, for a panel that draws one.
    pub(super) thumbnails: &'a HashMap<Handle<Texture>, TextureId>,
    /// The scene as this frame drew it, graded: what `capture_viewport`
    /// reads back.
    pub(super) graded: &'a voltra_render::RenderTarget,
    /// The game's input map, and the actions a tool holds through it.
    pub(super) actions: &'a crate::input::ActionMap,
    pub(super) held_actions: &'a mut crate::input::HeldActions,
    /// The title and close the UI asks for; see `ui_frame/window.rs`.
    pub(super) window: &'a mut WindowRequests,
    /// The components ticks spawn prefabs and load scenes with. A game the
    /// editor loads registers its own here, as `with_game` does.
    pub registry: &'a mut voltra_scene::ComponentRegistry,
    /// The game the turns run; see [`UiFrame::set_game`].
    pub(super) game: &'a mut Option<crate::game::Running>,
    /// The network session, which the editor hosts its in-process clients
    /// on when it plays with them.
    pub net: &'a mut crate::Net,
    /// Whether the renderer can simulate GPU particles; see
    /// [`UiFrame::gpu_particles`].
    pub(super) gpu_particles: bool,
    /// Surface materials asked for on a sphere; see
    /// [`UiFrame::surface_preview`].
    pub(super) previews: &'a mut super::draw::world3d::SurfacePreviews,
}

impl<'a> UiFrame<'a> {
    /// The scene as this frame drew it — lit and graded, before the editor's
    /// overlay — read back to the CPU.
    pub fn capture_viewport(&self) -> crate::Screenshot {
        let (width, height) = (self.graded.width(), self.graded.height());
        let rgba = voltra_render::read_rgba8(
            self.device,
            self.queue,
            self.graded.texture(),
            width,
            height,
        );
        crate::Screenshot {
            width,
            height,
            rgba,
        }
    }

    /// Holds the game's `action` for the next `frames` frames, through its
    /// first binding, as a player would. `false` for an action the input map
    /// does not bind.
    pub fn hold_action(&mut self, action: &str, frames: u32) -> bool {
        if self.actions.bindings(action).is_empty() {
            return false;
        }
        self.held_actions.hold(action, frames);
        true
    }

    /// Handle for the scene image.
    pub fn viewport(&self) -> TextureId {
        self.viewport
    }

    /// Physical size the scene was rendered at this frame.
    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport_size
    }

    /// The id egui draws a loaded texture by, if it has one yet.
    ///
    /// `None` for the frame a texture is first named: the registration happens
    /// before the layout, and a panel that names one is by definition inside
    /// the layout. Draw a blank tile and ask again next frame — the same
    /// one-frame lag the viewport has, for the same reason.
    pub fn thumbnail(&self, handle: Handle<Texture>) -> Option<TextureId> {
        self.thumbnails.get(&handle).copied()
    }

    /// The overlay a panel draws into: world-unit endpoints, pixel widths.
    ///
    /// Drawn over the scene image, under the same camera, so a segment lines up
    /// with the sprite it points at without the panel converting anything.
    pub fn lines(&mut self) -> &mut LineBatch {
        self.lines
    }

    /// What the last physics step found overlapping.
    ///
    /// Tied to the frame's own lifetime rather than the borrow of `self`, so
    /// reading it does not keep the frame borrowed — an overlay that draws
    /// contacts needs [`UiFrame::world_and_lines`] straight afterwards.
    pub fn contacts(&self) -> &'a [Contact] {
        self.contacts
    }

    /// The scene and the overlay at once, for a draw that walks one into the
    /// other.
    ///
    /// Both are separate references inside the frame, but a caller holding a
    /// `&mut UiFrame` cannot reach them together: [`UiFrame::lines`] borrows
    /// the whole frame mutably and `world` is a field of it. Splitting them
    /// here is what makes an overlay built *from* the scene — collider
    /// outlines, and later any other debug draw — expressible at all.
    pub fn world_and_lines(&mut self) -> (&World, &mut LineBatch) {
        (self.world, self.lines)
    }

    /// Whether emitters set to the GPU can run there — what a panel
    /// stepping particles itself (a preview) passes to
    /// [`voltra_scene::particles::advance_where`].
    pub fn gpu_particles(&self) -> bool {
        self.gpu_particles
    }

    /// Asks the next frame to draw the viewport as the game draws its
    /// window — every camera of both dimensions composited, the HUD over
    /// them — and answers whether it will: an active
    /// [`Camera3D`](voltra_scene::Camera3D) draws to the screen. A 2D scene's game view is the
    /// game camera framing the 2D view, as before. Honoured the next frame,
    /// as [`request_viewport_size`](Self::request_viewport_size) is; asked
    /// every frame the game view is shown, and forgotten when it is not.
    pub fn show_game_frame(&mut self) -> bool {
        *self.game_frame = true;
        voltra_scene::camera::shots_3d(self.world)
            .iter()
            .any(|s| s.target.is_none())
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
}
