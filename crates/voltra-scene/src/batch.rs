//! Turning a world full of sprites into quad instances and triangles.

use voltra_assets::Handle;
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::{MeshBuffers, Primitives, QuadInstance, Texture, Vertex};

use crate::sprite::quad::quad;
use crate::sprite::sheets::Sheets;
use crate::sprite::Sprite;
use crate::text::{shape, Text};

pub use material_use::MaterialUse;
pub(crate) use placed::{culled, instance, placed_corners, unchecked};

pub use skinned_draw::SkinnedDraw;
pub use surface::Surface;
pub use triangles::{MeshVertex, TriangleMesh};

/// Two triangles over a quad's four corners, for a quad drawn as triangles
/// (one that is no parallelogram, which an instance cannot draw).
pub(crate) const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

/// A contiguous run of one surface's quads within [`SpriteBatch::quads`],
/// or of its triangles within [`SpriteBatch::indices`].
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
    /// What the run draws: a range of [`SpriteBatch::quads`] or of
    /// [`SpriteBatch::indices`] (and thus of the uploaded mesh's). One kind
    /// a run: the two draw through different pipelines.
    pub primitives: Primitives,
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

/// Quad instances, vertices and indices for everything a world draws.
///
/// Built on the CPU each frame. Kept as plain `Vec`s rather than GPU buffers
/// so it can be produced and asserted on without a device.
///
/// A quad — a sprite, a tile, a glyph, a CPU particle, a piece of the
/// interface — is one [`QuadInstance`], its corners made by the vertex
/// shader: 64 bytes rather than four vertices and six indices' 152, which
/// is most of what building and uploading 100k sprites cost (stage 108).
/// Triangle meshes — lines, polygons, trails — stay vertices and indices.
#[derive(Debug, Default, Clone)]
pub struct SpriteBatch {
    /// The view quads are culled against, from the [`Sheets`] the batch was
    /// built with. `None` keeps every quad.
    visible: Option<(Vec2, Vec2)>,
    /// One instance per quad, in draw order.
    pub quads: Vec<QuadInstance>,
    /// The corners of triangle meshes, in draw order.
    pub vertices: Vec<Vertex>,
    /// Triangle indices into `vertices`, three a triangle.
    pub indices: Vec<u32>,
    /// Draw-order runs of same-surface quads or triangles, covering all of
    /// `quads` and `indices`.
    ///
    /// Split only where the texture actually changes — a run never merges two
    /// sprites that draw order keeps apart, even if they share a texture.
    pub ranges: Vec<SpriteRange>,
    /// GPU particle emitters, each drawn just before the range at its
    /// `before` index — where its sort key put it among the quads.
    pub gpu_emitters: Vec<GpuEmitterDraw>,
    /// Skinned meshes, placed among the ranges and the emitters the same
    /// way; see [`SkinnedDraw`].
    pub skinned: Vec<SkinnedDraw>,
    /// The range count when a draw of another pipeline (a GPU emitter, a
    /// skinned mesh) was last put in: no range may grow across it.
    sealed: Option<usize>,
    /// Every material and set of uniform values a range draws through,
    /// once each.
    pub materials: Vec<MaterialUse>,
    /// The world's matrices as last composed, kept so the next build
    /// composes into the same storage.
    transforms: crate::hierarchy::WorldTransforms,
    /// How many drawables the last build sorted: the next one's list is
    /// made that large up front.
    drawn_hint: usize,
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
        let placed = unchecked(matrix, quad.corners(), quad.uvs, sprite.color);
        if let Some(placed) = culled(self.visible, placed) {
            let material = self.material(sprite.material.as_ref(), sheets);
            self.append_quad(
                placed,
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
        for icon in shape::icons_at(text, fonts, text.px(), text.max_width) {
            let surface = Surface::textured(Some(icon.texture)).on(text.light_layers);
            self.push_quad_on(matrix, icon.corners, icon.uvs, icon.color, surface);
        }
        let (quads, Some(texture)) = shape::glyphs(text, fonts) else {
            return;
        };
        let surface = Surface::textured(Some(texture)).on(text.light_layers);
        for quad in quads {
            self.push_quad_on(matrix, quad.corners, quad.uvs, quad.color, surface);
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
        match instance(matrix, corners, uvs, color) {
            Some(quad) => {
                if let Some(quad) = culled(self.visible, quad) {
                    self.append_quad(quad, surface);
                }
            }
            // Four corners no instance can draw: two triangles, culled as
            // drawn, like any mesh.
            None => {
                if let Some(vertices) = placed_corners(self.visible, matrix, corners, uvs, color) {
                    self.append_corners(vertices, surface);
                }
            }
        }
    }

    /// [`push_quad_on`](Self::push_quad_on) for corners and UVs known to be
    /// parallelograms — a tile's, a rectangle sampling a rectangle of its
    /// sheet however the tile is turned — without asking.
    pub(crate) fn push_rect_on(
        &mut self,
        matrix: Mat3,
        corners: [Vec2; 4],
        uvs: [[f32; 2]; 4],
        color: [f32; 4],
        surface: Surface,
    ) {
        if let Some(quad) = culled(self.visible, unchecked(matrix, corners, uvs, color)) {
            self.append_quad(quad, surface);
        }
    }

    /// Appends an instance already placed and culled.
    pub(crate) fn append_quad(&mut self, quad: QuadInstance, surface: Surface) {
        let at = self.quads.len() as u32;
        self.quads.push(quad);
        self.extend_ranges(surface, Primitives::Quads(at..at + 1));
    }

    /// Appends four corners as two triangles.
    fn append_corners(&mut self, vertices: [Vertex; 4], surface: Surface) {
        // Every quad's indices are relative to its own first vertex; without
        // this offset each one would redraw the first. `u32` because a
        // batch holds a world, not one object's geometry.
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&vertices);
        let start = self.indices.len() as u32;
        self.indices
            .extend(QUAD_INDICES.iter().map(|offset| base + offset));
        let end = self.indices.len() as u32;
        self.extend_ranges(surface, Primitives::Triangles(start..end));
    }

    /// Adds `primitives` drawn with `surface` to the last range when it has
    /// the same surface and draws the same kind, ending where these start;
    /// as a range of its own otherwise. Never across a GPU emitter, which
    /// must draw between the two.
    pub(crate) fn extend_ranges(&mut self, surface: Surface, primitives: Primitives) {
        let Surface {
            texture,
            normal,
            light_layers,
            material,
        } = surface;
        let sealed = self.sealed == Some(self.ranges.len());
        if let Some(range) = self.ranges.last_mut() {
            let same = range.texture == texture
                && range.normal == normal
                && range.light_layers == light_layers
                && range.material == material
                && !sealed;
            if same {
                match (&mut range.primitives, &primitives) {
                    (Primitives::Quads(run), Primitives::Quads(next))
                    | (Primitives::Triangles(run), Primitives::Triangles(next))
                        if run.end == next.start =>
                    {
                        run.end = next.end;
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.ranges.push(SpriteRange {
            texture,
            normal,
            light_layers,
            primitives,
            material,
        });
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
        self.sealed = Some(self.ranges.len());
    }

    /// How many quads the batch holds as instances.
    pub fn sprite_count(&self) -> usize {
        self.quads.len()
    }

    /// Uploads the batch into fresh buffers, or `None` when there is
    /// nothing to draw. For a one-off draw — a screenshot, a test; a draw
    /// path building a batch every frame writes it into kept buffers with
    /// [`upload_into`](Self::upload_into) instead.
    pub fn upload(&self, device: &voltra_render::wgpu::Device) -> Option<voltra_render::Mesh> {
        if self.is_empty() {
            return None;
        }
        Some(voltra_render::Mesh::batch(
            device,
            "sprite-batch",
            &self.vertices,
            &self.indices,
            &self.quads,
        ))
    }

    /// The batch written into `buffers`, kept from last frame and grown
    /// when too small, or `None` when there is nothing to draw: no buffer
    /// made or destroyed on a steady frame.
    pub fn upload_into<'b>(
        &self,
        device: &voltra_render::wgpu::Device,
        queue: &voltra_render::wgpu::Queue,
        buffers: &'b mut MeshBuffers,
    ) -> Option<&'b voltra_render::Mesh> {
        buffers.write(device, queue, &self.vertices, &self.indices, &self.quads)
    }

    /// No quads and no triangles. A batch of nothing but GPU emitters is
    /// empty and still draws them.
    pub fn is_empty(&self) -> bool {
        self.quads.is_empty() && self.indices.is_empty()
    }

    /// Every quad's four corners as vertices, in draw order: what the
    /// vertex shader makes of [`quads`](Self::quads), for asserting on.
    pub fn quad_vertices(&self) -> Vec<Vertex> {
        self.quads.iter().flat_map(QuadInstance::corners).collect()
    }
}

mod build;
#[cfg(test)]
mod cull_tests;
mod material_use;
mod placed;
mod skinned_draw;
mod surface;
#[cfg(test)]
mod tests;
mod triangles;

#[cfg(test)]
mod material_tests;
