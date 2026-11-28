//! The assets a panel resolves and reads through the frame: handles for a
//! world or some entities, sprite shapes, the sheets sprites draw from.

use voltra_ecs::Entity;
use voltra_scene::sprite::sheets::Sheets;

use super::{resolve_world_atlases, resolve_world_clips, resolve_world_textures, UiFrame};

impl UiFrame<'_> {
    /// Re-resolves every runtime asset handle in the world from its path:
    /// textures, atlases and clips.
    ///
    /// For after a world-replacing edit — Open and undo are the callers.
    /// Handles from whatever was in the world before mean nothing once the
    /// entities they pointed at are gone, so this reloads every component
    /// unconditionally rather than trying to detect which ones changed. Not
    /// for per-frame use: a path whose handle is already correct still pays
    /// for a store's cache lookup.
    ///
    /// Named for the whole job rather than the first part of it. It has
    /// resolved atlases since sheets existed and clips since sound did, and a
    /// name that still said `sprite_textures` would be the one place a caller
    /// could reasonably believe a scene had loaded when a third of it had not.
    pub fn resolve_scene_assets(&mut self) {
        resolve_world_textures(self.world, self.textures, self.device, self.queue);
        resolve_world_atlases(self.world, self.atlases);
        voltra_scene::text::resolve_fonts(self.world, self.fonts);
        resolve_world_clips(self.world, self.clips);
    }

    /// [`UiFrame::resolve_scene_assets`] for just `entities`.
    ///
    /// What a panel that spawned a handful of entities wants: a prefab
    /// dropped into a large level must not pay for re-resolving the level.
    /// A despawned entity is skipped.
    pub fn resolve_assets_of(&mut self, entities: &[Entity]) {
        for &entity in entities {
            if !self.world.is_alive(entity) {
                continue;
            }
            crate::app::spawning::resolve_entity(
                self.world,
                entity,
                Some((self.textures, self.device, self.queue)),
                Some(self.atlases),
                Some(self.fonts),
                Some(self.clips),
            );
        }
    }

    /// The stores a sprite's geometry resolves against, as the batch has them.
    ///
    /// A panel picking an entity has to ask the same question the draw asked,
    /// or a click lands somewhere other than the pixels it appears to.
    /// What a caster shaped by its sprite needs to find its outline, when
    /// silhouettes are loaded.
    pub fn sprite_shapes(&self) -> Option<voltra_scene::shadow_caster::SpriteShapes<'_>> {
        self.silhouettes
            .as_deref()
            .map(|silhouettes| voltra_scene::shadow_caster::SpriteShapes {
                sheets: self.sheets(),
                silhouettes,
            })
    }

    /// `entity`'s sprite outline in its own space, traced now if it has not
    /// been: what a collider is fitted to. Empty without silhouettes, a
    /// textured sprite or any opaque texel.
    pub fn sprite_silhouette(
        &mut self,
        entity: voltra_ecs::Entity,
    ) -> Vec<Vec<voltra_render::glam::Vec2>> {
        let Some(silhouettes) = self.silhouettes.as_deref_mut() else {
            return Vec::new();
        };
        let sheets = Sheets::new(self.atlases, self.textures)
            .with_fonts(self.fonts)
            .seen_from(self.camera.position)
            .with_layers(self.sorting_layers);
        voltra_scene::shadow_caster::sprite_silhouette(
            self.world,
            entity,
            voltra_assets::silhouette::ALPHA_THRESHOLD,
            sheets,
            silhouettes,
        )
    }

    /// The sprite sheets the world is drawn with: atlases, textures and fonts, seen from the camera.
    pub fn sheets(&self) -> Sheets<'_> {
        Sheets::new(self.atlases, self.textures)
            .with_fonts(self.fonts)
            .seen_from(self.camera.position)
            .with_layers(self.sorting_layers)
    }
}
