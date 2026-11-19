//! Turning a world full of sprites into vertex and index data.

use voltra_assets::Handle;
use voltra_ecs::World;
use voltra_render::glam::Mat3;
use voltra_render::{Texture, Vertex};

use crate::hierarchy::WorldTransforms;
use crate::sprite::quad::quad;
use crate::sprite::sheets::Sheets;
use crate::sprite::{draw_key, Sprite};
use crate::transform::Transform;

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
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// Draw-order runs of same-texture sprites, covering all of `indices`.
    ///
    /// Split only where the texture actually changes — a run never merges two
    /// sprites that draw order keeps apart, even if they share a texture.
    pub ranges: Vec<SpriteRange>,
}

impl SpriteBatch {
    /// Walks every entity holding both a [`Transform`] and a [`Sprite`].
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
        let mut sprites: Vec<_> = world.query2::<Transform, Sprite>().collect();
        sprites.sort_unstable_by_key(|(entity, _, sprite)| draw_key(*entity, sprite));

        // Composed once for the whole world rather than walked per sprite: a
        // deep tree would otherwise cost one matrix chain per descendant, every
        // frame.
        let transforms = WorldTransforms::from_world(world);

        let mut batch = Self::default();
        for (entity, _transform, sprite) in sprites {
            batch.push(transforms.matrix(entity), sprite, sheets);
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
        // Every quad's indices are relative to its own first vertex; without
        // this offset each sprite would redraw the first one. `u32` because a
        // batch holds every sprite in the world, not one object's geometry.
        let base = self.vertices.len() as u32;

        // One `quad` call, shared with `pick::sprite_at`. A second opinion
        // about where a sprite's corners are is a click that misses the pixels
        // it lands on, and nothing reports it.
        let quad = quad(sprite, sheets);
        for (corner, uv) in quad.corners().into_iter().zip(quad.uvs) {
            let world = matrix.transform_point2(corner);
            self.vertices.push(Vertex::new(
                [world.x, world.y],
                [sprite.color[0], sprite.color[1], sprite.color[2]],
                uv,
            ));
        }

        let start = self.indices.len() as u32;
        self.indices
            .extend(QUAD_INDICES.iter().map(|offset| base + offset));
        let end = self.indices.len() as u32;

        match self.ranges.last_mut() {
            Some(range) if range.texture == quad.texture => range.indices.end = end,
            _ => self.ranges.push(SpriteRange {
                texture: quad.texture,
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
mod tests;
