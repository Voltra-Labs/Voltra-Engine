//! The assets a draw reads looked up first: materials, and the icons rich
//! texts name.

use super::super::App;

impl App {
    /// Points every sprite and tile layer naming a material at it — a
    /// spawned one, a path edited — before a draw reads them.
    pub(in crate::app) fn resolve_materials(&mut self) {
        if let Some(materials) = self.materials.as_mut() {
            voltra_scene::sprite_material::resolve_world(&mut self.world, materials);
        }
    }

    /// Loads the atlases and textures rich texts' `[icon=…]` tags name —
    /// every frame, as a language switch can bring an icon; a scan for the
    /// tag when nothing changed.
    pub(in crate::app) fn resolve_text_icons(&mut self) {
        if let (Some(renderer), Some(atlases), Some(textures)) = (
            self.renderer.as_ref(),
            self.atlases.as_mut(),
            self.textures.as_mut(),
        ) {
            voltra_scene::text::icons::resolve_world(
                &mut self.world,
                atlases,
                textures,
                renderer.context().device(),
                renderer.context().queue(),
            );
        }
    }
}
