//! egui ids for the textures the asset store holds.
//!
//! A panel that wants to draw a loaded texture cannot register it itself: the
//! layout callback runs *inside* `EguiLayer::prepare`, which already holds the
//! layer mutably. So the registration happens here, once per frame before the
//! layout, and the panel is handed a finished map.
//!
//! The consequence is the same one the viewport lives with: a texture named
//! this frame is drawable on the next. See "The viewport is one frame behind"
//! in `docs/ARCHITECTURE.md`.

use std::collections::HashMap;

use voltra_assets::{Handle, Textures};
use voltra_render::{wgpu, Filter, Texture};

use crate::ui::{EguiLayer, TextureId};

/// The egui id every loaded texture is known by, kept in step with the store.
#[derive(Default)]
pub(super) struct Thumbnails {
    ids: HashMap<Handle<Texture>, TextureId>,
}

impl Thumbnails {
    /// Registers every texture the store has gained since the last call.
    ///
    /// Cheap when nothing changed: one hash lookup per loaded texture, and no
    /// GPU work at all. A handle that vanished from the store keeps its id
    /// rather than being freed — `Textures` never removes, so the case cannot
    /// arise yet, and freeing on a guess would hand egui a dangling id.
    pub(super) fn sync(
        &mut self,
        egui: &mut EguiLayer,
        device: &wgpu::Device,
        textures: &Textures,
    ) {
        for handle in textures.loaded() {
            if self.ids.contains_key(&handle) {
                continue;
            }
            // `Filter::Linear`: a thumbnail is almost always a downscale, and
            // nearest-neighbour on a 512-pixel sprite drawn at 64 is a mess of
            // aliasing. The sprite's own sampler is untouched — this view is
            // only ever sampled by egui.
            let id = egui.register_view(device, textures.get(handle).raw_view(), Filter::Linear);
            self.ids.insert(handle, id);
        }
    }

    /// Points an existing id at the texture's current pixels.
    ///
    /// Hot reload swaps contents under a stable handle, which leaves the id
    /// correct and the *view* stale — the old one addresses a texture that no
    /// longer exists. Called with the handle whose file changed; unknown
    /// handles are ignored, because a texture nothing has drawn yet has no id
    /// to refresh.
    pub(super) fn refresh(
        &mut self,
        egui: &mut EguiLayer,
        device: &wgpu::Device,
        textures: &Textures,
        handle: Handle<Texture>,
    ) {
        let Some(&id) = self.ids.get(&handle) else {
            return;
        };
        egui.update_view(device, id, textures.get(handle).raw_view(), Filter::Linear);
    }

    /// What the UI reads. Empty until the first [`Self::sync`].
    pub(super) fn ids(&self) -> &HashMap<Handle<Texture>, TextureId> {
        &self.ids
    }
}
