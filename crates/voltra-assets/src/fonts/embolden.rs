//! Faux bold: a glyph's coverage thickened, for a family with no bold file.
//!
//! FreeType's `FT_Outline_Embolden` and Godot's `embolden` grow the outline;
//! we have the bitmap only, so the ink is dilated instead — each texel the
//! brightest of the `strength` × `strength` box below-left of it. The glyph
//! grows right and up by `strength` pixels, and the layout adds the same to
//! its advance, so emboldened words keep their spacing.

use super::atlas::GlyphMetrics;

/// How many pixels a glyph rasterized at `px` thickens by: one per 24 px
/// of em, at least one — about the stem weight between a regular and a
/// bold cut.
pub fn strength(px: u32) -> u32 {
    ((px as f32 / 24.0).round() as u32).max(1)
}

/// `coverage` (`metrics.width × metrics.height`, rows from the top)
/// dilated by `by` pixels right and up. A glyph with no ink stays inkless.
pub fn thicken(metrics: GlyphMetrics, coverage: &[u8], by: u32) -> (GlyphMetrics, Vec<u8>) {
    let (w, h) = (metrics.width as usize, metrics.height as usize);
    if w == 0 || h == 0 || coverage.len() != w * h || by == 0 {
        return (metrics, coverage.to_vec());
    }
    let r = by as usize;
    let (out_w, out_h) = (w + r, h + r);
    let mut out = vec![0u8; out_w * out_h];
    for y in 0..h {
        for x in 0..w {
            let value = coverage[y * w + x];
            if value == 0 {
                continue;
            }
            // Input row `y` sits at output row `y + r`: the extra rows are
            // on top, so the bottom edge — and `ymin` — stay put.
            for dy in 0..=r {
                let row = y + r - dy;
                for dx in 0..=r {
                    let texel = &mut out[row * out_w + x + dx];
                    *texel = (*texel).max(value);
                }
            }
        }
    }
    let grown = GlyphMetrics {
        width: out_w as u32,
        height: out_h as u32,
        ..metrics
    };
    (grown, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dot_grows_right_and_up_and_keeps_its_bottom() {
        let metrics = GlyphMetrics {
            xmin: 2,
            ymin: -1,
            width: 1,
            height: 1,
        };
        let (grown, out) = thicken(metrics, &[200], 1);
        assert_eq!((grown.width, grown.height), (2, 2));
        assert_eq!((grown.xmin, grown.ymin), (2, -1));
        assert_eq!(out, vec![200, 200, 200, 200]);
    }

    #[test]
    fn an_inkless_glyph_stays_inkless() {
        let metrics = GlyphMetrics::default();
        assert_eq!(thicken(metrics, &[], 2), (metrics, Vec::new()));
    }

    #[test]
    fn strength_grows_with_size() {
        assert_eq!(strength(12), 1);
        assert_eq!(strength(48), 2);
        assert!(strength(96) > strength(48));
    }
}
