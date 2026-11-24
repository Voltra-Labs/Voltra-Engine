//! Sprite meshes deformed by bones on the GPU: each vertex carries up to
//! four bone indices and weights, and the vertex shader blends the bones'
//! matrices — linear blend skinning, what Unity's `SpriteSkin` (on its GPU
//! path) and Godot's `Polygon2D` with a skeleton do.
//!
//! One uniform block per mesh: a tint and [`MAX_BONES`] bone matrices plus
//! one more, [`RIGID_SLOT`], the mesh's own placement, for vertices no bone
//! holds. A uniform array rather than a storage buffer, so it runs on
//! WebGL2 as on everything else; 64 bones is more than a 2D character uses.

mod mesh;
mod pipeline;

pub use mesh::{SkinVertex, SkinnedMesh};
pub use pipeline::SkinPipeline;

/// The most bones one mesh is deformed by.
pub const MAX_BONES: usize = 64;

/// The bone index a vertex no bone holds is given: the slot after the
/// bones, which holds the mesh's own matrix.
pub const RIGID_SLOT: u32 = MAX_BONES as u32;
