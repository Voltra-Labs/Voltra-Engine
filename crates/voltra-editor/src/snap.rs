//! Snapping a transform edit to a grid: moves to whole steps, turns to
//! whole angles, scales to whole increments.

use voltra_render::glam::Vec2;
use voltra_render::{LineBatch, View2D};
use voltra_scene::Transform;

use crate::tool::Tool;

/// The editor's grid and what snaps to it.
///
/// Unity's grid snapping and increment snapping, Godot's snap settings:
/// on, a move lands on the grid — absolute, not by steps from where the
/// drag began, so things line up with each other — a turn on a multiple of
/// `angle` degrees, and a scale on a multiple of `scale`. Ctrl held inverts
/// it for one drag, as both engines do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snap {
    pub enabled: bool,
    /// The grid's spacing, world units.
    pub step: f32,
    /// Degrees a turn snaps to.
    pub angle: f32,
    /// The increment a scale snaps to.
    pub scale: f32,
    /// Whether the grid is drawn over the viewport.
    pub show_grid: bool,
}

impl Default for Snap {
    fn default() -> Self {
        Self {
            enabled: false,
            step: 0.5,
            angle: 15.0,
            scale: 0.25,
            show_grid: true,
        }
    }
}

/// A value rounded to the nearest multiple of `step`, or as it is for a
/// step that is not a positive number.
fn round_to(value: f32, step: f32) -> f32 {
    if step.is_finite() && step > 0.0 {
        (value / step).round() * step
    } else {
        value
    }
}

impl Snap {
    /// `transform`, the result of a `tool` drag, snapped — when snapping is
    /// on, flipped by `invert` (Ctrl held). Only what the tool changes is
    /// touched.
    pub fn apply(&self, tool: Tool, transform: Transform, invert: bool) -> Transform {
        if self.enabled == invert {
            return transform;
        }
        match tool {
            Tool::Translate => Transform {
                translation: Vec2::new(
                    round_to(transform.translation.x, self.step),
                    round_to(transform.translation.y, self.step),
                ),
                ..transform
            },
            Tool::Rotate => Transform {
                rotation: round_to(transform.rotation.to_degrees(), self.angle).to_radians(),
                ..transform
            },
            Tool::Scale => {
                let snapped = |s: f32| {
                    let r = round_to(s, self.scale);
                    // Never snapped to nothing: a zero scale is a way in with
                    // no way out.
                    if r == 0.0 {
                        self.scale.copysign(s)
                    } else {
                        r
                    }
                };
                Transform {
                    scale: Vec2::new(snapped(transform.scale.x), snapped(transform.scale.y)),
                    ..transform
                }
            }
        }
    }

    /// The grid's lines across what `camera` shows — sparser when zoomed
    /// out so it never takes more than about a hundred lines a side.
    pub fn draw_grid(&self, camera: &View2D, lines: &mut LineBatch) {
        if !self.show_grid || !(self.step.is_finite() && self.step > 0.0) {
            return;
        }
        let (min, max) = camera.visible_bounds();
        let mut step = self.step;
        while (max.x - min.x) / step > 100.0 || (max.y - min.y) / step > 100.0 {
            step *= 2.0;
        }
        let color = [1.0, 1.0, 1.0, 0.08];
        let mut x = (min.x / step).floor() * step;
        while x <= max.x {
            lines.push(Vec2::new(x, min.y), Vec2::new(x, max.y), 1.0, color);
            x += step;
        }
        let mut y = (min.y / step).floor() * step;
        while y <= max.y {
            lines.push(Vec2::new(min.x, y), Vec2::new(max.x, y), 1.0, color);
            y += step;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> Snap {
        Snap {
            enabled: true,
            ..Snap::default()
        }
    }

    fn at(x: f32, y: f32) -> Transform {
        Transform::from_translation(Vec2::new(x, y))
    }

    #[test]
    fn a_move_lands_on_the_grid() {
        let snapped = on().apply(Tool::Translate, at(0.7, -1.3), false);
        assert_eq!(snapped.translation, Vec2::new(0.5, -1.5));
    }

    #[test]
    fn off_or_inverted_leaves_it_alone() {
        assert_eq!(
            Snap::default().apply(Tool::Translate, at(0.7, 0.0), false),
            at(0.7, 0.0)
        );
        assert_eq!(
            on().apply(Tool::Translate, at(0.7, 0.0), true),
            at(0.7, 0.0)
        );
        let inverted = Snap::default().apply(Tool::Translate, at(0.7, 0.0), true);
        assert_eq!(
            inverted.translation.x, 0.5,
            "Ctrl snaps when snapping is off"
        );
    }

    #[test]
    fn a_turn_lands_on_whole_angles_and_a_scale_on_increments() {
        let turned = Transform {
            rotation: 0.3,
            ..Transform::default()
        };
        let snapped = on().apply(Tool::Rotate, turned, false);
        assert!((snapped.rotation.to_degrees() - 15.0).abs() < 1e-4);
        let scaled = Transform::default().with_scale(Vec2::new(1.1, 0.05));
        let snapped = on().apply(Tool::Scale, scaled, false);
        assert_eq!(snapped.scale, Vec2::new(1.0, 0.25), "never to nothing");
        assert_eq!(snapped.translation, Vec2::ZERO, "the rest untouched");
    }

    #[test]
    fn the_grid_stays_sparse_however_far_out() {
        let mut lines = LineBatch::default();
        let far = View2D::new(Vec2::ZERO, 0.001, 1.0);
        on().draw_grid(&far, &mut lines);
        assert!(lines.len() <= 2 * 104, "{} lines", lines.len());
    }
}
