//! What picks a surface pipeline: the shader, the vertex layout, the
//! alpha class, the sides, the winding and the pass.

use super::standard::AlphaMode;
use crate::render3d::winding::Winding;

/// What a surface's alpha does to its pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum AlphaClass {
    /// Written, depth-tested and written.
    #[default]
    Opaque,
    /// Cut out: discarded under the cutoff, or alpha to coverage under
    /// MSAA.
    Mask,
    /// Blended premultiplied, depth-tested, not written.
    Blend,
}

impl AlphaClass {
    /// The class of `mode`.
    pub fn of(mode: AlphaMode) -> Self {
        match mode {
            AlphaMode::Opaque => Self::Opaque,
            AlphaMode::Mask(_) => Self::Mask,
            AlphaMode::Blend => Self::Blend,
        }
    }
}

/// Which pass a surface pipeline draws for. The main colour pass now; the
/// shadow (140) and picking (146) passes are its later variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SurfacePass {
    /// The camera's colour and depth.
    #[default]
    Main,
}

/// What a surface pipeline's vertex stage reads. A mesh's own vertices
/// and the instance's placement now; skinned meshes (155) are its later
/// variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SurfaceVertex {
    /// [`Vertex3D`] and [`Instance3D`].
    #[default]
    Plain,
}

/// What picks a surface pipeline, within a bank made for one target
/// format and sample count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceKey {
    /// The shader's revision ([`SurfaceShader::revision`]): the standard
    /// one, or a custom material's.
    pub shader: u64,
    /// What the vertex stage reads.
    pub vertex: SurfaceVertex,
    /// Opaque, masked or blended.
    pub alpha: AlphaClass,
    /// Both faces drawn.
    pub double_sided: bool,
    /// Which way the instances' fronts wind ([`Winding::of`]).
    pub winding: Winding,
    /// The pass drawn for.
    pub pass: SurfacePass,
}
