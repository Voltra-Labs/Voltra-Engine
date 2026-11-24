//! A line through points, drawn with a width: a rope, a laser, a road.
//!
//! Godot's `Line2D` and Unity's `LineRenderer` in 2D: points in the
//! entity's space, a width (shaped along the line by a curve), how corners
//! are joined and ends are capped, and a texture stretched along it or
//! tiled. Turned into triangles each frame ([`stroke`]) and drawn through
//! the sprite batch — lit, in sorting order — like everything else.

pub mod stroke;

use serde::{Deserialize, Serialize};
use voltra_assets::{AssetPath, Handle, Textures};
use voltra_render::glam::Vec2;
use voltra_render::Texture;

use crate::curve::Curve;

pub use stroke::{stroke, Stroke};

/// How two segments meet at a corner, on its outer side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineJoint {
    /// The edges carried on until they meet — a sharp point — or cut off
    /// as a bevel where that point would be further out than the limit.
    #[default]
    Miter,
    /// The corner cut straight across.
    Bevel,
    /// The corner rounded.
    Round,
}

/// What an open line ends in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineCap {
    /// Square, at the end point.
    #[default]
    Butt,
    /// Square, half a width past the end point.
    Square,
    /// A half circle round the end point.
    Round,
}

/// How a texture lies along the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LineTexture {
    /// Once from the first point to the last.
    #[default]
    Stretch,
    /// Repeated every width's length along the line: a square tile, the
    /// line's own height.
    Tile,
}

/// A line drawn through its points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line2D {
    /// The points, in the entity's space.
    pub points: Vec<Vec2>,
    /// Whether the last point joins the first.
    #[serde(default)]
    pub closed: bool,
    /// The line's width, in the entity's units.
    pub width: f32,
    /// What the width is multiplied by along the line, `0` at its first
    /// point to `1` at its last. `None` changes nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_curve: Option<Curve>,
    /// Linear RGBA, multiplied into the texture.
    pub color: [f32; 4],
    /// How corners are joined.
    #[serde(default)]
    pub joint: LineJoint,
    /// How far a miter may reach, in half-widths, before it is beveled.
    #[serde(default = "default_miter_limit")]
    pub miter_limit: f32,
    /// What the first point ends in, for an open line.
    #[serde(default)]
    pub begin_cap: LineCap,
    /// What the last point ends in, for an open line.
    #[serde(default)]
    pub end_cap: LineCap,
    /// The image along the line, by path. `None` is a flat colour.
    #[serde(default)]
    pub texture: Option<AssetPath>,
    /// What `texture` resolves to in this session's textures. Never written.
    #[serde(skip)]
    pub texture_handle: Option<Handle<Texture>>,
    /// How the texture lies along the line.
    #[serde(default)]
    pub texture_mode: LineTexture,
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

/// A miter reaches twice as far as a half-width at most: Godot's default
/// sharp limit, and the SVG default's neighbourhood.
fn default_miter_limit() -> f32 {
    2.0
}

impl Default for Line2D {
    /// A white line a tenth of a unit wide, from the origin one unit right
    /// then up.
    fn default() -> Self {
        Self {
            points: vec![Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)],
            closed: false,
            width: 0.1,
            width_curve: None,
            color: [1.0; 4],
            joint: LineJoint::Miter,
            miter_limit: default_miter_limit(),
            begin_cap: LineCap::Butt,
            end_cap: LineCap::Butt,
            texture: None,
            texture_handle: None,
            texture_mode: LineTexture::Stretch,
            sort_order: 0,
            sorting_layer: crate::sorting::default_layer(),
            light_layers: crate::light::default_light_layers(),
        }
    }
}

impl Line2D {
    /// The triangles it is drawn as, in its entity's space.
    pub fn mesh(&self) -> crate::batch::TriangleMesh {
        let width = |t: f32| {
            self.width
                * self
                    .width_curve
                    .as_ref()
                    .map_or(1.0, |curve| curve.sample(t))
        };
        let color = |_: f32| self.color;
        stroke(&Stroke {
            points: &self.points,
            closed: self.closed,
            width: &width,
            color: &color,
            joint: self.joint,
            miter_limit: self.miter_limit,
            caps: (self.begin_cap, self.end_cap),
            texture: self.texture_mode,
            tile: self.width,
        })
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
