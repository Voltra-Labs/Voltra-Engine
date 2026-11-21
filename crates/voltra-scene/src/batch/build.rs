//! Collecting a world's drawables in draw order and pushing each into the
//! batch.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Mat3;

use super::SpriteBatch;
use crate::hierarchy::WorldTransforms;
use crate::particles::{self, ParticleEmitter};
use crate::sprite::sheets::Sheets;
use crate::sprite::{draw_key, Sprite};
use crate::text::Text;
use crate::tilemap::{mesh, Tilemap};
use crate::transform::Transform;

/// What the batch draws one of: every kind sorts by the same key.
enum Drawn<'a> {
    Sprite(&'a Sprite),
    Tilemap(&'a Tilemap),
    Text(&'a Text),
    Particles(&'a ParticleEmitter),
}

impl Drawn<'_> {
    fn sort_order(&self) -> i32 {
        match self {
            Self::Sprite(sprite) => sprite.sort_order,
            Self::Tilemap(map) => map.sort_order,
            Self::Text(text) => text.sort_order,
            Self::Particles(emitter) => emitter.sort_order,
        }
    }

    fn sorting_layer(&self) -> &str {
        match self {
            Self::Sprite(sprite) => &sprite.sorting_layer,
            Self::Tilemap(map) => &map.sorting_layer,
            Self::Text(text) => &text.sorting_layer,
            Self::Particles(emitter) => &emitter.sorting_layer,
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
        // Sprites and tilemap layers in one list, because they share one draw
        // order: a layer behind the player and one in front of it are both
        // just a `sort_order` relative to the player's sprite.
        let mut drawn: Vec<(Entity, Drawn<'_>)> = world
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
            // A `UiRect` puts an entity on the screen instead: the
            // screen-space pass draws it, and drawing it here too would put a
            // second copy in the world at its stale transform.
            .filter(|(entity, _)| !crate::ui::is_ui(world, *entity))
            .collect();
        drawn.sort_unstable_by_key(|(entity, drawn)| {
            draw_key(
                *entity,
                sheets.rank(drawn.sorting_layer()),
                drawn.sort_order(),
            )
        });

        // Composed once for the whole world rather than walked per sprite: a
        // deep tree would otherwise cost one matrix chain per descendant, every
        // frame.
        let transforms = WorldTransforms::seen_from(world, sheets.camera);

        let mut batch = Self {
            visible: sheets.visible,
            ..Self::default()
        };
        // Asked once: a world with no repeating layer draws every entity
        // once, without looking for one above it.
        let repeating = world
            .query::<crate::Parallax>()
            .any(|(_, layer)| layer.repeat.is_some());
        for (entity, drawn) in drawn {
            let matrix = transforms.matrix(entity);
            let copies = if repeating {
                crate::parallax::copies_of(world, entity, matrix, sheets.visible)
            } else {
                vec![voltra_render::glam::Vec2::ZERO]
            };
            for offset in copies {
                let matrix = Mat3::from_translation(offset) * matrix;
                match drawn {
                    Drawn::Sprite(sprite) => batch.push(matrix, sprite, sheets),
                    Drawn::Tilemap(map) => mesh::push(&mut batch, matrix, map, sheets),
                    Drawn::Text(text) => batch.push_text(matrix, text, sheets),
                    Drawn::Particles(emitter) => particles::mesh::push(&mut batch, matrix, emitter),
                }
            }
        }
        batch
    }
}
