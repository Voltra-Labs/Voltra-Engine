//! A filled polygon: terrain, a backdrop shape, a hole's cover.
//!
//! Godot's `Polygon2D`, cut to one outline: points in the entity's space,
//! concave or not, filled by [`triangulate`](crate::triangulate) and drawn
//! through the sprite batch — lit, in sorting order. A texture is stretched
//! over the polygon's bounds (the sprites' sampler clamps, so it does not
//! repeat); its colour multiplies in.

use serde::{Deserialize, Serialize};
use voltra_assets::{AssetPath, Handle, Textures};
use voltra_render::glam::Vec2;
use voltra_render::Texture;

use crate::batch::TriangleMesh;
use crate::triangulate::triangulate;

/// A polygon filled with a colour, or a texture tinted by one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Polygon2D {
    /// The outline's corners, in the entity's space, either way round.
    pub points: Vec<Vec2>,
    /// Linear RGBA, multiplied into the texture.
    pub color: [f32; 4],
    /// The image over the polygon's bounds, by path. `None` is a flat
    /// colour.
    #[serde(default)]
    pub texture: Option<AssetPath>,
    /// What `texture` resolves to in this session's textures. Never written.
    #[serde(skip)]
    pub texture_handle: Option<Handle<Texture>>,
    /// Draw order among sprites. See [`Sprite2D::sort_order`](crate::Sprite2D::sort_order).
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

impl Default for Polygon2D {
    /// A white unit square round the origin.
    fn default() -> Self {
        Self {
            points: vec![
                Vec2::new(-0.5, -0.5),
                Vec2::new(0.5, -0.5),
                Vec2::new(0.5, 0.5),
                Vec2::new(-0.5, 0.5),
            ],
            color: [1.0; 4],
            texture: None,
            texture_handle: None,
            sort_order: 0,
            sorting_layer: crate::sorting::default_layer(),
            light_layers: crate::light::default_light_layers(),
        }
    }
}

impl Polygon2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Polygon2D";

    /// The triangles it is drawn as, in its entity's space; the texture's
    /// top-left at the bounds' top-left. A polygon that crosses itself is
    /// still drawn — as far as it can be cut — and warned about.
    pub fn mesh(&self) -> TriangleMesh {
        let cut = triangulate(&self.points);
        if !cut.clean {
            log::warn!(
                "a Polygon2D of {} points crosses itself; filled as well as it can be",
                self.points.len()
            );
        }
        let mut mesh = TriangleMesh::default();
        if cut.indices.is_empty() {
            return mesh;
        }
        let finite = self.points.iter().copied().filter(|p| p.is_finite());
        let (min, max) = finite.fold((Vec2::INFINITY, Vec2::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p), hi.max(p))
        });
        let span = (max - min).max(Vec2::splat(f32::EPSILON));
        for &p in &self.points {
            let at = (p - min) / span;
            // A point left out of the cut (not finite) is never indexed.
            let p = if p.is_finite() { p } else { Vec2::ZERO };
            mesh.vertex(p, [at.x, 1.0 - at.y], self.color);
        }
        mesh.indices = cut.indices;
        mesh
    }

    /// Sets or clears the texture path and refreshes its handle, as
    /// [`Sprite2D::set_texture`](crate::Sprite2D::set_texture) does.
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
mod tests {
    use super::*;

    #[test]
    fn a_concave_polygon_is_filled_without_its_notch() {
        let l = Polygon2D {
            points: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(2.0, 0.0),
                Vec2::new(2.0, 1.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(1.0, 2.0),
                Vec2::new(0.0, 2.0),
            ],
            ..Polygon2D::default()
        };
        let mesh = l.mesh();
        assert_eq!(mesh.indices.len(), 12);
        let uv = |i: usize| mesh.vertices[i].uv;
        assert_eq!(uv(0), [0.0, 1.0], "the bottom-left samples the bottom-left");
        assert_eq!(uv(5), [0.0, 0.0]);
    }

    #[test]
    fn a_polygon_round_trips() {
        let polygon = Polygon2D::default();
        let text = ron::to_string(&polygon).expect("writes");
        assert_eq!(ron::from_str::<Polygon2D>(&text).expect("reads"), polygon);
    }
}
