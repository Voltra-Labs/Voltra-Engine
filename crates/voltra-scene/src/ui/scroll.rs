//! A scrolling view: children moved by an offset and cut to its rect.

use voltra_render::glam::Vec2;

use super::rect::Rect;

/// Shows its UI descendants shifted by `offset` and masked to its own
/// rect: what does not fit is scrolled to, not drawn outside.
///
/// Godot's `ScrollContainer` with `clip_contents`, Unity's `ScrollRect`
/// with a `RectMask2D`. The mask is a rectangle, so it is applied on the
/// CPU — every UI quad is axis-aligned, and one cut to the mask with its
/// UVs moved in step is the same picture a scissor test would give,
/// without breaking the batch into one draw per mask. A masked-off part is
/// not picked either.
///
/// The wheel over it scrolls it `step` points a line; the focus moving to
/// a control inside it scrolls just far enough to show that control, which
/// is how a keyboard or a pad scrolls a list. The offset stays between none
/// and as far as the content reaches past the view — the bottom of its
/// lowest descendant, plus a stack's padding when it stacks them too.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiScroll {
    /// How far the content is scrolled, in points: positive moves it up
    /// and left.
    #[serde(default)]
    pub offset: Vec2,
    #[serde(default = "yes")]
    /// Whether it scrolls up and down.
    pub vertical: bool,
    #[serde(default)]
    /// Whether it scrolls left and right.
    pub horizontal: bool,
    /// Points scrolled per wheel line.
    #[serde(default = "line")]
    pub step: f32,
}

fn yes() -> bool {
    true
}

fn line() -> f32 {
    40.0
}

impl Default for UiScroll {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            vertical: true,
            horizontal: false,
            step: line(),
        }
    }
}

impl UiScroll {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "UiScroll";

    /// The offset the layout moves the content by: none on an axis that does
    /// not scroll, and none for a value that is not a number.
    pub fn shift(&self) -> Vec2 {
        let axis = |on: bool, v: f32| if on && v.is_finite() { v } else { 0.0 };
        Vec2::new(
            axis(self.horizontal, self.offset.x),
            axis(self.vertical, self.offset.y),
        )
    }

    /// Scrolls by `lines` of the wheel: positive is up, so the content
    /// moves down and the offset shrinks.
    pub fn wheel(&mut self, lines: Vec2) {
        let step = if self.step.is_finite() {
            self.step
        } else {
            line()
        };
        self.offset -= lines * step;
    }

    /// Keeps the offset between none and `reach`, the most the content
    /// extends past the view; an axis that does not scroll goes back to
    /// none.
    pub fn clamp(&mut self, reach: Vec2) {
        let shift = self.shift();
        self.offset = shift.clamp(Vec2::ZERO, reach.max(Vec2::ZERO));
    }

    /// The offset that brings `target` into `view` with the least movement,
    /// both in points as laid out with the current offset.
    pub fn reveal(&mut self, view: Rect, target: Rect) {
        let mut by = Vec2::ZERO;
        for axis in 0..2 {
            if target.min[axis] < view.min[axis] {
                by[axis] = target.min[axis] - view.min[axis];
            } else if target.max[axis] > view.max[axis] {
                // A target taller than the view shows its start.
                by[axis] =
                    (target.max[axis] - view.max[axis]).min(target.min[axis] - view.min[axis]);
            }
        }
        self.offset += by;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> Rect {
        Rect::new(Vec2::ZERO, Vec2::new(100.0, 100.0))
    }

    #[test]
    fn the_wheel_moves_by_steps_and_the_clamp_keeps_it_in_reach() {
        let mut scroll = UiScroll::default();
        scroll.wheel(Vec2::new(0.0, -2.0));
        assert_eq!(scroll.offset.y, 80.0);
        scroll.clamp(Vec2::new(0.0, 50.0));
        assert_eq!(scroll.offset.y, 50.0);
        scroll.wheel(Vec2::new(0.0, 5.0));
        scroll.clamp(Vec2::new(0.0, 50.0));
        assert_eq!(scroll.offset.y, 0.0);
    }

    #[test]
    fn an_axis_that_does_not_scroll_stays_put() {
        let mut scroll = UiScroll {
            offset: Vec2::new(30.0, 0.0),
            ..UiScroll::default()
        };
        assert_eq!(scroll.shift(), Vec2::ZERO);
        scroll.clamp(Vec2::splat(100.0));
        assert_eq!(scroll.offset, Vec2::ZERO);
    }

    #[test]
    fn revealing_scrolls_the_least_that_shows_it() {
        let mut scroll = UiScroll::default();
        scroll.reveal(
            view(),
            Rect::new(Vec2::new(0.0, 120.0), Vec2::new(10.0, 140.0)),
        );
        assert_eq!(scroll.offset.y, 40.0);
        scroll.reveal(
            view(),
            Rect::new(Vec2::new(0.0, -30.0), Vec2::new(10.0, -10.0)),
        );
        assert_eq!(scroll.offset.y, 10.0);
        let before = scroll.offset;
        scroll.reveal(view(), Rect::new(Vec2::splat(10.0), Vec2::splat(20.0)));
        assert_eq!(scroll.offset, before, "already in view");
    }

    #[test]
    fn a_nan_offset_is_no_offset() {
        let mut scroll = UiScroll {
            offset: Vec2::new(0.0, f32::NAN),
            ..UiScroll::default()
        };
        assert_eq!(scroll.shift(), Vec2::ZERO);
        scroll.clamp(Vec2::splat(10.0));
        assert_eq!(scroll.offset, Vec2::ZERO);
    }
}
