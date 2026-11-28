//! What one 3D frame draws: a view, the instances, and the draws over them.

use std::ops::Range;

use super::instance::Instance3D;
use super::mesh::Mesh3D;
use super::view::View3D;
use super::winding::Winding;

/// One instanced draw: `mesh`, once per instance in `instances` — a range
/// of [`Scene3D::instances`], all of one [`Winding`].
#[derive(Debug, Clone)]
pub struct MeshDraw3D<'a> {
    /// The mesh drawn.
    pub mesh: &'a Mesh3D,
    /// Which of the frame's instances draw it.
    pub instances: Range<u32>,
    /// Which way those instances' fronts wind ([`Winding::of`]): the
    /// pipeline variant they are drawn with.
    pub winding: Winding,
}

/// A 3D frame's whole input: what the scene side's draw list resolves to.
#[derive(Debug, Clone)]
pub struct Scene3D<'a> {
    /// The camera's view.
    pub view: View3D,
    /// Every instance drawn this frame, in draw order.
    pub instances: &'a [Instance3D],
    /// The draws, in order; each names a range of `instances`.
    pub draws: &'a [MeshDraw3D<'a>],
    /// What the colour target is cleared to first, linear.
    pub clear: wgpu::Color,
}
