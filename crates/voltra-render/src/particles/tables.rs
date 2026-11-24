//! What an emitter's particles look up rather than compute: colour, size,
//! speed and alpha over their life, flipbook frames and the outline they are
//! born on — one small float texture per emitter, read by both the
//! simulation and the draw.
//!
//! The CPU emitter evaluates its curves and gradient exactly; the GPU reads
//! [`TABLE_SAMPLES`] samples of each, evenly spaced over a life, and blends
//! the two nearest — close enough that the two paths draw alike, and a
//! texture lookup in place of a loop over keys.

/// Samples per row, and the most flipbook frames and outline points a GPU
/// emitter holds.
pub const TABLE_SAMPLES: usize = 64;

/// Rows in the texture: [`ParticleTables`]' four lists, in order.
pub const TABLE_ROWS: u32 = 4;

/// An emitter's lookup tables. Shorter lists are padded; longer ones cut at
/// [`TABLE_SAMPLES`].
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleTables {
    /// Linear RGBA over the life, from birth (`[0]`) to death (`[last]`).
    pub color: Vec<[f32; 4]>,
    /// Size, speed multiplier and alpha multiplier over the life, the fourth
    /// unused.
    pub curves: Vec<[f32; 4]>,
    /// Flipbook frames as UV rectangles `(min.x, min.y, max.x, max.y)`.
    /// Empty: every particle draws the whole texture.
    pub frames: Vec<[f32; 4]>,
    /// A closed outline particles are born on: each corner's position and
    /// the fraction of the perimeter before it, rising from `0`.
    pub outline: Vec<[f32; 4]>,
}

impl Default for ParticleTables {
    /// White, size one, speed and alpha unchanged, no frames, no outline.
    fn default() -> Self {
        Self {
            color: vec![[1.0; 4]; TABLE_SAMPLES],
            curves: vec![[1.0, 1.0, 1.0, 0.0]; TABLE_SAMPLES],
            frames: Vec::new(),
            outline: Vec::new(),
        }
    }
}

impl ParticleTables {
    /// A size and a colour blended straight from birth to death, speed and
    /// alpha unchanged: what an emitter with no curve and no gradient looks
    /// like.
    pub fn linear(size: (f32, f32), color_start: [f32; 4], color_end: [f32; 4]) -> Self {
        let last = (TABLE_SAMPLES - 1) as f32;
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        Self {
            color: (0..TABLE_SAMPLES)
                .map(|i| {
                    let t = i as f32 / last;
                    std::array::from_fn(|c| lerp(color_start[c], color_end[c], t))
                })
                .collect(),
            curves: (0..TABLE_SAMPLES)
                .map(|i| [lerp(size.0, size.1, i as f32 / last), 1.0, 1.0, 0.0])
                .collect(),
            ..Self::default()
        }
    }

    /// The four rows as the texture holds them, every value finite.
    pub(super) fn texels(&self) -> Vec<[f32; 4]> {
        let mut texels = Vec::with_capacity(TABLE_SAMPLES * TABLE_ROWS as usize);
        for (row, pad) in [
            (&self.color, [1.0; 4]),
            (&self.curves, [1.0, 1.0, 1.0, 0.0]),
            (&self.frames, [0.0, 0.0, 1.0, 1.0]),
            (&self.outline, [0.0; 4]),
        ] {
            // A short colour or curve row holds its last value, as a curve
            // does past its last key.
            let pad = row.last().copied().unwrap_or(pad);
            texels.extend(
                row.iter()
                    .copied()
                    .chain(std::iter::repeat(pad))
                    .take(TABLE_SAMPLES)
                    .map(|texel| texel.map(|v| if v.is_finite() { v } else { 0.0 })),
            );
        }
        texels
    }

    /// How many flipbook frames the GPU draws from.
    pub(super) fn frame_count(&self) -> u32 {
        self.frames.len().min(TABLE_SAMPLES) as u32
    }

    /// How many outline corners the GPU births on.
    pub(super) fn outline_count(&self) -> u32 {
        self.outline.len().min(TABLE_SAMPLES) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_padded_cut_and_cleaned() {
        let tables = ParticleTables {
            color: vec![[0.5; 4], [f32::NAN, 0.0, 0.0, 1.0]],
            frames: vec![[0.0, 0.0, 0.5, 0.5]; 100],
            ..ParticleTables::default()
        };
        let texels = tables.texels();
        assert_eq!(texels.len(), TABLE_SAMPLES * 4);
        assert_eq!(texels[1], [0.0, 0.0, 0.0, 1.0], "NaN is zero");
        assert_eq!(texels[63], texels[1], "a short row holds its end");
        assert_eq!(tables.frame_count(), TABLE_SAMPLES as u32);
        assert_eq!(tables.outline_count(), 0);
    }
}
