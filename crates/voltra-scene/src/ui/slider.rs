//! A value picked along a track, by dragging or by steps.

use voltra_render::glam::Vec2;

use super::rect::{Rect, UiRect};
use super::stack::Direction;

/// A value between `min` and `max`, set by pressing or dragging along the
/// element's rect — the track — or, while focused, by the navigation along
/// it: left and right for a row, down and up for a column. Across it, the
/// navigation moves the focus on, as Godot's `HSlider` does.
///
/// It draws nothing of its own: its `Sprite` is the track, and its UI
/// children marked [`UiSliderPart`] are moved to the value — a fill
/// stretched from the start, a handle placed at it — so either can be a
/// 9-sliced image, a plain colour or absent. A row runs left to right; a
/// column runs bottom to top, as values rise.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiSlider {
    /// The current value, from `min` to `max`.
    pub value: f32,
    #[serde(default)]
    /// The lowest value.
    pub min: f32,
    #[serde(default = "one")]
    /// The highest value.
    pub max: f32,
    /// What the value snaps to, from `min`; `0` for none. The navigation
    /// moves by one step, or by a twentieth of the range without one.
    #[serde(default)]
    pub step: f32,
    #[serde(default = "row")]
    /// Which way it runs.
    pub direction: Direction,
}

fn one() -> f32 {
    1.0
}

fn row() -> Direction {
    Direction::Row
}

/// Which part of a [`UiSlider`] this child of it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UiSliderPart {
    /// Stretched from the start of the track to the value.
    #[default]
    Fill,
    /// Centred on the value: its anchors move there, its offsets — its
    /// size — stay as authored.
    Handle,
}

impl UiSliderPart {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "UiSliderPart";
}

impl Default for UiSlider {
    fn default() -> Self {
        Self {
            value: 0.5,
            min: 0.0,
            max: 1.0,
            step: 0.0,
            direction: Direction::Row,
        }
    }
}

impl UiSlider {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "UiSlider";

    /// The range, low to high, whichever way round it was written; a range
    /// that is not a number is `0..1`.
    fn range(&self) -> (f32, f32) {
        if !(self.min.is_finite() && self.max.is_finite()) {
            return (0.0, 1.0);
        }
        (self.min.min(self.max), self.min.max(self.max))
    }

    /// The value kept in range and on a step: what [`set`](Self::set) and
    /// [`nudge`](Self::nudge) leave, and what a value typed in a scene file
    /// becomes.
    pub fn clean(&self, value: f32) -> f32 {
        let (low, high) = self.range();
        let value = if value.is_finite() { value } else { low };
        let value = if self.step.is_finite() && self.step > 0.0 {
            low + ((value - low) / self.step).round() * self.step
        } else {
            value
        };
        value.clamp(low, high)
    }

    /// How far along the track the value is, `0..=1`.
    pub fn fraction(&self) -> f32 {
        let (low, high) = self.range();
        if high <= low {
            return 0.0;
        }
        ((self.clean(self.value) - low) / (high - low)).clamp(0.0, 1.0)
    }

    /// Sets the value; `true` if it changed.
    pub fn set(&mut self, value: f32) -> bool {
        let value = self.clean(value);
        let changed = value != self.value;
        self.value = value;
        changed
    }

    /// Sets the value from `point` on the `track`; `true` if it changed.
    pub fn set_at(&mut self, track: Rect, point: Vec2) -> bool {
        let size = track.size();
        let t = match self.direction {
            Direction::Row if size.x > 0.0 => (point.x - track.min.x) / size.x,
            Direction::Column if size.y > 0.0 => 1.0 - (point.y - track.min.y) / size.y,
            _ => return false,
        };
        let (low, high) = self.range();
        self.set(low + t.clamp(0.0, 1.0) * (high - low))
    }

    /// Moves the value `steps` steps up or down; `true` if it changed.
    pub fn nudge(&mut self, steps: f32) -> bool {
        let (low, high) = self.range();
        let step = if self.step.is_finite() && self.step > 0.0 {
            self.step
        } else {
            (high - low) / 20.0
        };
        self.set(self.clean(self.value) + steps * step)
    }

    /// How far a navigation `direction` (y down) moves the value: along the
    /// track only, `None` across it.
    pub fn steps_for(&self, direction: Vec2) -> Option<f32> {
        let along = match self.direction {
            Direction::Row => direction.x,
            Direction::Column => -direction.y,
        };
        (along != 0.0).then_some(along.signum())
    }

    /// `rect` moved to show the value, as a `part` of this slider.
    pub fn place(&self, part: UiSliderPart, rect: &mut UiRect) {
        let t = self.fraction();
        match (self.direction, part) {
            (Direction::Row, UiSliderPart::Fill) => {
                rect.anchor_min.x = 0.0;
                rect.anchor_max.x = t;
            }
            (Direction::Row, UiSliderPart::Handle) => {
                rect.anchor_min.x = t;
                rect.anchor_max.x = t;
            }
            (Direction::Column, UiSliderPart::Fill) => {
                rect.anchor_min.y = 1.0 - t;
                rect.anchor_max.y = 1.0;
            }
            (Direction::Column, UiSliderPart::Handle) => {
                rect.anchor_min.y = 1.0 - t;
                rect.anchor_max.y = 1.0 - t;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume() -> UiSlider {
        UiSlider {
            value: 50.0,
            min: 0.0,
            max: 100.0,
            step: 10.0,
            direction: Direction::Row,
        }
    }

    #[test]
    fn a_value_snaps_to_its_step_and_stays_in_range() {
        let mut s = volume();
        assert!(s.set(44.0));
        assert_eq!(s.value, 40.0);
        s.set(250.0);
        assert_eq!(s.value, 100.0);
        s.set(f32::NAN);
        assert_eq!(s.value, 0.0);
    }

    #[test]
    fn a_point_on_the_track_sets_the_value() {
        let mut s = volume();
        let track = Rect::new(Vec2::new(100.0, 0.0), Vec2::new(300.0, 20.0));
        s.set_at(track, Vec2::new(250.0, 10.0));
        assert_eq!(s.value, 80.0, "three quarters along, on a step");
        s.set_at(track, Vec2::new(0.0, 10.0));
        assert_eq!(s.value, 0.0, "before the start is the start");
    }

    #[test]
    fn a_column_rises_from_the_bottom() {
        let mut s = UiSlider {
            direction: Direction::Column,
            ..volume()
        };
        let track = Rect::new(Vec2::ZERO, Vec2::new(20.0, 100.0));
        s.set_at(track, Vec2::new(10.0, 10.0));
        assert_eq!(s.value, 90.0);
        assert_eq!(s.steps_for(Vec2::new(0.0, -1.0)), Some(1.0), "up raises");
        assert_eq!(s.steps_for(Vec2::new(1.0, 0.0)), None, "across moves on");
    }

    #[test]
    fn nudging_moves_a_step_or_a_twentieth() {
        let mut s = volume();
        s.nudge(1.0);
        assert_eq!(s.value, 60.0);
        let mut smooth = UiSlider::default();
        smooth.nudge(-1.0);
        assert!((smooth.value - 0.45).abs() < 1e-6);
    }

    #[test]
    fn the_parts_follow_the_value() {
        let s = UiSlider {
            value: 30.0,
            ..volume()
        };
        let mut fill = UiRect::stretched(0.0);
        s.place(UiSliderPart::Fill, &mut fill);
        assert_eq!((fill.anchor_min.x, fill.anchor_max.x), (0.0, 0.3));
        let mut handle = UiRect::pinned(Vec2::ZERO, Vec2::new(-4.0, 0.0), Vec2::new(8.0, 20.0));
        s.place(UiSliderPart::Handle, &mut handle);
        assert_eq!((handle.anchor_min.x, handle.anchor_max.x), (0.3, 0.3));
        assert_eq!(
            handle.offset_max.x - handle.offset_min.x,
            8.0,
            "its size kept"
        );
    }

    #[test]
    fn a_reversed_or_empty_range_does_not_divide_by_zero() {
        let s = UiSlider {
            min: 5.0,
            max: 5.0,
            value: 5.0,
            ..UiSlider::default()
        };
        assert_eq!(s.fraction(), 0.0);
        let mut reversed = UiSlider {
            min: 10.0,
            max: 0.0,
            ..UiSlider::default()
        };
        reversed.set(4.0);
        assert_eq!(reversed.value, 4.0);
    }
}
