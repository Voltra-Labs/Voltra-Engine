//! One loaded font: the file's bytes, which the shaper reads, and the face
//! `fontdue` parsed from them, which rasterizes.

use std::fmt;
use std::sync::Arc;

use super::atlas::GlyphMetrics;
use super::embolden;

/// A font file, parsed.
///
/// Two readers of the same bytes: `rustybuzz` shapes a run into glyph ids
/// and positions (its face borrows the bytes, so one is made per layout —
/// cheap, the tables are parsed lazily), and `fontdue` rasterizes those ids.
/// Both index glyphs the way the file does, so an id one hands out the other
/// draws.
///
/// The face is optional: a path that would not parse, in a project whose
/// default font is missing too, still gets a slot — so a later fix to the
/// file can fill it under the handle every `Text` already holds — but it
/// draws nothing until then.
#[derive(Clone)]
pub struct Font {
    face: Option<Face>,
}

/// The parsed file.
#[derive(Clone)]
struct Face {
    bytes: Arc<[u8]>,
    raster: fontdue::Font,
}

/// Baseline-relative heights of a face at one size, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineMetrics {
    /// Baseline to the top of the tallest glyph the face declares.
    pub ascent: f32,
    /// Baseline to the bottom of the lowest glyph the face declares;
    /// negative, below the baseline being down.
    pub descent: f32,
    /// Baseline to baseline, line gap included.
    pub line_height: f32,
}

impl Font {
    /// A font over the file `bytes`, or one that draws nothing when they
    /// are not a font `fontdue` reads.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, &'static str> {
        let bytes: Arc<[u8]> = bytes.into();
        let raster = fontdue::Font::from_bytes(&*bytes, fontdue::FontSettings::default())?;
        if rustybuzz::Face::from_slice(&bytes, 0).is_none() {
            return Err("the shaper cannot read its tables");
        }
        Ok(Self {
            face: Some(Face { bytes, raster }),
        })
    }

    /// A font with no face: draws nothing, lays nothing out.
    pub fn empty() -> Self {
        Self { face: None }
    }

    /// The rasterizing face, or `None` when the file was missing or
    /// unreadable.
    pub fn face(&self) -> Option<&fontdue::Font> {
        self.face.as_ref().map(|face| &face.raster)
    }

    /// Whether it has a face to shape and draw with.
    pub fn is_loaded(&self) -> bool {
        self.face.is_some()
    }

    /// A shaping face over the file's bytes, borrowed for one layout.
    pub(crate) fn shaper(&self) -> Option<rustybuzz::Face<'_>> {
        rustybuzz::Face::from_slice(&self.face.as_ref()?.bytes, 0)
    }

    /// Ascent, descent and line height at `px` pixels per em. A face with
    /// no horizontal metrics is one em per line; no face, nothing.
    pub fn line_metrics(&self, px: u32) -> Option<LineMetrics> {
        let face = self.face()?;
        let size = px as f32;
        Some(match face.horizontal_line_metrics(size) {
            Some(m) => LineMetrics {
                ascent: m.ascent,
                descent: m.descent,
                line_height: m.new_line_size,
            },
            None => LineMetrics {
                ascent: size,
                descent: 0.0,
                line_height: size,
            },
        })
    }

    /// Glyph `glyph`'s coverage at `px`, thickened when `embolden` — the
    /// bold a family with no bold file gets. `None` with no face.
    pub fn rasterize(
        &self,
        glyph: u16,
        px: u32,
        embolden: bool,
    ) -> Option<(GlyphMetrics, Vec<u8>)> {
        let (metrics, coverage) = self.face()?.rasterize_indexed(glyph, px as f32);
        let metrics = GlyphMetrics {
            xmin: metrics.xmin,
            ymin: metrics.ymin,
            width: metrics.width as u32,
            height: metrics.height as u32,
        };
        Some(if embolden {
            embolden::thicken(metrics, &coverage, embolden::strength(px))
        } else {
            (metrics, coverage)
        })
    }
}

impl fmt::Debug for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Font")
            .field("face", &self.face().and_then(|face| face.name()))
            .finish()
    }
}
