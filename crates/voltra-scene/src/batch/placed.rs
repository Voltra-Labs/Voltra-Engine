//! One quad through its matrix: the instance that draws it, culled
//! against the view.

use voltra_render::glam::{Mat3, Vec2};
use voltra_render::{QuadInstance, Vertex};

use crate::sprite::sheets::Sheets;

/// The four vertices of `corners` through `matrix` — a quad no instance
/// can draw, being no parallelogram, drawn as two triangles instead — or
/// `None` when they lie wholly outside `visible`.
pub(crate) fn placed_corners(
    visible: Option<(Vec2, Vec2)>,
    matrix: Mat3,
    corners: [Vec2; 4],
    uvs: [[f32; 2]; 4],
    color: [f32; 4],
) -> Option<[Vertex; 4]> {
    let world = corners.map(|corner| matrix.transform_point2(corner));
    let min = world.iter().copied().fold(Vec2::INFINITY, Vec2::min);
    let max = world.iter().copied().fold(Vec2::NEG_INFINITY, Vec2::max);
    if !seen(visible, min, max) {
        return None;
    }
    let mut corner = 0;
    Some(world.map(|at| {
        let vertex = Vertex::new([at.x, at.y], color, uvs[corner]);
        corner += 1;
        vertex
    }))
}

/// `quad`, or `None` when it lies wholly outside `visible`.
///
/// Culled on the corners as they will be drawn, so a rotated, scaled,
/// parented or parallax-displaced quad is tested where it really is.
pub(crate) fn culled(visible: Option<(Vec2, Vec2)>, quad: QuadInstance) -> Option<QuadInstance> {
    let origin = Vec2::from(quad.origin);
    let across = Vec2::from(quad.across);
    let down = Vec2::from(quad.down);
    let min = origin + across.min(Vec2::ZERO) + down.min(Vec2::ZERO);
    let max = origin + across.max(Vec2::ZERO) + down.max(Vec2::ZERO);
    // `min` and `max` would hide a NaN corner, which `sees` keeps.
    let finite = origin.is_finite() && across.is_finite() && down.is_finite();
    (!finite || seen(visible, min, max)).then_some(quad)
}

/// The instance drawing `corners` (sampling `uvs`) through `matrix`, or
/// `None` when either is no parallelogram. Corner 0 and the two edges
/// leaving it go through the matrix; corner 2 is where they put it.
///
/// One function for every quad the batch holds, so a tilemap chunk kept
/// from last frame and the same tiles pushed afresh are the same bits.
pub(crate) fn instance(
    matrix: Mat3,
    corners: [Vec2; 4],
    uvs: [[f32; 2]; 4],
    color: [f32; 4],
) -> Option<QuadInstance> {
    let [c0, c1, c2, c3] = corners;
    let [u0, u1, u2, u3] = uvs.map(Vec2::from);
    (parallelogram(c0, c1, c2, c3) && parallelogram(u0, u1, u2, u3))
        .then(|| unchecked(matrix, corners, uvs, color))
}

/// [`instance`] for corners and UVs already known to be parallelograms: a
/// sprite's quad and a tile's, rectangles sampling rectangles by
/// construction, which need not be asked.
pub(crate) fn unchecked(
    matrix: Mat3,
    corners: [Vec2; 4],
    uvs: [[f32; 2]; 4],
    color: [f32; 4],
) -> QuadInstance {
    let [c0, c1, _, c3] = corners;
    let [u0, u1, _, u3] = uvs.map(Vec2::from);
    QuadInstance {
        origin: matrix.transform_point2(c0).into(),
        across: matrix.transform_vector2(c3 - c0).into(),
        down: matrix.transform_vector2(c1 - c0).into(),
        uv_origin: uvs[0],
        uv_across: (u3 - u0).into(),
        uv_down: (u1 - u0).into(),
        color,
    }
}

/// Whether `c2` is where the other three put it, to within a few ulps of
/// the largest coordinate. Checked before the matrix, which keeps a
/// parallelogram one.
fn parallelogram(c0: Vec2, c1: Vec2, c2: Vec2, c3: Vec2) -> bool {
    let scale = c0
        .abs()
        .max(c1.abs())
        .max(c2.abs())
        .max(c3.abs())
        .max_element()
        .max(1.0);
    (c0 + c2 - c1 - c3).abs().max_element() <= scale * 1e-5
}

fn seen(visible: Option<(Vec2, Vec2)>, min: Vec2, max: Vec2) -> bool {
    Sheets {
        visible,
        ..Sheets::default()
    }
    .sees(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    const UVS: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

    fn square() -> [Vec2; 4] {
        [
            Vec2::new(-0.5, 0.5),
            Vec2::new(-0.5, -0.5),
            Vec2::new(0.5, -0.5),
            Vec2::new(0.5, 0.5),
        ]
    }

    #[test]
    fn a_turned_quad_is_an_instance_with_its_corners_where_the_matrix_puts_them() {
        let matrix = Mat3::from_scale_angle_translation(Vec2::new(2.0, 1.0), 0.7, Vec2::X);
        let quad = instance(matrix, square(), UVS, [1.0; 4]).expect("a turned square is one");
        for (vertex, corner) in quad.corners().iter().zip(square()) {
            let expected = matrix.transform_point2(corner);
            assert!((Vec2::from(vertex.position) - expected).length() < 1e-5);
        }
    }

    #[test]
    fn a_quad_out_of_view_is_culled_and_one_touching_it_is_not() {
        let away = Some((Vec2::splat(10.0), Vec2::splat(20.0)));
        let at = |matrix| instance(matrix, square(), UVS, [1.0; 4]).expect("a square");
        assert!(culled(away, at(Mat3::IDENTITY)).is_none());
        let touching = Mat3::from_translation(Vec2::splat(9.5));
        assert!(culled(away, at(touching)).is_some());
    }

    #[test]
    fn corners_no_instance_can_draw_are_placed_as_vertices() {
        let mut kite = square();
        kite[2] = Vec2::new(2.0, -2.0);
        assert!(instance(Mat3::IDENTITY, kite, UVS, [1.0; 4]).is_none());
        let vertices = placed_corners(None, Mat3::IDENTITY, kite, UVS, [1.0; 4]).expect("in view");
        assert_eq!(vertices[2].position, [2.0, -2.0]);
    }
}
