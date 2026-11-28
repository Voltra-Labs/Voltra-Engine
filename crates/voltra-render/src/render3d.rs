//! The 3D pass: views, meshes and instances drawn depth-tested — the twin
//! of the 2D sprite pass, beside it rather than inside it.
//!
//! **2D is never 3D with z = 0.** Nothing here is reached by the 2D path:
//! the sprite pass keeps no depth buffer and `Vertex2D` stays 2D. A 3D
//! camera draws through [`Pass3D`]: reversed Z into a `Depth32Float`
//! target, every instance of a mesh in one instanced draw, culled on the
//! CPU against the view's [`Frustum`] by the scene side's draw list. Unlit
//! until surface materials arrive (stage 137).

mod bounds;
mod depth;
mod frustum;
mod hdr;
mod instance;
mod mesh;
mod mesh_data;
mod pass;
mod pipeline;
mod scene;
mod targets;
mod tonemap;
mod vertex;
mod view;
mod winding;

pub use bounds::Aabb3;
pub use depth::{DepthTarget, DEPTH_CLEAR, DEPTH_COMPARE, DEPTH_FORMAT};
pub use frustum::Frustum;
pub use hdr::{Hdr3D, HdrDegraded};
pub use instance::{Instance3D, InstanceRefused};
pub use mesh::Mesh3D;
pub use mesh_data::{MeshData3D, MeshDataError};
pub use pass::Pass3D;
pub use pipeline::UnlitPipeline3D;
pub use scene::{Look3D, MeshDraw3D, Scene3D};
pub use targets::Targets3D;
pub use tonemap::Tonemap;
pub use vertex::Vertex3D;
pub use view::View3D;
pub use winding::Winding;
