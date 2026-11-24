//! 9-slice drawing: an image whose corners keep their size while its edges
//! and middle stretch to fill the rect.

use voltra_render::glam::{UVec2, Vec2};

use super::edges::Edges;
use super::rect::Rect;

/// Draws this entity's `Sprite` as nine pieces over its [`UiRect`]: the
/// corners at their own size, the edges stretched along one axis, the
/// middle along both — so a panel or a button face of any size keeps a
/// crisp frame.
///
/// Godot's `NinePatchRect` margins, on the element rather than, as Unity's
/// sprite `border`, on the image: a sheet here is a texture and an atlas of
/// frames with no per-image metadata to hang borders on, and one frame
/// drawn with different borders in two places costs nothing this way.
///
/// Borders are in the image's texels — what an artist measures — and drawn
/// at `scale` points per texel. Borders wider than the rect shrink, in
/// proportion, until they meet, as Unity's do; they never overlap.
///
/// [`UiRect`]: super::UiRect
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiSlice {
    /// The fixed border of the image, in texels of the drawn frame.
    pub borders: Edges,
    /// Points per texel the borders are drawn at.
    #[serde(default = "one")]
    pub scale: f32,
    /// Whether the middle piece is drawn: off for a frame around nothing.
    #[serde(default = "yes")]
    pub center: bool,
}

fn one() -> f32 {
    1.0
}

fn yes() -> bool {
    true
}

impl Default for UiSlice {
    fn default() -> Self {
        Self {
            borders: Edges::uniform(4.0),
            scale: 1.0,
            center: true,
        }
    }
}

/// One of the nine pieces: corners and UVs in the batch's order — top-left,
/// bottom-left, bottom-right, top-right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    /// The piece's corners, in points.
    pub corners: [Vec2; 4],
    /// The texture coordinates of each corner.
    pub uvs: [[f32; 2]; 4],
}

/// What is being sliced: the drawn frame's size in texels, the UVs it
/// samples (top-left, bottom-left, bottom-right, top-right, flips already
/// in them), and whether it is mirrored on each axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Image {
    /// The image's size, in texels.
    pub texels: UVec2,
    /// The texture coordinates it samples, flips included.
    pub uvs: [[f32; 2]; 4],
    /// Whether it is mirrored left to right.
    pub flip_x: bool,
    /// Whether it is mirrored top to bottom.
    pub flip_y: bool,
}

impl UiSlice {
    /// The pieces covering `rect` — in pixels, y down — with borders drawn
    /// at `pixels_per_point` times their size in points. Pieces of no area
    /// are left out, and so is the middle when [`center`](Self::center) is
    /// off.
    pub fn pieces(&self, rect: Rect, image: Image, pixels_per_point: f32) -> Vec<Piece> {
        let clean = |w: f32| if w.is_finite() { w.max(0.0) } else { 0.0 };
        // Mirrored, the image's right border is drawn on the left.
        let (left, right) = if image.flip_x {
            (self.borders.right, self.borders.left)
        } else {
            (self.borders.left, self.borders.right)
        };
        let (top, bottom) = if image.flip_y {
            (self.borders.bottom, self.borders.top)
        } else {
            (self.borders.top, self.borders.bottom)
        };
        let texels = image.texels.as_vec2();
        // Borders past the image's own size meet in its middle.
        let (tl, tr) = fit(clean(left), clean(right), texels.x);
        let (tt, tb) = fit(clean(top), clean(bottom), texels.y);

        let scale = clean(self.scale) * clean(pixels_per_point);
        let size = rect.size();
        let (pl, pr) = fit(tl * scale, tr * scale, size.x);
        let (pt, pb) = fit(tt * scale, tb * scale, size.y);
        let xs = [rect.min.x, rect.min.x + pl, rect.max.x - pr, rect.max.x];
        let ys = [rect.min.y, rect.min.y + pt, rect.max.y - pb, rect.max.y];

        let [u0, v0] = image.uvs[0];
        let [u1, v1] = image.uvs[2];
        let along = |from: f32, to: f32, texels: f32, a: f32, b: f32| {
            if texels <= 0.0 {
                return [from, from, to, to];
            }
            let step = (to - from) / texels;
            [from, from + step * a, to - step * b, to]
        };
        let us = along(u0, u1, texels.x, tl, tr);
        let vs = along(v0, v1, texels.y, tt, tb);

        let mut pieces = Vec::with_capacity(9);
        for row in 0..3 {
            for column in 0..3 {
                if row == 1 && column == 1 && !self.center {
                    continue;
                }
                let (x0, x1, y0, y1) = (xs[column], xs[column + 1], ys[row], ys[row + 1]);
                if x1 <= x0 || y1 <= y0 {
                    continue;
                }
                let (a0, a1, b0, b1) = (us[column], us[column + 1], vs[row], vs[row + 1]);
                pieces.push(Piece {
                    corners: [
                        Vec2::new(x0, y0),
                        Vec2::new(x0, y1),
                        Vec2::new(x1, y1),
                        Vec2::new(x1, y0),
                    ],
                    uvs: [[a0, b0], [a0, b1], [a1, b1], [a1, b0]],
                });
            }
        }
        pieces
    }
}

/// Two borders that fit in `room`: as they are, or shrunk in proportion
/// until they meet.
fn fit(a: f32, b: f32, room: f32) -> (f32, f32) {
    let total = a + b;
    if total <= room || total <= 0.0 {
        return (a, b);
    }
    let shrink = room.max(0.0) / total;
    (a * shrink, b * shrink)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHOLE: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

    fn image() -> Image {
        Image {
            texels: UVec2::splat(16),
            uvs: WHOLE,
            flip_x: false,
            flip_y: false,
        }
    }

    fn rect(w: f32, h: f32) -> Rect {
        Rect::new(Vec2::ZERO, Vec2::new(w, h))
    }

    #[test]
    fn nine_pieces_with_corners_at_their_own_size() {
        let pieces = UiSlice::default().pieces(rect(100.0, 40.0), image(), 1.0);
        assert_eq!(pieces.len(), 9);
        let top_left = pieces[0];
        assert_eq!(top_left.corners[2], Vec2::splat(4.0));
        assert_eq!(top_left.uvs[2], [0.25, 0.25]);
        let bottom_right = pieces[8];
        assert_eq!(bottom_right.corners[0], Vec2::new(96.0, 36.0));
        assert_eq!(bottom_right.uvs[0], [0.75, 0.75]);
        let middle = pieces[4];
        assert_eq!(middle.corners[0], Vec2::splat(4.0));
        assert_eq!(middle.corners[2], Vec2::new(96.0, 36.0));
    }

    #[test]
    fn borders_scale_with_the_points_and_the_pixels() {
        let slice = UiSlice {
            scale: 2.0,
            ..UiSlice::default()
        };
        let pieces = slice.pieces(rect(100.0, 40.0), image(), 1.5);
        assert_eq!(pieces[0].corners[2], Vec2::splat(12.0));
    }

    #[test]
    fn borders_wider_than_the_rect_meet_and_leave_no_middle() {
        let pieces = UiSlice::default().pieces(rect(6.0, 40.0), image(), 1.0);
        // Left and right are 3 wide each; nothing is left between them.
        assert_eq!(pieces.len(), 6);
        assert_eq!(pieces[0].corners[2].x, 3.0);
        assert!(pieces.iter().all(|p| p.corners[2].x <= 6.0));
    }

    #[test]
    fn without_a_center_the_middle_is_not_drawn() {
        let slice = UiSlice {
            center: false,
            ..UiSlice::default()
        };
        assert_eq!(slice.pieces(rect(100.0, 40.0), image(), 1.0).len(), 8);
    }

    #[test]
    fn a_mirrored_image_draws_its_right_border_on_the_left() {
        let slice = UiSlice {
            borders: Edges {
                left: 2.0,
                right: 6.0,
                ..Edges::default()
            },
            ..UiSlice::default()
        };
        let flipped = Image {
            uvs: [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]],
            flip_x: true,
            ..image()
        };
        let pieces = slice.pieces(rect(100.0, 40.0), flipped, 1.0);
        // No top or bottom row: three pieces, left to right.
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].corners[2].x, 6.0);
        // It samples the image's right six texels, mirrored.
        assert_eq!(pieces[0].uvs[0][0], 1.0);
        assert_eq!(pieces[0].uvs[2][0], 1.0 - 6.0 / 16.0);
    }

    #[test]
    fn bad_numbers_draw_no_border_rather_than_nan() {
        let slice = UiSlice {
            borders: Edges {
                left: f32::NAN,
                top: -3.0,
                right: f32::INFINITY,
                bottom: 0.0,
            },
            scale: f32::NAN,
            center: true,
        };
        let pieces = slice.pieces(rect(100.0, 40.0), image(), 1.0);
        assert_eq!(pieces.len(), 1);
        assert!(pieces[0]
            .corners
            .iter()
            .all(|c| c.is_finite() && c.cmpge(Vec2::ZERO).all()));
    }
}
