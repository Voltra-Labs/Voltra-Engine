//! The pixel work of packing: cutting transparent borders off, and copying a
//! picture onto the sheet with its edges extruded.

use image::RgbaImage;
use voltra_render::glam::UVec2;

/// The smallest rectangle of `image` holding every pixel that is not fully
/// transparent, as `(offset, size)`.
///
/// A picture with nothing in it keeps one pixel — its top-left, transparent —
/// rather than becoming a zero-area frame, which would draw the missing-frame
/// checker where the artist drew an empty cell on purpose.
pub fn opaque_bounds(image: &RgbaImage) -> (UVec2, UVec2) {
    let (width, height) = image.dimensions();
    let mut min = UVec2::new(width, height);
    let mut max = UVec2::ZERO;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] != 0 {
            min = min.min(UVec2::new(x, y));
            max = max.max(UVec2::new(x + 1, y + 1));
        }
    }
    if max.x <= min.x || max.y <= min.y {
        return (UVec2::ZERO, UVec2::ONE.min(UVec2::new(width, height)));
    }
    (min, max - min)
}

/// Copies the `size` rectangle of `source` at `from` onto `sheet` with its
/// top-left at `to`, then repeats its outermost rows and columns `extrude`
/// texels outward, corners included.
///
/// The extrusion is what stops bleeding: a bilinear sample at a frame's edge
/// reads half a texel past it, and what it reads there is now the edge again
/// rather than the neighbour's pixels or transparent black. Padding alone
/// only swaps the neighbour for transparency, which still darkens the edge.
pub fn blit_extruded(
    sheet: &mut RgbaImage,
    source: &RgbaImage,
    from: UVec2,
    size: UVec2,
    to: UVec2,
    extrude: u32,
) {
    let e = extrude as i64;
    for dy in -e..(size.y as i64 + e) {
        for dx in -e..(size.x as i64 + e) {
            let sx = dx.clamp(0, size.x as i64 - 1) as u32 + from.x;
            let sy = dy.clamp(0, size.y as i64 - 1) as u32 + from.y;
            let tx = to.x as i64 + dx;
            let ty = to.y as i64 + dy;
            if tx < 0 || ty < 0 || tx >= sheet.width() as i64 || ty >= sheet.height() as i64 {
                continue;
            }
            sheet.put_pixel(tx as u32, ty as u32, *source.get_pixel(sx, sy));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn with_dot(width: u32, height: u32, at: (u32, u32)) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        image.put_pixel(at.0, at.1, Rgba([255, 0, 0, 255]));
        image
    }

    #[test]
    fn the_bounds_hug_the_opaque_pixels() {
        let mut image = with_dot(10, 8, (2, 3));
        image.put_pixel(6, 4, Rgba([0, 0, 255, 1]));

        assert_eq!(opaque_bounds(&image), (UVec2::new(2, 3), UVec2::new(5, 2)));
    }

    #[test]
    fn an_empty_picture_keeps_one_pixel() {
        assert_eq!(
            opaque_bounds(&RgbaImage::new(4, 4)),
            (UVec2::ZERO, UVec2::ONE)
        );
    }

    #[test]
    fn an_extruded_copy_repeats_its_edges() {
        let mut source = RgbaImage::new(2, 1);
        source.put_pixel(0, 0, Rgba([1, 0, 0, 255]));
        source.put_pixel(1, 0, Rgba([2, 0, 0, 255]));
        let mut sheet = RgbaImage::new(6, 4);

        blit_extruded(
            &mut sheet,
            &source,
            UVec2::ZERO,
            UVec2::new(2, 1),
            UVec2::new(2, 1),
            1,
        );

        let red = |x, y| sheet.get_pixel(x, y)[0];
        assert_eq!((red(1, 1), red(2, 1), red(3, 1), red(4, 1)), (1, 1, 2, 2));
        assert_eq!((red(1, 0), red(4, 2)), (1, 2), "corners too");
        assert_eq!(sheet.get_pixel(0, 1)[3], 0, "and nothing further");
    }
}
