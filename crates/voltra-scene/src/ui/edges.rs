//! Four widths, one per side of a box: a padding, or a 9-slice's borders.

use voltra_render::glam::Vec2;

use super::rect::Rect;

/// A width for each side, top-left origin and y down as every UI box is.
///
/// Godot's `patch_margin_*` and its containers' margins, Unity's
/// `RectOffset` and sprite `border`: the same four numbers serve a stack's
/// padding and a 9-slice's borders, so they are one type.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Edges {
    /// Width on the left, in points.
    pub left: f32,
    /// Width on the top, in points.
    pub top: f32,
    /// Width on the right, in points.
    pub right: f32,
    /// Width on the bottom, in points.
    pub bottom: f32,
}

impl Edges {
    /// The same width on every side.
    pub const fn uniform(width: f32) -> Self {
        Self {
            left: width,
            top: width,
            right: width,
            bottom: width,
        }
    }

    /// The widths before the box: left and top.
    pub fn min(&self) -> Vec2 {
        Vec2::new(self.left, self.top)
    }

    /// The widths after the box: right and bottom.
    pub fn max(&self) -> Vec2 {
        Vec2::new(self.right, self.bottom)
    }

    /// `rect` with these widths taken off its sides. Widths wider than the
    /// box collapse it at its min edge rather than turning it inside out, as
    /// [`UiRect::resolve`](super::UiRect::resolve) does; a negative or
    /// non-finite width counts as none.
    pub fn shrink(&self, rect: Rect) -> Rect {
        let clean = |w: f32| if w.is_finite() { w.max(0.0) } else { 0.0 };
        let min = rect.min + Vec2::new(clean(self.left), clean(self.top));
        let max = rect.max - Vec2::new(clean(self.right), clean(self.bottom));
        Rect::new(min, max.max(min))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_takes_each_side_off() {
        let rect = Rect::new(Vec2::ZERO, Vec2::new(100.0, 50.0));
        let edges = Edges {
            left: 1.0,
            top: 2.0,
            right: 3.0,
            bottom: 4.0,
        };
        assert_eq!(
            edges.shrink(rect),
            Rect::new(Vec2::new(1.0, 2.0), Vec2::new(97.0, 46.0))
        );
    }

    #[test]
    fn edges_wider_than_the_box_collapse_it() {
        let rect = Rect::new(Vec2::ZERO, Vec2::splat(10.0));
        let shrunk = Edges::uniform(8.0).shrink(rect);
        assert_eq!(shrunk.size(), Vec2::ZERO);
        assert_eq!(shrunk.min, Vec2::splat(8.0));
    }

    #[test]
    fn bad_widths_count_as_none() {
        let rect = Rect::new(Vec2::ZERO, Vec2::splat(10.0));
        let edges = Edges {
            left: -5.0,
            top: f32::NAN,
            right: f32::INFINITY,
            bottom: 0.0,
        };
        assert_eq!(edges.shrink(rect), rect);
    }
}
