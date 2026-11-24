//! One loaded font: the file's bytes, which the shaper reads, and the face
//! `fontdue` parsed from them, which rasterizes.

use std::fmt;
use std::sync::{Arc, Mutex};

use super::atlas::GlyphMetrics;
use super::embolden;

/// A font file, parsed.
///
/// Two readers of the same bytes: `rustybuzz` shapes a run into glyph ids
/// and positions, and `fontdue` rasterizes those ids. The shaping face
/// borrows the bytes, and is kept beside them in a [`Shaper`]: making one
/// builds every lookup's accelerator, and was most of laying out a short
/// label while it was made per layout (stage 108).
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
    raster: fontdue::Font,
    /// Shared, as the bytes it owns are: a clone of the font is the same
    /// file.
    shaper: Arc<Shaper>,
    /// Shape plans made for this face, by what they were made for: making
    /// one looks up every feature of the script's lookups, which
    /// `rustybuzz::shape` does on every call.
    plans: Arc<Mutex<Plans>>,
}

/// The plans one face keeps.
type Plans = Vec<(PlanKey, Arc<rustybuzz::ShapePlan>)>;

/// What a shape plan is made for: direction, script and language.
type PlanKey = (
    rustybuzz::Direction,
    rustybuzz::Script,
    Option<rustybuzz::Language>,
);

/// Plans kept per face before they start over: a few scripts in each
/// direction is all a game's text uses.
const PLANS: usize = 32;

/// A shaping face, as `rustybuzz` names its borrowing type.
type ShapingFace<'a> = rustybuzz::Face<'a>;

self_cell::self_cell!(
    /// The file's bytes and the shaping face reading them, kept together:
    /// the face borrows the bytes, which a plain struct cannot hold.
    struct Shaper {
        owner: Arc<[u8]>,
        #[covariant]
        dependent: ShapingFace,
    }
);

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
        let shaper = Shaper::try_new(bytes, |bytes| {
            rustybuzz::Face::from_slice(bytes, 0).ok_or("the shaper cannot read its tables")
        })?;
        Ok(Self {
            face: Some(Face {
                raster,
                shaper: Arc::new(shaper),
                plans: Arc::default(),
            }),
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

    /// The shaping face over the file's bytes, made when the font was.
    pub(crate) fn shaper(&self) -> Option<&rustybuzz::Face<'_>> {
        Some(self.face.as_ref()?.shaper.borrow_dependent())
    }

    /// `buffer` shaped by this face, through a plan kept for its
    /// direction, script and language — what `rustybuzz::shape` makes
    /// afresh each call. `None` with no face. The buffer's segment
    /// properties must already be set (`guess_segment_properties`).
    pub(crate) fn shape(&self, buffer: rustybuzz::UnicodeBuffer) -> Option<rustybuzz::GlyphBuffer> {
        let face = self.face.as_ref()?;
        let shaper = face.shaper.borrow_dependent();
        let key: PlanKey = (buffer.direction(), buffer.script(), buffer.language());
        let made = || {
            // An unset script reads back as `UNKNOWN`; the plan is made for
            // none, as `rustybuzz::shape` makes it.
            let script = (key.1 != rustybuzz::script::UNKNOWN).then_some(key.1);
            Arc::new(rustybuzz::ShapePlan::new(
                shaper,
                key.0,
                script,
                key.2.as_ref(),
                &[],
            ))
        };
        let plan = match face.plans.lock() {
            Ok(mut plans) => match plans.iter().find(|(k, _)| *k == key) {
                Some((_, plan)) => plan.clone(),
                None => {
                    if plans.len() >= PLANS {
                        plans.clear();
                    }
                    let plan = made();
                    plans.push((key, plan.clone()));
                    plan
                }
            },
            // A panic while the lock was held: shape without keeping.
            Err(_) => made(),
        };
        Some(rustybuzz::shape_with_plan(shaper, &plan, buffer))
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
