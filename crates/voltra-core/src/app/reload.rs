//! Hot reload: the files the watcher saw change, applied between the clock
//! and the render.

use super::App;

impl App {
    /// Applies whatever the watcher saw since the last frame.
    ///
    /// Between the clock and the render, so a texture that changed this frame
    /// is on screen this frame rather than next. Costs one non-blocking
    /// `try_recv` when nothing changed, which is almost every frame.
    pub(super) fn reload_changed_assets(&mut self) {
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
