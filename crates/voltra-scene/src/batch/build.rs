//! Collecting a world's drawables in draw order and pushing each into the
//! batch.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Mat3;

use super::{SpriteBatch, Surface};
use crate::line2d::Line2D;
use crate::particles::{self, ParticleEmitter};
use crate::polygon2d::Polygon2D;
use crate::skinned::{pose, SkinnedSprite};
use crate::sprite::sheets::Sheets;
use crate::sprite::{draw_key, Sprite};
use crate::text::Text;
use crate::tilemap::{mesh, TileMeshCache, Tilemap};
use crate::trail2d::Trail2D;
use crate::transform::Transform;

/// What the batch draws one of: every kind sorts by the same key.
enum Drawn<'a> {
    Sprite(&'a Sprite),
    Tilemap(&'a Tilemap),
    Text(&'a Text),
    Particles(&'a ParticleEmitter),
    Line(&'a Line2D),
    Polygon(&'a Polygon2D),
    Trail(&'a Trail2D),
    Skinned(&'a SkinnedSprite),
}

impl<'a> Drawn<'a> {
    fn sort_order(&self) -> i32 {
        match self {
            Self::Sprite(sprite) => sprite.sort_order,
            Self::Tilemap(map) => map.sort_order,
            Self::Text(text) => text.sort_order,
            Self::Particles(emitter) => emitter.sort_order,
            Self::Line(line) => line.sort_order,
            Self::Polygon(polygon) => polygon.sort_order,
            Self::Trail(trail) => trail.sort_order,
            Self::Skinned(skinned) => skinned.sort_order,
        }
    }

    fn sorting_layer(&self) -> &'a str {
        match self {
            Self::Sprite(sprite) => &sprite.sorting_layer,
            Self::Tilemap(map) => &map.sorting_layer,
            Self::Text(text) => &text.sorting_layer,
            Self::Particles(emitter) => &emitter.sorting_layer,
            Self::Line(line) => &line.sorting_layer,
            Self::Polygon(polygon) => &polygon.sorting_layer,
            Self::Trail(trail) => &trail.sorting_layer,
            Self::Skinned(skinned) => &skinned.sorting_layer,
        }
    }
}

impl SpriteBatch {
    /// Walks every entity holding a [`Transform`] and a [`Sprite`] or a
    /// [`Tilemap`].
    ///
    /// Collected and sorted rather than pushed as we go. With no depth buffer,
    /// the order geometry is written in is the order it is drawn in — and
    /// `query2` yields sparse-set storage order, which shifts when an unrelated
    /// entity is despawned. `Entity::index` breaks ties so sprites sharing a
    /// `sort_order` keep a stable order too; without it the common case, where
    /// everything sits on 0, would be exactly as unstable as before.
    ///
    /// Sorted with the unstable variant: `draw_key` is unique per entity, so
    /// there are never any equal keys for stability to preserve, and a
    /// dropped tiebreaker fails louder without it.
    pub fn from_world(world: &World, sheets: Sheets<'_>) -> Self {
        let mut batch = Self::default();
        batch.fill(world, sheets, None);
        batch
    }

    /// [`SpriteBatch::from_world`], keeping tilemap chunks in `tiles` from
    /// one build to the next: the same batch, built for what changed.
    pub fn from_world_cached(world: &World, sheets: Sheets<'_>, tiles: &mut TileMeshCache) -> Self {
        let mut batch = Self::default();
        batch.fill(world, sheets, Some(tiles));
        batch
    }

    /// [`SpriteBatch::from_world_cached`] into this batch, keeping its
    /// buffers. What a draw path calling it every frame wants: a frame's
    /// batch is megabytes once a level is in view, and allocating it afresh
    /// costs more than the cached chunks take to copy in.
    pub fn rebuild(&mut self, world: &World, sheets: Sheets<'_>, tiles: &mut TileMeshCache) {
        self.fill(world, sheets, Some(tiles));
    }

    /// Empties the batch and builds `sheets`' frame of `world` into it.
    fn fill<'a>(
        &mut self,
        world: &'a World,
        sheets: Sheets<'_>,
        mut tiles: Option<&mut TileMeshCache>,
    ) {
        // Sprites and tilemap layers in one list, because they share one draw
        // order: a layer behind the player and one in front of it are both
        // just a `sort_order` relative to the player's sprite.
        // A `UiRect` puts an entity on the screen instead: the screen-space
        // pass draws it, and drawing it here too would put a second copy in
        // the world at its stale transform. Asked of each entity only in a
        // world that has any.
        let any_ui = world.query::<crate::UiRect>().next().is_some();
        // Keyed once per entity, not once per comparison: a key names its
        // sorting layer, and looking a name up in the project's list on each
        // of the n·log n comparisons was most of a large scene's batch.
        // Neighbours mostly share a layer, so the last one's rank is kept.
        let mut last_rank: Option<(&str, i32)> = None;
        let mut drawn: Vec<(_, Entity, Drawn<'_>)> = Vec::with_capacity(self.drawn_hint);
        let mut add = |entity: Entity, item: Drawn<'a>| {
            if any_ui && crate::ui::is_ui(world, entity) {
                return;
            }
            let layer = item.sorting_layer();
            let rank = match last_rank {
                Some((name, rank)) if name == layer => rank,
                _ => {
                    let rank = sheets.rank(layer);
                    last_rank = Some((layer, rank));
                    rank
                }
            };
            drawn.push((draw_key(entity, rank, item.sort_order()), entity, item));
        };
        // One plain loop per kind rather than one iterator chained through
        // eight: the chain's every step asked each level whether it was
        // done, which cost more than the sprites it carried (stage 108).
        for (entity, _, sprite) in world.query2::<Transform, Sprite>() {
            add(entity, Drawn::Sprite(sprite));
        }
        for (entity, _, map) in world.query2::<Transform, Tilemap>() {
            add(entity, Drawn::Tilemap(map));
        }
        for (entity, _, text) in world.query2::<Transform, Text>() {
            add(entity, Drawn::Text(text));
        }
        for (entity, _, emitter) in world.query2::<Transform, ParticleEmitter>() {
            add(entity, Drawn::Particles(emitter));
        }
        for (entity, _, line) in world.query2::<Transform, Line2D>() {
            add(entity, Drawn::Line(line));
        }
        for (entity, _, polygon) in world.query2::<Transform, Polygon2D>() {
            add(entity, Drawn::Polygon(polygon));
        }
        for (entity, _, trail) in world.query2::<Transform, Trail2D>() {
            add(entity, Drawn::Trail(trail));
        }
        for (entity, _, skinned) in world.query2::<Transform, SkinnedSprite>() {
            add(entity, Drawn::Skinned(skinned));
        }
        drawn.sort_unstable_by_key(|(key, _, _)| *key);
        self.drawn_hint = drawn.len();

        // Composed once for the whole world rather than walked per sprite: a
        // deep tree would otherwise cost one matrix chain per descendant, every
        // frame.
        let mut transforms = std::mem::take(&mut self.transforms);
        transforms.recompose_seen_from(world, sheets.camera);

        self.visible = sheets.visible;
        self.quads.clear();
        self.vertices.clear();
        self.indices.clear();
        self.ranges.clear();
        self.gpu_emitters.clear();
        self.skinned.clear();
        self.sealed = None;
        let batch = self;
        // Asked once: a world with no repeating layer draws every entity
        // once, without looking for one above it.
        let repeating = world
            .query::<crate::Parallax>()
            .any(|(_, layer)| layer.repeat.is_some());
        if let Some(tiles) = tiles.as_deref_mut() {
            tiles.begin();
        }
        for &(_, entity, ref drawn) in &drawn {
            let matrix = transforms.matrix(entity);
            // A repeating layer's copies, each moved by its offset; once,
            // where it is, otherwise — without a list made per entity.
            let repeats;
            let copies: &[voltra_render::glam::Vec2] = if repeating {
                repeats = crate::parallax::copies_of(world, entity, matrix, sheets.visible);
                &repeats
            } else {
                &[voltra_render::glam::Vec2::ZERO]
            };
            for (copy, &offset) in copies.iter().enumerate() {
                let matrix = if repeating {
                    Mat3::from_translation(offset) * matrix
                } else {
                    matrix
                };
                match drawn {
                    Drawn::Sprite(sprite) => batch.push(matrix, sprite, sheets),
                    Drawn::Tilemap(map) => match tiles.as_deref_mut() {
                        Some(tiles) => {
                            mesh::push_cached(batch, matrix, map, sheets, tiles.layer(entity, copy))
                        }
                        None => mesh::push(batch, matrix, map, sheets),
                    },
                    Drawn::Text(text) => batch.push_text(matrix, text, sheets),
                    Drawn::Line(line) => batch.push_triangles(
                        matrix,
                        &line.mesh(),
                        Surface::textured(line.texture_handle).on(line.light_layers),
                        sheets,
                    ),
                    Drawn::Polygon(polygon) => batch.push_triangles(
                        matrix,
                        &polygon.mesh(),
                        Surface::textured(polygon.texture_handle).on(polygon.light_layers),
                        sheets,
                    ),
                    // In the world already, and once: the ribbon is where the
                    // entity has been, not where a copy of it is.
                    Drawn::Trail(trail) if copy == 0 => batch.push_triangles(
                        Mat3::IDENTITY,
                        &trail.mesh(matrix.transform_point2(voltra_render::glam::Vec2::ZERO)),
                        Surface::textured(trail.texture_handle).on(trail.light_layers),
                        sheets,
                    ),
                    Drawn::Trail(_) => {}
                    // Once, where its bones are: the GPU deforms it.
                    Drawn::Skinned(skinned) if copy == 0 => {
                        let posed =
                            pose::skin_pose(world, entity, skinned, &|e| transforms.matrix(e));
                        batch.push_skinned(
                            entity,
                            skinned.texture_handle,
                            skinned.light_layers,
                            &pose::deformed(skinned, &posed),
                        );
                    }
                    Drawn::Skinned(_) => {}
                    Drawn::Particles(emitter) => {
                        // What the CPU still holds — all of it, on the CPU;
                        // what is left from before a switch, on the GPU.
                        particles::mesh::push(batch, matrix, emitter, sheets.atlases);
                        // One draw however many repeats: the particles are
                        // where they are, not where a copy of the emitter is.
                        // Left out when nothing can be alive, or nothing
                        // alive can be in view.
                        let seen = emitter
                            .gpu_bounds(matrix)
                            .is_some_and(|(min, max)| sheets.sees(min, max));
                        if copy == 0 && emitter.gpu_capacity().is_some() && seen {
                            batch.push_gpu_emitter(
                                entity,
                                emitter.texture_handle,
                                emitter.light_layers,
                            );
                        }
                    }
                }
            }
        }
        if let Some(tiles) = tiles {
            tiles.finish();
        }
        batch.transforms = transforms;
    }
}
