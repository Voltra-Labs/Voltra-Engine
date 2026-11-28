//! What one 3D frame draws: a view, the instances, and the draws over them.

use std::ops::Range;

use super::instance::Instance3D;
use super::lighting::Lighting3D;
use super::mesh::Mesh3D;
use super::surface::SurfaceMaterial3D;
use super::tonemap::Tonemap;
use super::view::View3D;
use super::winding::Winding;
use crate::post::PostSettings;

/// One instanced draw: `mesh` through `material`, once per instance in
/// `instances` — a range of [`Scene3D::instances`], all of one
/// [`Winding`].
#[derive(Debug, Clone)]
pub struct MeshDraw3D<'a> {
    /// The mesh drawn.
    pub mesh: &'a Mesh3D,
    /// What its surface is: the pipeline variant, with `winding`, and
    /// group 1.
    pub material: &'a SurfaceMaterial3D,
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
    /// What lights the surfaces.
    pub lighting: Lighting3D,
    /// Seconds since the game started, what a surface's `in.time` reads:
    /// wrapped at an hour ([`FrameUniform3D::TIME_WRAP`](super::FrameUniform3D::TIME_WRAP)),
    /// so an `f32` keeps a tenth of a millisecond.
    pub time: f32,
}

/// How a 3D shot's light becomes its picture: the grade, bloom and
/// exposure included, then the curve. [`Look3D::NEUTRAL`] changes nothing
/// — a flat colour reads back as given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look3D {
    /// The tone curve, after exposure and before the grade.
    pub tonemap: Tonemap,
    /// Exposure, bloom and the grade, as a 2D camera's.
    pub post: PostSettings,
}

impl Look3D {
    /// No curve and a neutral grade.
    pub const NEUTRAL: Self = Self {
        tonemap: Tonemap::None,
        post: PostSettings::NEUTRAL,
    };
}

impl Default for Look3D {
    /// The default curve, [`Tonemap::AgX`], and a neutral grade.
    fn default() -> Self {
        Self {
            tonemap: Tonemap::default(),
            post: PostSettings::NEUTRAL,
        }
    }
}
