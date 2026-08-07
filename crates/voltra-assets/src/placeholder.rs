//! The texture drawn in place of one that could not be loaded.

/// Width and height of the placeholder, in pixels.
pub const SIZE: u32 = 8;

/// Side of one check, in pixels.
pub const CELL: u32 = 4;

/// Magenta-and-black checks, RGBA, row-major from the top-left.
///
/// Magenta because nothing in real art is this colour by accident, and checks
/// rather than a flat fill because a flat magenta square could pass for a
/// sprite someone deliberately tinted. The alternative already in the tree —
/// binding the 1x1 white texture — makes a broken path look exactly like a
/// sprite with no texture at all, which hides the failure everywhere but the
/// log.
pub fn rgba() -> Vec<u8> {
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let magenta = (x / CELL + y / CELL).is_multiple_of(2);
            pixels.extend_from_slice(if magenta {
                &[255, 0, 255, 255]
            } else {
                &[0, 0, 0, 255]
            });
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAGENTA: [u8; 4] = [255, 0, 255, 255];
    const BLACK: [u8; 4] = [0, 0, 0, 255];

    fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * SIZE + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    }

    #[test]
    fn it_is_the_right_number_of_bytes() {
        assert_eq!(rgba().len(), (SIZE * SIZE * 4) as usize);
    }

    #[test]
    fn it_starts_magenta() {
        assert_eq!(pixel(&rgba(), 0, 0), MAGENTA);
    }

    #[test]
    fn the_next_cell_across_is_black() {
        // A checker, not a flat fill. A flat magenta square could be mistaken
        // for a sprite someone actually tinted magenta.
        let pixels = rgba();
        assert_eq!(pixel(&pixels, CELL, 0), BLACK);
        assert_eq!(pixel(&pixels, 0, CELL), BLACK);
        assert_eq!(pixel(&pixels, CELL, CELL), MAGENTA);
    }

    #[test]
    fn a_cell_is_one_colour_throughout() {
        let pixels = rgba();
        for y in 0..CELL {
            for x in 0..CELL {
                assert_eq!(pixel(&pixels, x, y), MAGENTA, "at {x},{y}");
            }
        }
    }

    #[test]
    fn every_pixel_is_opaque() {
        // A transparent placeholder is an invisible one, which defeats it.
        for chunk in rgba().chunks_exact(4) {
            assert_eq!(chunk[3], 255);
        }
    }

    #[test]
    fn both_colours_appear() {
        let pixels = rgba();
        let magenta = pixels.chunks_exact(4).filter(|c| *c == MAGENTA).count();
        let black = pixels.chunks_exact(4).filter(|c| *c == BLACK).count();
        assert_eq!(magenta, black, "the checker should be evenly split");
    }
}
