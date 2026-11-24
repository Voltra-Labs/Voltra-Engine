//! A sprite cut into a mesh whose vertices are held by bones: a limb that
//! bends smoothly at the joint instead of two cut-out parts hinged there.
//!
//! Unity's `SpriteSkin` over a sprite's mesh and Godot's `Polygon2D` bound
//! to a `Skeleton2D`. The component stores everything the file needs — the
//! mesh in the entity's bind-pose space, each vertex's texture point and its
//! up to four bone weights, and the bones by path from the skeleton root —
//! so painted weights round-trip through the scene file. The mesh is made
//! from the sprite's traced outline ([`generate`]), weighted by distance to
//! the bones to start ([`weights::automatic`]) and painted from there
//! ([`weights::paint`]). Deformed on the GPU (`voltra_render::skinning`):
//! the CPU computes one matrix per bone per frame ([`pose`]).

pub mod generate;
pub mod pose;
pub mod weights;

use serde::{Deserialize, Serialize};
use voltra_assets::{AssetPath, Handle, Textures};
use voltra_render::glam::Vec2;
use voltra_render::skinning::{MAX_BONES, RIGID_SLOT};
use voltra_render::{SkinVertex, Texture};

/// The most bones that hold one vertex.
pub const MAX_INFLUENCES: usize = 4;

/// One vertex of a skinned mesh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkinPoint {
    /// Where, in the entity's space, in the bind pose.
    pub position: Vec2,
    /// Where in the texture, `(0, 0)` top-left.
    pub uv: Vec2,
    /// Which of the component's bones hold it and how much: at most
    /// [`MAX_INFLUENCES`], an index into [`SkinnedSprite::bones`]. Empty
    /// is held by none — it stays where the entity puts it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weights: Vec<(u32, f32)>,
}

/// A skinned sprite's triangles.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SkinMesh {
    /// The corners.
    pub vertices: Vec<SkinPoint>,
    /// Three indices into `vertices` each.
    pub triangles: Vec<[u32; 3]>,
}

/// A sprite drawn as a mesh deformed by a skeleton's bones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkinnedSprite {
    /// The image, by path.
    #[serde(default)]
    pub texture: Option<AssetPath>,
    /// What `texture` resolves to in this session's textures. Never written.
    #[serde(skip)]
    pub texture_handle: Option<Handle<Texture>>,
    /// Linear RGBA, multiplied into the texture.
    pub color: [f32; 4],
    /// Texels per unit the mesh was made at: what *Generate mesh* sizes the
    /// outline by.
    #[serde(default = "default_pixels_per_unit")]
    pub pixels_per_unit: f32,
    /// The bones that hold it, by path of names from the skeleton's root;
    /// a weight's index is into this list. At most
    /// [`voltra_render::skinning::MAX_BONES`].
    #[serde(default)]
    pub bones: Vec<String>,
    /// The mesh, in the entity's space in the bind pose, with its weights.
    #[serde(default)]
    pub mesh: SkinMesh,
    /// Draw order among sprites. See [`Sprite::sort_order`](crate::Sprite::sort_order).
    #[serde(default)]
    pub sort_order: i32,
    /// The named layer it draws on. See [`SortingLayers`](crate::sorting::SortingLayers).
    #[serde(
        default = "crate::sorting::default_layer",
        skip_serializing_if = "crate::sorting::is_default_layer"
    )]
    pub sorting_layer: String,
    /// The light layers it is on, as a sprite's are.
    #[serde(
        default = "crate::light::default_light_layers",
        skip_serializing_if = "crate::light::is_default_light_layers"
    )]
    pub light_layers: u32,
}

fn default_pixels_per_unit() -> f32 {
    100.0
}

impl Default for SkinnedSprite {
    /// White, no texture, no mesh, no bones: *Generate mesh* fills it in.
    fn default() -> Self {
        Self {
            texture: None,
            texture_handle: None,
            color: [1.0; 4],
            pixels_per_unit: default_pixels_per_unit(),
            bones: Vec::new(),
            mesh: SkinMesh::default(),
            sort_order: 0,
            sorting_layer: crate::sorting::default_layer(),
            light_layers: crate::light::default_light_layers(),
        }
    }
}

impl SkinnedSprite {
    /// The mesh as the GPU draws it: vertices with their bone slots and
    /// weights (a weight on a bone past the list or past
    /// [`MAX_BONES`] dropped; a vertex held by nothing given wholly to the
    /// mesh's own placement), and the triangles' indices. Empty when an
    /// index points past the vertices.
    pub fn gpu_mesh(&self) -> (Vec<SkinVertex>, Vec<u32>) {
        let count = self.mesh.vertices.len() as u32;
        if self.mesh.triangles.iter().flatten().any(|&i| i >= count) {
            log::warn!("a skinned mesh's triangle points past its {count} vertices; not drawn");
            return (Vec::new(), Vec::new());
        }
        let usable = self.bones.len().min(MAX_BONES) as u32;
        let vertices = self
            .mesh
            .vertices
            .iter()
            .map(|point| {
                let mut bones = [RIGID_SLOT, 0, 0, 0];
                let mut weights = [0.0; 4];
                for (slot, &(bone, weight)) in point
                    .weights
                    .iter()
                    .filter(|(bone, _)| *bone < usable)
                    .take(MAX_INFLUENCES)
                    .enumerate()
                {
                    bones[slot] = bone;
                    weights[slot] = weight;
                }
                SkinVertex {
                    position: point.position.to_array(),
                    uv: point.uv.to_array(),
                    bones,
                    weights,
                }
                .sanitized()
            })
            .collect();
        let indices = self.mesh.triangles.iter().flatten().copied().collect();
        (vertices, indices)
    }

    /// Sets or clears the texture path and refreshes its handle, as
    /// [`Sprite::set_texture`](crate::Sprite::set_texture) does.
    pub fn set_texture(
        &mut self,
        path: Option<AssetPath>,
        textures: &mut Textures,
        device: &voltra_render::wgpu::Device,
        queue: &voltra_render::wgpu::Queue,
    ) {
        self.texture_handle = path.as_ref().map(|path| textures.load(device, queue, path));
        self.texture = path;
    }
}

#[cfg(test)]
mod tests;
