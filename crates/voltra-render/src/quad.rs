//! One quad as one instance: the sprite batch's unit of geometry, its four
//! corners made by the vertex shader instead of written by the CPU.
//!
//! A quad drawn through [`Vertex2D`] costs four vertices and six indices,
//! 152 bytes written and uploaded; as a [`QuadInstance`] it costs 64. A
//! sprite is a parallelogram under any 2D affine transform, so a corner,
//! the two edges leaving it and the same three for its texture
//! coordinates say everything the four vertices did. Bevy's sprite batch
//! and Unity's SRP batcher draw instanced quads for the same reason. A
//! quad that is no parallelogram (four corners a caller made up) cannot be
//! an instance; [`QuadInstance::from_corners`] says so, and the batch draws
//! it as two triangles instead.
//!
//! The corners come out of `quad.wgsl`'s `quad_corner`, which
//! [`QuadInstance::corners`] mirrors on the CPU.

use bytemuck::{Pod, Zeroable};

use crate::mesh::Vertex2D;

/// A quad of 2D geometry as one instance of a six-vertex draw.
///
/// Corners are numbered as the batch's quads have always been: 0 top-left,
/// 1 bottom-left, 2 bottom-right, 3 top-right (`y` up, image rows down).
/// Corner 2 is never stored: it is `origin + across + down`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct QuadInstance {
    /// Corner 0, in the space of the camera drawing it.
    pub origin: [f32; 2],
    /// From corner 0 to corner 3.
    pub across: [f32; 2],
    /// From corner 0 to corner 1.
    pub down: [f32; 2],
    /// Corner 0's texture coordinates.
    pub uv_origin: [f32; 2],
    /// From corner 0's texture coordinates to corner 3's.
    pub uv_across: [f32; 2],
    /// From corner 0's texture coordinates to corner 1's.
    pub uv_down: [f32; 2],
    /// Multiplied with the texture, as [`Vertex2D::color`] is; one for the
    /// whole quad.
    pub color: [f32; 4],
}

impl QuadInstance {
    /// Vertices drawn per instance: two triangles over corners 0 1 2 and
    /// 0 2 3, in that order — [`CORNERS`](Self::CORNERS).
    pub const VERTICES: u32 = 6;

    /// The corner each of the six vertices is, as `quad.wgsl` picks them.
    pub const CORNERS: [usize; 6] = [0, 1, 2, 0, 2, 3];

    /// Attribute layout matching `QuadInput` in `quad.wgsl`, stepped once
    /// per instance.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x2,
            3 => Float32x2,
            4 => Float32x2,
            5 => Float32x2,
            6 => Float32x4,
        ],
    };

    /// The instance drawing a quad with these corners (in the order above),
    /// texture coordinates and colour; `None` when the corners or the
    /// texture coordinates are no parallelogram, which an instance cannot
    /// draw.
    pub fn from_corners(
        corners: [[f32; 2]; 4],
        uvs: [[f32; 2]; 4],
        color: [f32; 4],
    ) -> Option<Self> {
        if !parallelogram(corners) || !parallelogram(uvs) {
            return None;
        }
        Some(Self {
            origin: corners[0],
            across: sub(corners[3], corners[0]),
            down: sub(corners[1], corners[0]),
            uv_origin: uvs[0],
            uv_across: sub(uvs[3], uvs[0]),
            uv_down: sub(uvs[1], uvs[0]),
            color,
        })
    }

    /// The four corners as vertices, in corner order, computed as the
    /// vertex shader computes them.
    pub fn corners(&self) -> [Vertex2D; 4] {
        // (along across, along down) for each corner.
        const AT: [(f32, f32); 4] = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)];
        AT.map(|(s, t)| {
            Vertex2D::new(
                along(self.origin, self.across, self.down, s, t),
                self.color,
                along(self.uv_origin, self.uv_across, self.uv_down, s, t),
            )
        })
    }
}

fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn along(origin: [f32; 2], across: [f32; 2], down: [f32; 2], s: f32, t: f32) -> [f32; 2] {
    [
        origin[0] + across[0] * s + down[0] * t,
        origin[1] + across[1] * s + down[1] * t,
    ]
}

/// Whether corner 2 is where the other three put it, to within rounding:
/// a few ulps of the largest coordinate, which an affine transform's
/// arithmetic stays inside and a made-up corner does not.
fn parallelogram(points: [[f32; 2]; 4]) -> bool {
    let scale = points
        .iter()
        .flatten()
        .fold(1.0f32, |largest, v| largest.max(v.abs()));
    let tolerance = scale * 1e-5;
    (0..2).all(|axis| {
        let expected = points[1][axis] + points[3][axis] - points[0][axis];
        (points[2][axis] - expected).abs() <= tolerance
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const UVS: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

    #[test]
    fn the_layout_matches_the_struct() {
        assert_eq!(
            QuadInstance::LAYOUT.array_stride as usize,
            size_of::<QuadInstance>()
        );
        assert_eq!(size_of::<QuadInstance>(), 16 * size_of::<f32>());
        let last = QuadInstance::LAYOUT.attributes.last().expect("seven");
        assert_eq!(last.offset, 12 * size_of::<f32>() as wgpu::BufferAddress);
        assert_eq!(last.format, wgpu::VertexFormat::Float32x4);
    }

    #[test]
    fn a_rotated_quad_comes_back_corner_for_corner() {
        let corners = [[1.0, 3.0], [0.0, 1.0], [2.0, 0.0], [3.0, 2.0]];
        let quad = QuadInstance::from_corners(corners, UVS, [0.5; 4]).expect("a parallelogram");
        let back = quad.corners();
        for (vertex, (corner, uv)) in back.iter().zip(corners.iter().zip(UVS)) {
            assert_eq!(vertex.position, *corner);
            assert_eq!(vertex.uv, uv);
            assert_eq!(vertex.color, [0.5; 4]);
        }
    }

    #[test]
    fn a_turned_tile_keeps_its_texture_coordinates() {
        // A tile turned a quarter: each corner samples the next one's UV.
        let turned = [UVS[1], UVS[2], UVS[3], UVS[0]];
        let corners = [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
        let quad = QuadInstance::from_corners(corners, turned, [1.0; 4]).expect("affine");
        let uvs: Vec<[f32; 2]> = quad.corners().iter().map(|v| v.uv).collect();
        assert_eq!(uvs, turned.to_vec());
    }

    #[test]
    fn four_corners_that_are_no_parallelogram_are_refused() {
        let kite = [[0.0, 1.0], [0.0, 0.0], [2.0, -1.0], [1.0, 1.0]];
        assert!(QuadInstance::from_corners(kite, UVS, [1.0; 4]).is_none());
        let square = [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]];
        let skewed = [[0.0, 0.0], [0.0, 1.0], [0.7, 0.9], [1.0, 0.0]];
        assert!(QuadInstance::from_corners(square, skewed, [1.0; 4]).is_none());
    }

    #[test]
    fn the_corner_table_is_two_triangles_over_every_corner() {
        assert_eq!(QuadInstance::CORNERS.len() as u32, QuadInstance::VERTICES);
        assert_eq!(
            QuadInstance::CORNERS,
            crate::mesh::QUAD_INDICES.map(|i| i as usize)
        );
    }
}
