//! Turning a world full of sprites into vertex and index data.

use voltra_assets::Handle;
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::{Texture, Vertex};

use crate::hierarchy::WorldTransforms;
use crate::particles::{self, ParticleEmitter};
use crate::sprite::quad::quad;
use crate::sprite::sheets::Sheets;
use crate::sprite::{draw_key, Sprite};
use crate::text::{shape, Text};
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
}

/// Corners per quad. Where they land is [`quad`]'s question, not this one's.
const CORNERS_PER_QUAD: usize = 4;

/// Two triangles over those four corners.
const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

/// A contiguous run of one texture's sprites within [`SpriteBatch::indices`].
///
/// Painter's order wins over batching: sprites are never reordered to grow a
/// run, so a run only ever covers sprites that were already adjacent in draw
/// order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteRange {
    /// `None` draws against the renderer's white bind group.
    pub texture: Option<Handle<Texture>>,
    /// Index range into [`SpriteBatch::indices`] (and thus the uploaded mesh).
    pub indices: std::ops::Range<u32>,
}

/// Vertex and index data for every sprite in a world.
///
/// Built on the CPU each frame. Kept as plain `Vec`s rather than GPU buffers
/// so it can be produced and asserted on without a device.
#[derive(Debug, Default, Clone)]
pub struct SpriteBatch {
    /// The view quads are culled against, from the [`Sheets`] the batch was
    /// built with. `None` keeps every quad.
    visible: Option<(Vec2, Vec2)>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// Draw-order runs of same-texture sprites, covering all of `indices`.
    ///
    /// Split only where the texture actually changes — a run never merges two
    /// sprites that draw order keeps apart, even if they share a texture.
    pub ranges: Vec<SpriteRange>,
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
        drawn.sort_unstable_by_key(|(entity, drawn)| draw_key(*entity, drawn.sort_order()));

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

    /// Appends one quad, extending or starting a [`SpriteRange`] to match.
    ///
    /// Takes the sprite's **world** matrix, not its [`Transform`]: a parented
    /// entity's transform is local to its parent, and composing the chain is
    /// [`WorldTransforms`]'s job. Passing a matrix rather than a composed
    /// `Transform` also keeps the shear a chain of parents can produce, which
    /// three fields cannot hold.
    ///
    /// Grows the last range when this sprite's handle matches it, so two
    /// pushes in a row for the same texture draw in one call; starts a new
    /// range otherwise. Never looks further back than the last range, so a
    /// texture that reappears later still gets its own run — the same
    /// painter's-order-over-batching rule `from_world`'s sort already commits
    /// to.
    pub fn push(&mut self, matrix: Mat3, sprite: &Sprite, sheets: Sheets<'_>) {
        // One `quad` call, shared with `pick::sprite_at`. A second opinion
        // about where a sprite's corners are is a click that misses the pixels
        // it lands on, and nothing reports it.
        let quad = quad(sprite, sheets);
        self.push_quad(matrix, quad.corners(), quad.uvs, sprite.color, quad.texture);
    }

    /// Appends one quad per glyph of `text`, all sampling its font's atlas.
    ///
    /// Nothing without fonts in `sheets`, or before the atlas's first upload:
    /// with no texture to sample, a glyph would draw as a solid block.
    pub fn push_text(&mut self, matrix: Mat3, text: &Text, sheets: Sheets<'_>) {
        let Some(fonts) = sheets.fonts else {
            return;
        };
        let (quads, Some(texture)) = shape::glyphs(text, fonts) else {
            return;
        };
        for quad in quads {
            self.push_quad(matrix, quad.corners, quad.uvs, text.color, Some(texture));
        }
    }

    /// Appends one quad given in its owner's space, extending or starting a
    /// [`SpriteRange`] to match. What [`SpriteBatch::push`] does once it knows
    /// where a sprite's corners are, and what a tilemap does once per tile.
    ///
    /// `corners` in [`Quad::corners`](crate::sprite::quad::Quad::corners)
    /// order, with `uvs` matching them.
    pub fn push_quad(
        &mut self,
        matrix: Mat3,
        corners: [Vec2; 4],
        uvs: [[f32; 2]; 4],
        color: [f32; 4],
        texture: Option<Handle<Texture>>,
    ) {
        let world = corners.map(|corner| matrix.transform_point2(corner));
        // Culled on the corners as they will be drawn, so a rotated, scaled,
        // parented or parallax-displaced quad is tested where it really is.
        let view = Sheets {
            visible: self.visible,
            ..Sheets::default()
        };
        let min = world.iter().copied().fold(Vec2::INFINITY, Vec2::min);
        let max = world.iter().copied().fold(Vec2::NEG_INFINITY, Vec2::max);
        if !view.sees(min, max) {
            return;
        }

        // Every quad's indices are relative to its own first vertex; without
        // this offset each sprite would redraw the first one. `u32` because a
        // batch holds every sprite in the world, not one object's geometry.
        let base = self.vertices.len() as u32;

        for (world, uv) in world.into_iter().zip(uvs) {
            self.vertices
                .push(Vertex::new([world.x, world.y], color, uv));
        }

        let start = self.indices.len() as u32;
        self.indices
            .extend(QUAD_INDICES.iter().map(|offset| base + offset));
        let end = self.indices.len() as u32;

        match self.ranges.last_mut() {
            Some(range) if range.texture == texture => range.indices.end = end,
            _ => self.ranges.push(SpriteRange {
                texture,
                indices: start..end,
            }),
        }
    }

    pub fn sprite_count(&self) -> usize {
        self.vertices.len() / CORNERS_PER_QUAD
    }

    /// Uploads the batch, or `None` when there is nothing to draw.
    ///
    /// Allocates fresh buffers every call. A persistent growable buffer
    /// written with `Queue::write_buffer` is the obvious next step, and worth
    /// doing once a frame's sprite count justifies it.
    pub fn upload(&self, device: &voltra_render::wgpu::Device) -> Option<voltra_render::Mesh> {
        if self.is_empty() {
            return None;
        }
        Some(voltra_render::Mesh::indexed(
            device,
            "sprite-batch",
            &self.vertices,
            &self.indices,
        ))
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

#[cfg(test)]
mod cull_tests;
#[cfg(test)]
mod tests;
