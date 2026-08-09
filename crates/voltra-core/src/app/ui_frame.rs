//! The seam a UI callback sees for one frame, and the plumbing behind it.
//!
//! `UiFrame` hands a panel the world, the camera, and just enough of the GPU
//! to resolve a texture — never the renderer or the swapchain, so a panel
//! cannot reach the frame already in flight. `resolve_world_textures` exists
//! only to back `UiFrame::resolve_sprite_textures`.

use voltra_assets::Textures;
use voltra_ecs::World;
use voltra_render::{wgpu, Camera2D, LineBatch};
use voltra_scene::Sprite;

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
    /// The rendered scene, ready for `egui::Image::new`.
    pub(super) viewport: TextureId,
    pub(super) viewport_size: (u32, u32),
    pub(super) requested_size: &'a mut (u32, u32),
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

    /// The overlay a panel draws into: world-unit endpoints, pixel widths.
    ///
    /// Drawn over the scene image, under the same camera, so a segment lines up
    /// with the sprite it points at without the panel converting anything.
    pub fn lines(&mut self) -> &mut LineBatch {
        self.lines
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
