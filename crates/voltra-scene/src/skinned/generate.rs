//! A skinned mesh made from a sprite's outline: the traced silhouette
//! (stage 59), cut on a grid so a limb has rows of vertices along it to
//! bend with, each cell's piece of the outline filled by ear clipping.
//!
//! Unity's Sprite Editor makes its mesh the same way — an outline, then
//! interior points, then triangles; a grid is the interior points a 2D limb
//! needs without a constrained Delaunay triangulator, and every vertex on
//! a cell edge is shared by the cells either side, so the mesh has no
//! cracks to open when it bends.

use std::collections::HashMap;

use voltra_render::glam::Vec2;

use super::{SkinMesh, SkinPoint};
use crate::triangulate::{signed_area2, triangulate};

/// The most cells along either side; a finer grid is coarsened to it.
pub const MAX_CELLS: usize = 48;

/// The outline of a whole texture's opaque texels, traced in `0..=1` across
/// it (`u` right, `v` down), in the entity's space: centred on the origin,
/// `size` texels at `pixels_per_unit` — as a sprite of that texture is
/// drawn.
pub fn outline_in_units(outline: &[Vec2], size: Vec2, pixels_per_unit: f32) -> Vec<Vec2> {
    let extent = size / pixels_per_unit.max(f32::EPSILON);
    outline
        .iter()
        .map(|uv| Vec2::new(uv.x - 0.5, 0.5 - uv.y) * extent)
        .collect()
}

/// The texture point under `point` in the entity's space, for a texture of
/// `size` texels at `pixels_per_unit`: the inverse of [`outline_in_units`].
pub fn uv_at(point: Vec2, size: Vec2, pixels_per_unit: f32) -> Vec2 {
    let extent = (size / pixels_per_unit.max(f32::EPSILON)).max(Vec2::splat(f32::EPSILON));
    Vec2::new(point.x / extent.x + 0.5, 0.5 - point.y / extent.y)
}

/// A mesh filling `outline` (a closed loop, either way round), cut every
/// `cell` units, each vertex's texture point from `uv_of`. Empty for an
/// outline with no area.
pub fn from_outline(outline: &[Vec2], cell: f32, uv_of: impl Fn(Vec2) -> Vec2) -> SkinMesh {
    let outline: Vec<Vec2> = outline.iter().copied().filter(|p| p.is_finite()).collect();
    let mut mesh = SkinMesh::default();
    if outline.len() < 3 || signed_area2(&outline).abs() <= f32::EPSILON {
        return mesh;
    }
    let (min, max) = outline
        .iter()
        .fold((Vec2::INFINITY, Vec2::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(*p), hi.max(*p))
        });
    let span = max - min;
    let cell = if cell.is_finite() && cell > 0.0 {
        cell.max(span.max_element() / MAX_CELLS as f32)
    } else {
        span.max_element()
    };
    let columns = ((span.x / cell).ceil() as usize).max(1);
    let rows = ((span.y / cell).ceil() as usize).max(1);
    let mut index: HashMap<(i64, i64), u32> = HashMap::new();
    let mut vertex = |mesh: &mut SkinMesh, p: Vec2| -> u32 {
        let key = ((p.x * 1e4).round() as i64, (p.y * 1e4).round() as i64);
        *index.entry(key).or_insert_with(|| {
            mesh.vertices.push(SkinPoint {
                position: p,
                uv: uv_of(p),
                weights: Vec::new(),
            });
            (mesh.vertices.len() - 1) as u32
        })
    };
    for row in 0..rows {
        for column in 0..columns {
            let lo = min + Vec2::new(column as f32, row as f32) * cell;
            let hi = (lo + Vec2::splat(cell)).min(max);
            let piece = clip_to_box(&outline, lo, hi);
            if piece.len() < 3 || signed_area2(&piece).abs() <= 1e-9 {
                continue;
            }
            let cut = triangulate(&piece);
            for triangle in cut.indices.chunks(3) {
                let [a, b, c] = [0, 1, 2].map(|k| piece[triangle[k] as usize]);
                if (b - a).perp_dot(c - a).abs() <= 1e-9 {
                    continue;
                }
                let corners = [a, b, c].map(|p| vertex(&mut mesh, p));
                mesh.triangles.push(corners);
            }
        }
    }
    mesh
}

/// `polygon` clipped to the box `lo..hi` — Sutherland–Hodgman, one side of
/// the box at a time. A concave polygon may come out joined along the box's
/// edge by a sliver with no area, which the triangulation drops.
fn clip_to_box(polygon: &[Vec2], lo: Vec2, hi: Vec2) -> Vec<Vec2> {
    let sides: [(Vec2, f32); 4] = [
        (Vec2::X, lo.x),
        (Vec2::NEG_X, -hi.x),
        (Vec2::Y, lo.y),
        (Vec2::NEG_Y, -hi.y),
    ];
    let mut out = polygon.to_vec();
    for (normal, offset) in sides {
        if out.is_empty() {
            break;
        }
        let inside = |p: Vec2| p.dot(normal) >= offset;
        let input = std::mem::take(&mut out);
        for i in 0..input.len() {
            let (a, b) = (input[i], input[(i + 1) % input.len()]);
            let (da, db) = (a.dot(normal) - offset, b.dot(normal) - offset);
            if inside(a) {
                out.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                out.push(a + (b - a) * (da / (da - db)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(mesh: &SkinMesh) -> f32 {
        mesh.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| mesh.vertices[i as usize].position);
                (b - a).perp_dot(c - a).abs() * 0.5
            })
            .sum()
    }

    #[test]
    fn a_rectangle_is_cut_into_a_grid_with_shared_vertices() {
        let rect = [
            Vec2::new(-0.25, -1.0),
            Vec2::new(0.25, -1.0),
            Vec2::new(0.25, 1.0),
            Vec2::new(-0.25, 1.0),
        ];
        let mesh = from_outline(&rect, 0.25, |p| p);
        assert!((area(&mesh) - 1.0).abs() < 1e-4);
        // Two columns, eight rows: a lattice of 3 × 9 points, shared.
        assert_eq!(mesh.vertices.len(), 27);
        assert_eq!(mesh.triangles.len(), 2 * 2 * 8);
    }

    #[test]
    fn a_concave_outline_is_covered_and_its_notch_left_out() {
        let l = [
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(2.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ];
        let mesh = from_outline(&l, 0.3, |p| p);
        assert!((area(&mesh) - 3.0).abs() < 1e-3, "{}", area(&mesh));
    }

    #[test]
    fn texels_and_units_go_back_and_forth() {
        let size = Vec2::new(16.0, 64.0);
        let outline = outline_in_units(&[Vec2::ZERO, Vec2::new(1.0, 1.0)], size, 32.0);
        assert_eq!(outline, vec![Vec2::new(-0.25, 1.0), Vec2::new(0.25, -1.0)]);
        assert_eq!(uv_at(outline[1], size, 32.0), Vec2::new(1.0, 1.0));
    }

    #[test]
    fn nothing_to_fill_is_no_mesh() {
        assert!(from_outline(&[Vec2::ZERO, Vec2::X], 0.1, |p| p)
            .triangles
            .is_empty());
        let line = [Vec2::ZERO, Vec2::X, Vec2::new(2.0, 0.0)];
        assert!(from_outline(&line, 0.1, |p| p).triangles.is_empty());
    }
}
