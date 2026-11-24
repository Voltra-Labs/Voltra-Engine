//! What a frame carries besides its rectangle: a name, a trim, a pivot.

use serde::{Deserialize, Serialize};
use voltra_render::glam::{UVec2, Vec2};

/// A frame's name, trim and pivot. Every field is optional, and a frame
/// with none of them draws as frames always have: the whole rectangle,
/// centred on the sprite's origin.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FrameMeta {
    /// What the frame is called: the file a packer cut it from, or what the
    /// atlas editor named it. A sprite still names its frame by index; the
    /// name is what keeps that index stable when a sheet is re-packed.
    pub name: Option<String>,
    /// Where the frame sat inside the picture it was trimmed from, if
    /// transparent borders were cut off.
    pub trim: Option<Trim>,
    /// The point of the picture that lands on the sprite's origin, in
    /// `0..1` across its untrimmed size, `x` right and `y` *down* — texel
    /// order, as the frame itself is. `None` is the centre.
    pub pivot: Option<Vec2>,
}

impl FrameMeta {
    /// Metadata naming the frame `name`, with no trim and the centre pivot.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    /// The pivot, the centre when none is set. Clamped to `0..1` and freed
    /// of `NaN`: a file can say anything, and a pivot outside the picture
    /// draws it away from where the sprite is.
    pub fn pivot_or_centre(&self) -> Vec2 {
        match self.pivot {
            Some(pivot) if pivot.is_finite() => pivot.clamp(Vec2::ZERO, Vec2::ONE),
            _ => Vec2::splat(0.5),
        }
    }
}

/// Where a trimmed frame sits inside the picture it was cut from.
///
/// TexturePacker's and Aseprite's `spriteSourceSize` + `sourceSize`: the
/// rectangle on the sheet is only the opaque part, and these two put it back
/// where it was, so a trimmed walk cycle does not jitter as each frame's
/// borders differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trim {
    /// The frame's top-left corner, in texels from the source's top-left.
    pub offset: UVec2,
    /// The untrimmed picture's size, in texels.
    pub source: UVec2,
}

impl Trim {
    /// Whether a frame of `size` texels fits inside the source at its
    /// offset — what a file's trim must satisfy to mean anything.
    pub fn holds(&self, size: UVec2) -> bool {
        self.source.x > 0
            && self.source.y > 0
            && self
                .offset
                .x
                .checked_add(size.x)
                .is_some_and(|x| x <= self.source.x)
            && self
                .offset
                .y
                .checked_add(size.y)
                .is_some_and(|y| y <= self.source.y)
    }
}
