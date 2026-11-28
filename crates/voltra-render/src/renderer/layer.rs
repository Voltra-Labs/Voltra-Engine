//! What a frame draws over its scene in the target's own pixels: the HUD.

use crate::mesh::Mesh2D;
use crate::pass::MeshDraw;

/// Geometry drawn in a target's own pixels over the scene - y down, origin
/// top-left - with the sprite pipeline. [`Layer::EMPTY`] draws nothing.
#[derive(Clone, Copy)]
pub struct Layer<'a> {
    /// The mesh the draws index into, or `None` for nothing.
    pub mesh: Option<&'a Mesh2D>,
    /// The index ranges to draw, in order.
    pub draws: &'a [MeshDraw<'a>],
}

impl Layer<'_> {
    /// A layer that draws nothing.
    pub const EMPTY: Layer<'static> = Layer {
        mesh: None,
        draws: &[],
    };
}
