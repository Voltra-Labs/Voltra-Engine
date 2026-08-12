//! The seam a UI callback sees for one frame, and the plumbing behind it.
//!
//! `UiFrame` hands a panel the world, the camera, and just enough of the GPU
//! to resolve a texture — never the renderer or the swapchain, so a panel
//! cannot reach the frame already in flight. `resolve_world_textures` exists
//! only to back `UiFrame::resolve_sprite_textures`.

use voltra_assets::Textures;
use voltra_ecs::World;
use voltra_physics::Contact;
use voltra_render::{wgpu, Camera2D, LineBatch};
use voltra_scene::Sprite;

use super::simulation::Simulation;
use crate::ui::TextureId;

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
    /// physics is off, which is the default — see [`App::with_physics`].
    ///
    /// [`App::with_physics`]: crate::app::App::with_physics
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
}

impl<'a> UiFrame<'a> {
    /// Handle for the scene image.
    pub fn viewport(&self) -> TextureId {
        self.viewport
    }

    /// Physical size the scene was rendered at this frame.
    pub fn viewport_size(&self) -> (u32, u32) {
        self.viewport_size
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

    /// Whether each frame runs the physics steps it owes.
    pub fn simulating(&self) -> bool {
        self.simulation.is_running()
    }

    /// Turns per-frame stepping on or off, from the next frame onwards.
    ///
    /// One frame of latency by construction: `App::update` runs before this
    /// callback, so a switch thrown here first applies on the next frame. At
    /// 60 Hz nobody can see it, and the alternative — the UI reaching back into
    /// the frame that already ran — is worse.
    pub fn set_simulating(&mut self, simulating: bool) {
        self.simulation.set_running(simulating);
    }

    /// Runs `count` fixed steps on the next frame regardless of the switch.
    ///
    /// Additive, so two requests in one frame run two steps.
    pub fn request_steps(&mut self, count: u32) {
        self.simulation.request_steps(count);
    }

    /// Forgets the accumulated impulses and the clock's banked time, before
    /// the next frame steps anything.
    ///
    /// For a world that is no longer the same world: an editor's Stop, which
    /// despawns and respawns every entity the scene holds.
    pub fn reset_physics(&mut self) {
        self.simulation.request_reset();
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
/// Visible to `app` as well as to [`UiFrame`] because the world can arrive
/// already populated: [`App::world`](crate::app::App::world) is public and
/// documented to be filled before `run`, and those sprites have never met a
/// [`Textures`].
pub(super) fn resolve_world_textures(
    world: &mut World,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    for (_, sprite) in world.query_mut::<Sprite>() {
        let path = sprite.texture.clone();
        sprite.set_texture(path, textures, device, queue);
    }
}
