//! Every font's rasterized glyphs, packed into one RGBA8 image: one
//! texture, so a line falling back from one font to another — or a rich
//! text switching fonts — still draws from one texture.

use std::collections::HashMap;

use super::shelf::Shelves;

/// Side the atlas starts at, in texels. Room for a few hundred glyphs at the
/// default raster size before the first growth.
pub const INITIAL_SIDE: u32 = 256;

/// Side the atlas never grows past, in texels: the smallest
/// `max_texture_dimension_2d` wgpu guarantees on every backend is 8192, and
/// half of that keeps the atlas at 64 MiB in the worst case.
pub const MAX_SIDE: u32 = 4096;

/// Transparent texels left right of and below every glyph.
///
/// Linear sampling at a glyph's edge reads half a texel past it; without the
/// gap that half texel is the neighbouring glyph's ink.
pub const PADDING: u32 = 1;

/// A glyph at one raster size: which font, which glyph of its face, at how
/// many pixels per em, thickened or not.
///
/// By glyph id, not `char`: shaping turns `ffi` into one ligature glyph and
/// an Arabic letter into one of four joined forms, none of which a
/// character names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GlyphKey {
    /// The font's slot, [`Handle::index`](crate::Handle::index) of its
    /// handle: one atlas holds every font's glyphs.
    pub font: u32,
    /// The glyph's index in the face.
    pub glyph: u16,
    /// Raster size, in pixels per em.
    pub px: u32,
    /// Faux bold: the coverage dilated, for a `[b]` in a family with no
    /// bold file.
    pub embolden: bool,
}

/// Where a rasterized glyph's bitmap sits relative to the pen, in pixels,
/// y up: its bottom-left corner is `(xmin, ymin)` from the pen on the
/// baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlyphMetrics {
    /// Pixels from the pen to the bitmap's left edge.
    pub xmin: i32,
    /// Pixels from the baseline to the bitmap's bottom edge.
    pub ymin: i32,
    /// Bitmap width, in pixels.
    pub width: u32,
    /// Bitmap height, in pixels.
    pub height: u32,
}

/// A region of the atlas, in texels, top-left origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphRect {
    /// Left edge, in texels.
    pub x: u32,
    /// Top edge, in texels.
    pub y: u32,
    /// Width, in texels.
    pub width: u32,
    /// Height, in texels.
    pub height: u32,
}

/// What the atlas knows about one glyph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Glyph {
    /// Where its bitmap sits relative to the pen.
    pub metrics: GlyphMetrics,
    /// `None` for a glyph with no ink (a space) and for one refused as too
    /// large: either way there is nothing to draw, and the advance is still
    /// the layout's to apply.
    pub rect: Option<GlyphRect>,
    /// The coverage bitmap, row-major from the top, kept so a growth can
    /// repack without rasterizing again.
    coverage: Vec<u8>,
}

/// Glyph bitmaps packed into one RGBA8 image: white, with coverage in alpha.
///
/// White RGB so the sprite pipeline's vertex colour tints it; coverage in
/// alpha because `Rgba8UnormSrgb` decodes only RGB through the sRGB curve, so
/// alpha reaches the blend exactly as rasterized.
///
/// CPU-side only. [`Fonts::upload`](crate::Fonts::upload) turns the dirty
/// image into a texture; everything else here is testable headless.
#[derive(Debug, Clone)]
pub struct GlyphAtlas {
    side: u32,
    max_side: u32,
    shelves: Shelves,
    glyphs: HashMap<GlyphKey, Glyph>,
    /// Insertion order, so a repack places glyphs deterministically.
    order: Vec<GlyphKey>,
    pixels: Vec<u8>,
    dirty: bool,
}

impl Default for GlyphAtlas {
    fn default() -> Self {
        Self::new(INITIAL_SIDE, MAX_SIDE)
    }
}

impl GlyphAtlas {
    /// An empty atlas `side` texels square that may grow to `max_side`.
    pub fn new(side: u32, max_side: u32) -> Self {
        let side = side.clamp(1, max_side.max(1));
        Self {
            side,
            max_side: max_side.max(side),
            shelves: Shelves::new(side, side),
            glyphs: HashMap::new(),
            order: Vec::new(),
            pixels: blank(side),
            dirty: false,
        }
    }

    /// The glyph under `key`, if it has been inserted.
    pub fn get(&self, key: GlyphKey) -> Option<&Glyph> {
        self.glyphs.get(&key)
    }

    /// Whether `key` has been inserted.
    pub fn contains(&self, key: GlyphKey) -> bool {
        self.glyphs.contains_key(&key)
    }

    /// Stores a rasterized glyph, growing the atlas if it does not fit.
    ///
    /// `coverage` is `metrics.width × metrics.height` bytes, row-major from
    /// the top — `fontdue`'s bitmap as it comes. A glyph that would not fit
    /// even at the maximum side is logged and stored with no rect, so it is
    /// not rasterized and refused again every frame. Re-inserting a key
    /// already present does nothing.
    pub fn insert(&mut self, key: GlyphKey, metrics: GlyphMetrics, coverage: Vec<u8>) {
        if self.contains(key) {
            return;
        }
        let expected = (metrics.width * metrics.height) as usize;
        let coverage = if coverage.len() == expected {
            coverage
        } else {
            log::warn!(
                "glyph {} at {} px: bitmap is {} bytes, expected {expected}; drawing nothing",
                key.glyph,
                key.px,
                coverage.len()
            );
            Vec::new()
        };
        let inked = !coverage.is_empty() && metrics.width > 0 && metrics.height > 0;

        let rect = if inked {
            self.place(key, metrics)
        } else {
            None
        };
        if let Some(rect) = rect {
            blit(&mut self.pixels, self.side, rect, &coverage);
            self.dirty = true;
        }
        self.glyphs.insert(
            key,
            Glyph {
                metrics,
                rect,
                coverage,
            },
        );
        self.order.push(key);
    }

    /// A rect for a new glyph: into the current sheet if it fits, else into
    /// a larger one with everything repacked, else nowhere.
    fn place(&mut self, key: GlyphKey, metrics: GlyphMetrics) -> Option<GlyphRect> {
        let (w, h) = (metrics.width + PADDING, metrics.height + PADDING);
        if w > self.max_side || h > self.max_side {
            log::warn!(
                "glyph {} at {} px is {}x{}, larger than the {} px atlas limit; not drawn",
                key.glyph,
                key.px,
                metrics.width,
                metrics.height,
                self.max_side
            );
            return None;
        }
        if let Some((x, y)) = self.shelves.insert(w, h) {
            return Some(rect_at(x, y, metrics));
        }

        let mut side = self.side;
        while side < self.max_side {
            side = (side * 2).min(self.max_side);
            if let Some(rect) = self.repack(side, metrics) {
                return Some(rect);
            }
        }
        log::warn!(
            "glyph atlas is full at {} px; glyph {} at {} px not drawn",
            self.max_side,
            key.glyph,
            key.px
        );
        None
    }

    /// Repacks every stored glyph plus one of `metrics` into a `side` sheet,
    /// committing only if all of them fit. Returns the new glyph's rect.
    fn repack(&mut self, side: u32, metrics: GlyphMetrics) -> Option<GlyphRect> {
        let mut shelves = Shelves::new(side, side);
        let mut placed = Vec::with_capacity(self.order.len());
        for key in &self.order {
            let glyph = &self.glyphs[key];
            if glyph.rect.is_none() {
                continue;
            }
            let m = glyph.metrics;
            let (x, y) = shelves.insert(m.width + PADDING, m.height + PADDING)?;
            placed.push((*key, rect_at(x, y, m)));
        }
        let (x, y) = shelves.insert(metrics.width + PADDING, metrics.height + PADDING)?;

        let mut pixels = blank(side);
        for (key, rect) in placed {
            let glyph = self.glyphs.get_mut(&key).expect("placed from `order`");
            blit(&mut pixels, side, rect, &glyph.coverage);
            glyph.rect = Some(rect);
        }
        log::debug!("glyph atlas grew from {} to {side} px", self.side);
        self.side = side;
        self.shelves = shelves;
        self.pixels = pixels;
        self.dirty = true;
        Some(rect_at(x, y, metrics))
    }

    /// Forgets every glyph, keeping the image's size: a reloaded font's
    /// glyph ids may now name other shapes. The next prepare rasterizes
    /// again what is still drawn.
    pub fn clear(&mut self) {
        self.shelves = Shelves::new(self.side, self.side);
        self.glyphs.clear();
        self.order.clear();
        self.pixels = blank(self.side);
        self.dirty = true;
    }

    /// Width and height of the image, in texels. Always square.
    pub fn side(&self) -> u32 {
        self.side
    }

    /// The image, RGBA8, row-major from the top.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Whether the image changed since [`mark_clean`](Self::mark_clean).
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Records that the image has been uploaded, so [`is_dirty`](Self::is_dirty) reads false.
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// How many glyphs are stored, drawable or not.
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Whether no glyph is stored.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// `rect`'s corners as UVs in the order the sprite batch takes corners:
    /// top-left, bottom-left, bottom-right, top-right.
    pub fn uvs(&self, rect: GlyphRect) -> [[f32; 2]; 4] {
        let side = self.side as f32;
        let u0 = rect.x as f32 / side;
        let v0 = rect.y as f32 / side;
        let u1 = (rect.x + rect.width) as f32 / side;
        let v1 = (rect.y + rect.height) as f32 / side;
        [[u0, v0], [u0, v1], [u1, v1], [u1, v0]]
    }
}

fn rect_at(x: u32, y: u32, metrics: GlyphMetrics) -> GlyphRect {
    GlyphRect {
        x,
        y,
        width: metrics.width,
        height: metrics.height,
    }
}

/// A `side` × `side` sheet of transparent white.
fn blank(side: u32) -> Vec<u8> {
    [255, 255, 255, 0].repeat((side * side) as usize)
}

/// Writes `coverage` into the alpha of `rect`.
fn blit(pixels: &mut [u8], side: u32, rect: GlyphRect, coverage: &[u8]) {
    for row in 0..rect.height {
        for column in 0..rect.width {
            let source = (row * rect.width + column) as usize;
            let target = (((rect.y + row) * side + rect.x + column) * 4 + 3) as usize;
            pixels[target] = coverage[source];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(glyph: u16, px: u32) -> GlyphKey {
        GlyphKey {
            font: 0,
            glyph,
            px,
            embolden: false,
        }
    }

    fn square(side: u32) -> (GlyphMetrics, Vec<u8>) {
        let metrics = GlyphMetrics {
            xmin: 0,
            ymin: 0,
            width: side,
            height: side,
        };
        (metrics, vec![200; (side * side) as usize])
    }

    fn alpha(atlas: &GlyphAtlas, x: u32, y: u32) -> u8 {
        atlas.pixels()[((y * atlas.side() + x) * 4 + 3) as usize]
    }

    #[test]
    fn an_inserted_glyph_lands_in_alpha_with_white_colour() {
        let mut atlas = GlyphAtlas::new(16, 64);
        let (metrics, coverage) = square(4);
        atlas.insert(key(1, 8), metrics, coverage);

        let rect = atlas.get(key(1, 8)).and_then(|g| g.rect).expect("placed");
        assert_eq!(alpha(&atlas, rect.x, rect.y), 200);
        let i = ((rect.y * atlas.side() + rect.x) * 4) as usize;
        assert_eq!(&atlas.pixels()[i..i + 3], &[255, 255, 255]);
        assert!(atlas.is_dirty());
    }

    #[test]
    fn neighbours_are_kept_a_texel_apart() {
        let mut atlas = GlyphAtlas::new(16, 64);
        let (metrics, coverage) = square(4);
        atlas.insert(key(1, 8), metrics, coverage.clone());
        atlas.insert(key(2, 8), metrics, coverage);

        let a = atlas.get(key(1, 8)).and_then(|g| g.rect).expect("placed");
        let b = atlas.get(key(2, 8)).and_then(|g| g.rect).expect("placed");
        assert_eq!(b.x, a.x + a.width + PADDING);
        assert_eq!(alpha(&atlas, a.x + a.width, a.y), 0, "the gap is clear");
    }

    #[test]
    fn the_same_glyph_at_two_sizes_is_two_entries() {
        let mut atlas = GlyphAtlas::new(32, 64);
        let (small, small_cov) = square(4);
        let (large, large_cov) = square(8);
        atlas.insert(key(1, 8), small, small_cov);
        atlas.insert(key(1, 16), large, large_cov);

        assert_eq!(atlas.len(), 2);
        assert_eq!(atlas.get(key(1, 8)).map(|g| g.metrics.width), Some(4));
        assert_eq!(atlas.get(key(1, 16)).map(|g| g.metrics.width), Some(8));
    }

    #[test]
    fn a_full_atlas_grows_and_keeps_every_glyph() {
        let mut atlas = GlyphAtlas::new(16, 64);
        let (metrics, coverage) = square(7);
        for glyph in 0..6 {
            atlas.insert(key(glyph, 8), metrics, coverage.clone());
        }

        assert!(atlas.side() > 16, "four 8x8 slots fill a 16 px sheet");
        for glyph in 0..6 {
            let rect = atlas.get(key(glyph, 8)).and_then(|g| g.rect);
            let rect = rect.expect("every glyph survived the repack");
            assert_eq!(alpha(&atlas, rect.x, rect.y), 200, "glyph {glyph}");
        }
    }

    #[test]
    fn a_glyph_larger_than_the_limit_is_refused_but_remembered() {
        let mut atlas = GlyphAtlas::new(16, 32);
        let (metrics, coverage) = square(40);
        atlas.insert(key(1, 99), metrics, coverage);

        let glyph = atlas.get(key(1, 99)).expect("remembered");
        assert_eq!(glyph.rect, None);
        assert_eq!(atlas.side(), 16, "no growth for a glyph that cannot fit");
        assert!(!atlas.is_dirty());
    }

    #[test]
    fn a_full_atlas_at_its_limit_refuses_the_next_glyph_and_keeps_the_rest() {
        let mut atlas = GlyphAtlas::new(16, 16);
        let (metrics, coverage) = square(7);
        for glyph in 0..4 {
            atlas.insert(key(glyph, 8), metrics, coverage.clone());
        }
        atlas.insert(key(9, 8), metrics, coverage);

        assert_eq!(atlas.get(key(9, 8)).and_then(|g| g.rect), None);
        assert!(atlas.get(key(0, 8)).and_then(|g| g.rect).is_some());
    }

    #[test]
    fn a_space_is_stored_without_a_rect_or_a_dirty_image() {
        let mut atlas = GlyphAtlas::new(16, 64);
        atlas.insert(key(3, 8), GlyphMetrics::default(), Vec::new());

        assert!(atlas.contains(key(3, 8)));
        assert_eq!(atlas.get(key(3, 8)).and_then(|g| g.rect), None);
        assert!(!atlas.is_dirty());
    }

    #[test]
    fn a_bitmap_of_the_wrong_length_draws_nothing_rather_than_panicking() {
        let mut atlas = GlyphAtlas::new(16, 64);
        let (metrics, _) = square(4);
        atlas.insert(key(1, 8), metrics, vec![1, 2, 3]);
        assert_eq!(atlas.get(key(1, 8)).and_then(|g| g.rect), None);
    }

    #[test]
    fn uvs_cover_the_rect_in_corner_order() {
        let atlas = GlyphAtlas::new(16, 16);
        let uvs = atlas.uvs(GlyphRect {
            x: 4,
            y: 8,
            width: 4,
            height: 8,
        });
        assert_eq!(uvs, [[0.25, 0.5], [0.25, 1.0], [0.5, 1.0], [0.5, 0.5]]);
    }

    #[test]
    fn mark_clean_clears_the_flag() {
        let mut atlas = GlyphAtlas::new(16, 64);
        let (metrics, coverage) = square(4);
        atlas.insert(key(1, 8), metrics, coverage);
        atlas.mark_clean();
        assert!(!atlas.is_dirty());
    }
}
