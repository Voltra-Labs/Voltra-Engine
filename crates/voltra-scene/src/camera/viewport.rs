//! The part of its target a camera draws into.

/// A rectangle of the target, in fractions of it from its top-left: `x`
/// and `y` where it starts, `width` and `height` how much of it it covers.
/// Unity's `Camera.rect`, with y down as the screen's is.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self::FULL
    }
}

impl Viewport {
    /// All of the target.
    pub const FULL: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    };

    pub fn is_full(&self) -> bool {
        *self == Self::FULL
    }

    /// The left or right half, top or bottom, for two-player split screen.
    pub const LEFT: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.5,
        height: 1.0,
    };
    pub const RIGHT: Self = Self {
        x: 0.5,
        y: 0.0,
        width: 0.5,
        height: 1.0,
    };

    /// Kept inside the target: each edge clamped to `0..=1`, never inside
    /// out, a number that is not one taken as the full extent.
    pub fn clamped(&self) -> Self {
        let clean = |v: f32, or: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { or };
        let x = clean(self.x, 0.0);
        let y = clean(self.y, 0.0);
        let right = clean(self.x + self.width, 1.0).max(x);
        let bottom = clean(self.y + self.height, 1.0).max(y);
        Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }

    /// Where it falls on a target `width` × `height` pixels: `[x, y, w, h]`
    /// in whole pixels from the top-left, at least one pixel each way.
    pub fn pixels(&self, width: u32, height: u32) -> [u32; 4] {
        let v = self.clamped();
        let (w, h) = (width as f32, height as f32);
        let x0 = (v.x * w).round() as u32;
        let y0 = (v.y * h).round() as u32;
        let x1 = ((v.x + v.width) * w).round() as u32;
        let y1 = ((v.y + v.height) * h).round() as u32;
        let x0 = x0.min(width.saturating_sub(1));
        let y0 = y0.min(height.saturating_sub(1));
        [x0, y0, x1.max(x0 + 1) - x0, y1.max(y0 + 1) - y0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_split_a_target_without_a_gap() {
        let left = Viewport::LEFT.pixels(1280, 720);
        let right = Viewport::RIGHT.pixels(1280, 720);
        assert_eq!(left, [0, 0, 640, 720]);
        assert_eq!(right, [640, 0, 640, 720]);
    }

    #[test]
    fn a_viewport_past_the_edges_is_kept_inside() {
        let odd = Viewport {
            x: -0.5,
            y: 0.9,
            width: 2.0,
            height: f32::NAN,
        };
        let v = odd.clamped();
        assert_eq!((v.x, v.width), (0.0, 1.0));
        assert!((v.y - 0.9).abs() < 1e-6 && (v.height - 0.1).abs() < 1e-6);
        let tiny = Viewport {
            width: 0.0,
            ..Viewport::FULL
        };
        assert_eq!(tiny.pixels(100, 100)[2], 1, "never less than a pixel");
    }
}
