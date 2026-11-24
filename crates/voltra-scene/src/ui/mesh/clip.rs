//! An axis-aligned quad cut to a mask, its UVs cut with it.

use voltra_assets::Handle;
use voltra_render::glam::{Mat3, Vec2};
use voltra_render::Texture;

use crate::batch::SpriteBatch;
use crate::ui::Rect;

/// Pushes the quad with `corners` — top-left, bottom-left, bottom-right,
/// top-right, in pixels with y down — cut to `mask` when there is one.
///
/// The quad must be axis-aligned, as every screen-space quad is: then each
/// UV varies along one axis only, and the cut moves it in proportion. A
/// quad outside the mask pushes nothing; one with no width or height too.
pub(crate) fn push(
    batch: &mut SpriteBatch,
    corners: [Vec2; 4],
    uvs: [[f32; 2]; 4],
    color: [f32; 4],
    texture: Option<Handle<Texture>>,
    mask: Option<Rect>,
) {
    let (min, max) = (corners[0].min(corners[2]), corners[0].max(corners[2]));
    if !(max.x > min.x && max.y > min.y) {
        return;
    }
    let Some(mask) = mask else {
        batch.push_quad(Mat3::IDENTITY, corners, uvs, color, texture);
        return;
    };
    let cut_min = min.max(mask.min);
    let cut_max = max.min(mask.max);
    if !(cut_max.x > cut_min.x && cut_max.y > cut_min.y) {
        return;
    }
    if cut_min == min && cut_max == max {
        batch.push_quad(Mat3::IDENTITY, corners, uvs, color, texture);
        return;
    }
    // The UV at each edge of the box, from the corners: left/right from the
    // top-left and top-right, top/bottom from the top-left and bottom-left.
    let (left, right) = (corners[0].x, corners[3].x);
    let (top, bottom) = (corners[0].y, corners[1].y);
    let u = |x: f32| lerp(uvs[0][0], uvs[3][0], (x - left) / (right - left));
    let v = |y: f32| lerp(uvs[0][1], uvs[1][1], (y - top) / (bottom - top));
    // Back in the corners' own orientation.
    let (x0, x1) = if left <= right {
        (cut_min.x, cut_max.x)
    } else {
        (cut_max.x, cut_min.x)
    };
    let (y0, y1) = if top <= bottom {
        (cut_min.y, cut_max.y)
    } else {
        (cut_max.y, cut_min.y)
    };
    batch.push_quad(
        Mat3::IDENTITY,
        [
            Vec2::new(x0, y0),
            Vec2::new(x0, y1),
            Vec2::new(x1, y1),
            Vec2::new(x1, y0),
        ],
        [
            [u(x0), v(y0)],
            [u(x0), v(y1)],
            [u(x1), v(y1)],
            [u(x1), v(y0)],
        ],
        color,
        texture,
    );
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    const UVS: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

    fn square() -> [Vec2; 4] {
        [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 10.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(10.0, 0.0),
        ]
    }

    fn pushed(mask: Option<Rect>) -> SpriteBatch {
        let mut batch = SpriteBatch::default();
        push(&mut batch, square(), UVS, [1.0; 4], None, mask);
        batch
    }

    #[test]
    fn an_unmasked_quad_is_pushed_as_it_is() {
        assert_eq!(pushed(None).sprite_count(), 1);
    }

    #[test]
    fn a_quad_outside_the_mask_is_not_pushed() {
        let mask = Rect::new(Vec2::splat(20.0), Vec2::splat(30.0));
        assert_eq!(pushed(Some(mask)).sprite_count(), 0);
    }

    #[test]
    fn a_cut_moves_the_uvs_in_proportion() {
        let mask = Rect::new(Vec2::new(5.0, -5.0), Vec2::new(20.0, 2.5));
        let batch = pushed(Some(mask));
        assert_eq!(batch.sprite_count(), 1);
        let v = &batch.quad_vertices();
        // Top-left corner moved to (5, 0) samples u = 0.5.
        assert_eq!(v[0].position, [5.0, 0.0]);
        assert_eq!(v[0].uv, [0.5, 0.0]);
        // Bottom-right at (10, 2.5) samples v = 0.25.
        assert_eq!(v[2].position, [10.0, 2.5]);
        assert_eq!(v[2].uv, [1.0, 0.25]);
    }
}
