//! Turning a world full of sprites into vertex and index data.

use voltra_assets::Handle;
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::{Texture, Vertex};

use crate::sprite::quad::quad;
use crate::sprite::sheets::Sheets;
use crate::sprite::Sprite;
use crate::text::{shape, Text};

/// Corners per quad. Where they land is [`quad`]'s question, not this one's.
pub(crate) const CORNERS_PER_QUAD: usize = 4;

/// Two triangles over those four corners.
pub(crate) const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

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

/// A GPU particle emitter's place in the batch's draw order. Its particles
/// are the renderer's; the batch only knows when to draw them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuEmitterDraw {
    pub entity: voltra_ecs::Entity,
    /// `None` draws against the renderer's white bind group.
    pub texture: Option<Handle<Texture>>,
    /// Drawn before `ranges[before]`, or after every range when it is
    /// `ranges.len()`.
    pub before: usize,
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
    /// GPU particle emitters, each drawn just before the range at its
    /// `before` index — where its sort key put it among the quads.
    pub gpu_emitters: Vec<GpuEmitterDraw>,
}

impl SpriteBatch {
    /// Appends one quad, extending or starting a [`SpriteRange`] to match.
    ///
    /// Takes the sprite's **world** matrix, not its [`Transform`]: a parented
    /// entity's transform is local to its parent, and composing the chain is
    /// [`WorldTransforms`](crate::hierarchy::WorldTransforms)'s job. Passing a matrix rather than a composed
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

        self.extend_ranges(texture, start..end);
    }

    /// Adds `indices` drawn with `texture` to the last range when it is the
    /// same texture, or as a range of its own. Never across a GPU emitter,
    /// which must draw between the two.
    pub(crate) fn extend_ranges(
        &mut self,
        texture: Option<Handle<Texture>>,
        indices: std::ops::Range<u32>,
    ) {
        let sealed = self
            .gpu_emitters
            .last()
            .is_some_and(|emitter| emitter.before == self.ranges.len());
        match self.ranges.last_mut() {
            Some(range) if range.texture == texture && !sealed => range.indices.end = indices.end,
            _ => self.ranges.push(SpriteRange { texture, indices }),
        }
    }

    /// Puts a GPU emitter's draw here in painter's order.
    pub fn push_gpu_emitter(
        &mut self,
        entity: voltra_ecs::Entity,
        texture: Option<Handle<Texture>>,
    ) {
        self.gpu_emitters.push(GpuEmitterDraw {
            entity,
            texture,
            before: self.ranges.len(),
        });
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

    /// No quads. A batch of nothing but GPU emitters is empty and still
    /// draws them.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

mod build;
#[cfg(test)]
mod cull_tests;
#[cfg(test)]
mod tests;
