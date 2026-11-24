//! Collecting a world's drawables in draw order and pushing each into the
//! batch.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Mat3;

use super::{SpriteBatch, Surface};
use crate::hierarchy::WorldTransforms;
use crate::line2d::Line2D;
use crate::particles::{self, ParticleEmitter};
use crate::polygon2d::Polygon2D;
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
}

impl Drawn<'_> {
    fn sort_order(&self) -> i32 {
        match self {
            Self::Sprite(sprite) => sprite.sort_order,
            Self::Tilemap(map) => map.sort_order,
            Self::Text(text) => text.sort_order,
            Self::Particles(emitter) => emitter.sort_order,
            Self::Line(line) => line.sort_order,
            Self::Polygon(polygon) => polygon.sort_order,
            Self::Trail(trail) => trail.sort_order,
        }
    }

    fn sorting_layer(&self) -> &str {
        match self {
            Self::Sprite(sprite) => &sprite.sorting_layer,
            Self::Tilemap(map) => &map.sorting_layer,
            Self::Text(text) => &text.sorting_layer,
            Self::Particles(emitter) => &emitter.sorting_layer,
            Self::Line(line) => &line.sorting_layer,
            Self::Polygon(polygon) => &polygon.sorting_layer,
            Self::Trail(trail) => &trail.sorting_layer,
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
    fn fill(&mut self, world: &World, sheets: Sheets<'_>, mut tiles: Option<&mut TileMeshCache>) {
        // Sprites and tilemap layers in one list, because they share one draw
        // order: a layer behind the player and one in front of it are both
        // just a `sort_order` relative to the player's sprite.
        let drawn: Vec<(Entity, Drawn<'_>)> = world
            .query2::<Transform, Sprite>()
            .map(|(entity, _, sprite)| (entity, Drawn::Sprite(sprite)))
            .chain(
                world
                    .query2::<Transform, Tilemap>()
                    .map(|(entity, _, map)| (entity, Drawn::Tilemap(map))),
            )
            .chain(
                world
                    .query2::<Transform, Text>()
                    .map(|(entity, _, text)| (entity, Drawn::Text(text))),
            )
            .chain(
                world
                    .query2::<Transform, ParticleEmitter>()
                    .map(|(entity, _, emitter)| (entity, Drawn::Particles(emitter))),
            )
            .chain(
                world
                    .query2::<Transform, Line2D>()
                    .map(|(entity, _, line)| (entity, Drawn::Line(line))),
            )
            .chain(
                world
                    .query2::<Transform, Polygon2D>()
                    .map(|(entity, _, polygon)| (entity, Drawn::Polygon(polygon))),
            )
            .chain(
                world
                    .query2::<Transform, Trail2D>()
                    .map(|(entity, _, trail)| (entity, Drawn::Trail(trail))),
            )
            // A `UiRect` puts an entity on the screen instead: the
            // screen-space pass draws it, and drawing it here too would put a
            // second copy in the world at its stale transform.
            .filter(|(entity, _)| !crate::ui::is_ui(world, *entity))
            .collect();
        // Keyed once per entity, not once per comparison: a key names its
        // sorting layer, and looking a name up in the project's list on each
        // of the n·log n comparisons was most of a large scene's batch.
        let mut keyed: Vec<_> = drawn
            .into_iter()
            .map(|(entity, drawn)| {
                let key = draw_key(
                    entity,
                    sheets.rank(drawn.sorting_layer()),
                    drawn.sort_order(),
                );
                (key, entity, drawn)
            })
            .collect();
        keyed.sort_unstable_by_key(|(key, _, _)| *key);
        let drawn: Vec<(Entity, Drawn<'_>)> = keyed
            .into_iter()
            .map(|(_, entity, drawn)| (entity, drawn))
            .collect();

        // Composed once for the whole world rather than walked per sprite: a
        // deep tree would otherwise cost one matrix chain per descendant, every
        // frame.
        let transforms = WorldTransforms::seen_from(world, sheets.camera);

        self.visible = sheets.visible;
        self.vertices.clear();
        self.indices.clear();
        self.ranges.clear();
        self.gpu_emitters.clear();
        let batch = self;
        // Asked once: a world with no repeating layer draws every entity
        // once, without looking for one above it.
        let repeating = world
            .query::<crate::Parallax>()
            .any(|(_, layer)| layer.repeat.is_some());
        if let Some(tiles) = tiles.as_deref_mut() {
            tiles.begin();
        }
        for &(entity, ref drawn) in &drawn {
            let matrix = transforms.matrix(entity);
            let copies = if repeating {
                crate::parallax::copies_of(world, entity, matrix, sheets.visible)
            } else {
                vec![voltra_render::glam::Vec2::ZERO]
            };
            for (copy, offset) in copies.into_iter().enumerate() {
                let matrix = Mat3::from_translation(offset) * matrix;
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
    }
}
