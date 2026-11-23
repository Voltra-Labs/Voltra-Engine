//! Where a screen-space element sits: anchors into its parent, plus offsets.

use voltra_render::glam::Vec2;

/// An axis-aligned box in screen space, in points, origin top-left, y down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// The whole of a screen `size` points across.
    pub fn screen(size: Vec2) -> Self {
        Self::new(Vec2::ZERO, size.max(Vec2::ZERO))
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    /// Edges included. An empty box contains nothing.
    pub fn contains(&self, point: Vec2) -> bool {
        self.max.x > self.min.x
            && self.max.y > self.min.y
            && point.cmpge(self.min).all()
            && point.cmple(self.max).all()
    }
}

/// Places an entity on the screen rather than in the world: Godot's
/// `Control` anchors and offsets, Unity's `RectTransform` without the pivot.
///
/// Anchors are fractions of the parent rect — the nearest ancestor with a
/// `UiRect`, or the screen — from its top-left: `(0, 0)` is the parent's
/// top-left corner, `(1, 1)` its bottom-right. Offsets are points added to
/// the anchored corners. Equal anchors pin a fixed-size box to one point of
/// the parent; spread anchors stretch it with the parent.
///
/// Y runs **down**, unlike the world. Screen-space layout is authored from
/// the top-left in every toolkit, and a HUD's "16 points below the top" is a
/// positive number there.
///
/// An entity with a `UiRect` is drawn by the screen-space pass only: its
/// `Sprite` fills the rect and its `Text` is laid out inside it, and its
/// `Transform`, if any, is ignored.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiRect {
    pub anchor_min: Vec2,
    pub anchor_max: Vec2,
    pub offset_min: Vec2,
    pub offset_max: Vec2,
    /// Where it goes among the children of a [`UiStack`](super::UiStack):
    /// lower first, ties by entity index. Ignored anywhere else; `0`, the
    /// default, is not written.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub slot: i32,
}

fn is_zero(slot: &i32) -> bool {
    *slot == 0
}

impl Default for UiRect {
    /// A 160 × 40 box in the top-left corner, 16 points in.
    fn default() -> Self {
        Self::pinned(Vec2::ZERO, Vec2::splat(16.0), Vec2::new(160.0, 40.0))
    }
}

impl UiRect {
    /// A fixed-size box pinned to `anchor` of its parent, `offset` points
    /// from it, extending away from the anchored edge — so `(1, 0)` with an
    /// offset of `(-16, 16)` is a box whose top-right corner is 16 points in
    /// from the screen's.
    pub fn pinned(anchor: Vec2, offset: Vec2, size: Vec2) -> Self {
        let min = offset - size * anchor;
        Self {
            anchor_min: anchor,
            anchor_max: anchor,
            offset_min: min,
            offset_max: min + size,
            slot: 0,
        }
    }

    /// Fills the parent, `margin` points in from every edge.
    pub fn stretched(margin: f32) -> Self {
        Self {
            anchor_min: Vec2::ZERO,
            anchor_max: Vec2::ONE,
            offset_min: Vec2::splat(margin),
            offset_max: Vec2::splat(-margin),
            slot: 0,
        }
    }

    /// The same box, `slot`th among a stack's children.
    pub fn with_slot(mut self, slot: i32) -> Self {
        self.slot = slot;
        self
    }

    /// The box inside `parent`.
    ///
    /// Offsets that cross — a margin wider than the parent — collapse the
    /// box to zero width or height at its min edge rather than turning it
    /// inside out, so it is never drawn mirrored.
    pub fn resolve(&self, parent: Rect) -> Rect {
        let size = parent.size();
        let min = parent.min + size * self.anchor_min + self.offset_min;
        let max = parent.min + size * self.anchor_max + self.offset_max;
        Rect::new(min, max.max(min))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        min: Vec2::ZERO,
        max: Vec2::new(800.0, 600.0),
    };

    #[test]
    fn a_box_pinned_top_left_sits_at_its_offset() {
        let rect = UiRect::pinned(Vec2::ZERO, Vec2::new(16.0, 8.0), Vec2::new(100.0, 20.0));
        assert_eq!(
            rect.resolve(SCREEN),
            Rect::new(Vec2::new(16.0, 8.0), Vec2::new(116.0, 28.0))
        );
    }

    #[test]
    fn a_box_pinned_bottom_right_grows_inward() {
        let rect = UiRect::pinned(Vec2::ONE, Vec2::splat(-16.0), Vec2::new(100.0, 20.0));
        assert_eq!(
            rect.resolve(SCREEN),
            Rect::new(Vec2::new(684.0, 564.0), Vec2::new(784.0, 584.0))
        );
    }

    #[test]
    fn a_centred_box_is_centred() {
        let rect = UiRect::pinned(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(100.0, 20.0));
        assert_eq!(rect.resolve(SCREEN).center(), Vec2::new(400.0, 300.0));
    }

    #[test]
    fn a_pinned_box_keeps_its_size_when_the_screen_resizes() {
        let rect = UiRect::pinned(Vec2::ONE, Vec2::ZERO, Vec2::new(100.0, 20.0));
        let small = Rect::screen(Vec2::new(320.0, 240.0));
        assert_eq!(rect.resolve(small).size(), Vec2::new(100.0, 20.0));
        assert_eq!(rect.resolve(small).max, Vec2::new(320.0, 240.0));
    }

    #[test]
    fn a_stretched_box_follows_the_screen() {
        let rect = UiRect::stretched(10.0);
        assert_eq!(
            rect.resolve(SCREEN),
            Rect::new(Vec2::splat(10.0), Vec2::new(790.0, 590.0))
        );
    }

    #[test]
    fn crossing_offsets_collapse_rather_than_invert() {
        let rect = UiRect::stretched(500.0);
        let resolved = rect.resolve(SCREEN);
        assert!(resolved.size().cmpge(Vec2::ZERO).all());
        assert!(!resolved.contains(resolved.min));
    }

    #[test]
    fn anchors_are_relative_to_a_parent_rect_not_the_origin() {
        let parent = Rect::new(Vec2::new(100.0, 100.0), Vec2::new(300.0, 200.0));
        let rect = UiRect::pinned(Vec2::ONE, Vec2::ZERO, Vec2::new(10.0, 10.0));
        assert_eq!(rect.resolve(parent).max, Vec2::new(300.0, 200.0));
    }

    #[test]
    fn contains_includes_edges() {
        let rect = Rect::new(Vec2::ZERO, Vec2::splat(10.0));
        assert!(rect.contains(Vec2::ZERO));
        assert!(rect.contains(Vec2::splat(10.0)));
        assert!(!rect.contains(Vec2::new(10.5, 5.0)));
    }
}
