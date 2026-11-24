//! Turning a world full of sprites into vertex and index data.

use voltra_assets::Handle;
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::{Texture, Vertex};

use crate::sprite::quad::quad;
use crate::sprite::sheets::Sheets;
use crate::sprite::Sprite;
use crate::text::{shape, Text};

pub use material_use::MaterialUse;

pub use surface::Surface;
pub use triangles::{MeshVertex, TriangleMesh};

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
    /// The normal map drawn over `texture`, for lights with a height; `None`
    /// is flat. Part of what splits a run, as the texture is.
    pub normal: Option<Handle<Texture>>,
    /// The light layers of what the run draws. Part of what splits a run.
    pub light_layers: u32,
    /// Index range into [`SpriteBatch::indices`] (and thus the uploaded mesh).
    pub indices: std::ops::Range<u32>,
    /// The material the run draws through, an index into
    /// [`SpriteBatch::materials`]; `None` for the plain sprite shader. Part
    /// of what splits a run.
    pub material: Option<u32>,
}

/// A GPU particle emitter's place in the batch's draw order. Its particles
/// are the renderer's; the batch only knows when to draw them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuEmitterDraw {
    /// The emitter's entity.
    pub entity: voltra_ecs::Entity,
    /// `None` draws against the renderer's white bind group.
    pub texture: Option<Handle<Texture>>,
    /// The light layers its particles are on.
    pub light_layers: u32,
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
    /// Four corners per sprite, in draw order.
    pub vertices: Vec<Vertex>,
    /// Triangle indices into `vertices`, six per sprite.
    pub indices: Vec<u32>,
    /// Draw-order runs of same-texture sprites, covering all of `indices`.
    ///
    /// Split only where the texture actually changes — a run never merges two
    /// sprites that draw order keeps apart, even if they share a texture.
    pub ranges: Vec<SpriteRange>,
    /// GPU particle emitters, each drawn just before the range at its
    /// `before` index — where its sort key put it among the quads.
    pub gpu_emitters: Vec<GpuEmitterDraw>,
    /// Every material and set of uniform values a range draws through,
    /// once each.
    pub materials: Vec<MaterialUse>,
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
        if let Some(vertices) = placed(self.visible, matrix, quad.corners(), quad.uvs, sprite.color)
        {
            let material = self.material(sprite.material.as_ref(), sheets);
            self.append_quad(
                vertices,
                Surface {
                    texture: quad.texture,
                    normal: sprite.normal_handle,
                    light_layers: sprite.light_layers,
                    material,
                },
            );
        }
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
        let surface = Surface::textured(Some(texture)).on(text.light_layers);
        for quad in quads {
            self.push_quad_on(matrix, quad.corners, quad.uvs, text.color, surface);
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
        self.push_quad_on(matrix, corners, uvs, color, Surface::textured(texture));
    }

    /// [`push_quad`](Self::push_quad) drawn with a whole `surface` rather
    /// than a texture alone: a tilemap's tiles, which carry the layer's
    /// normal map and light layers.
    pub fn push_quad_on(
        &mut self,
        matrix: Mat3,
        corners: [Vec2; 4],
        uvs: [[f32; 2]; 4],
        color: [f32; 4],
        surface: Surface,
    ) {
        if let Some(vertices) = placed(self.visible, matrix, corners, uvs, color) {
            self.append_quad(vertices, surface);
        }
    }

    /// Appends a quad already placed and culled by [`placed`].
    pub(crate) fn append_quad(&mut self, vertices: [Vertex; 4], surface: Surface) {
        // Every quad's indices are relative to its own first vertex; without
        // this offset each sprite would redraw the first one. `u32` because a
        // batch holds every sprite in the world, not one object's geometry.
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&vertices);

        let start = self.indices.len() as u32;
        self.indices
            .extend(QUAD_INDICES.iter().map(|offset| base + offset));
        let end = self.indices.len() as u32;

        self.extend_ranges(surface, start..end);
    }

    /// Adds `indices` drawn with `surface` to the last range when it has the
    /// same one, or as a range of its own. Never across a GPU emitter, which
    /// must draw between the two.
    pub(crate) fn extend_ranges(&mut self, surface: Surface, indices: std::ops::Range<u32>) {
        let Surface {
            texture,
            normal,
            light_layers,
            material,
        } = surface;
        let sealed = self
            .gpu_emitters
            .last()
            .is_some_and(|emitter| emitter.before == self.ranges.len());
        match self.ranges.last_mut() {
            Some(range)
                if range.texture == texture
                    && range.normal == normal
                    && range.light_layers == light_layers
                    && range.material == material
                    && !sealed =>
            {
                range.indices.end = indices.end;
            }
            _ => self.ranges.push(SpriteRange {
                texture,
                normal,
                light_layers,
                indices,
                material,
            }),
        }
    }

    /// Puts a GPU emitter's draw here in painter's order.
    pub fn push_gpu_emitter(
        &mut self,
        entity: voltra_ecs::Entity,
        texture: Option<Handle<Texture>>,
        light_layers: u32,
    ) {
        self.gpu_emitters.push(GpuEmitterDraw {
            entity,
            texture,
            light_layers,
            before: self.ranges.len(),
        });
    }

    /// How many sprite quads the batch holds.
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

/// A quad's four vertices through `matrix`, or `None` when it lies wholly
/// outside `visible`.
///
/// Culled on the corners as they will be drawn, so a rotated, scaled,
/// parented or parallax-displaced quad is tested where it really is.
pub(crate) fn placed(
    visible: Option<(Vec2, Vec2)>,
    matrix: Mat3,
    corners: [Vec2; 4],
    uvs: [[f32; 2]; 4],
    color: [f32; 4],
) -> Option<[Vertex; 4]> {
    let world = corners.map(|corner| matrix.transform_point2(corner));
    let view = Sheets {
        visible,
        ..Sheets::default()
    };
    let min = world.iter().copied().fold(Vec2::INFINITY, Vec2::min);
    let max = world.iter().copied().fold(Vec2::NEG_INFINITY, Vec2::max);
    if !view.sees(min, max) {
        return None;
    }
    let mut corner = 0;
    Some(world.map(|at| {
        let vertex = Vertex::new([at.x, at.y], color, uvs[corner]);
        corner += 1;
        vertex
    }))
}

mod build;
#[cfg(test)]
mod cull_tests;
mod material_use;
mod surface;
#[cfg(test)]
mod tests;
mod triangles;

#[cfg(test)]
mod material_tests;
