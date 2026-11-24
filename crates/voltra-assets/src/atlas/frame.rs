//! One rectangle of a sheet.

use serde::{Deserialize, Serialize};
use voltra_render::glam::UVec2;

/// One rectangle of a sheet, in texels.
///
/// Texels rather than UVs because texels are what a sheet is authored in: a
/// slicer, an artist and Aseprite's exporter all speak in pixels, and `0.0625`
/// is nobody's idea of a sixteen-pixel cell. The conversion happens once, at
/// batch time, against [`Atlas::sheet`](super::Atlas::sheet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Frame {
    /// Left edge, in texels from the sheet's left.
    pub x: u32,
    /// Top edge, in texels from the sheet's top.
    pub y: u32,
    /// Width, in texels.
    pub width: u32,
    /// Height, in texels.
    pub height: u32,
}

impl Frame {
    /// A frame of `width` by `height` texels with its top-left corner at (`x`, `y`).
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Width and height, in texels.
    pub const fn size(&self) -> UVec2 {
        UVec2::new(self.width, self.height)
    }

    /// Whether the frame covers any texels at all.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The frame as `(min, max)` in UV space, given the sheet it is cut from.
    ///
    /// `None` for an empty frame or an empty sheet: a zero-area quad is not a
    /// picture of anything, and dividing by a zero sheet is how a `NaN` reaches
    /// the vertex buffer.
    pub fn uv(&self, sheet: UVec2) -> Option<([f32; 2], [f32; 2])> {
        if self.is_empty() || sheet.x == 0 || sheet.y == 0 {
            return None;
        }
        let sheet = sheet.as_vec2();
        let min = [self.x as f32 / sheet.x, self.y as f32 / sheet.y];
        let max = [
            (self.x + self.width) as f32 / sheet.x,
            (self.y + self.height) as f32 / sheet.y,
        ];
        Some((min, max))
    }
}
