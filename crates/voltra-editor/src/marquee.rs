//! The box drawn over the viewport to select what it takes in.
//!
//! A left-button drag that starts on nothing the gizmo wants — Unity's and
//! Godot's rectangle selection. Followed in world units, so a box held while
//! the camera flies keeps its first corner where it was put; what it takes
//! in is [`pick::entities_within`]'s rule. Shift or Ctrl held at the
//! release adds to the selection instead of replacing it.

use voltra_core::egui::{PointerButton, Response};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_render::LineBatch;
use voltra_scene::pick;

use crate::picking::world_at;

/// The box's own blue-white: not a gizmo axis or any overlay's colour.
const COLOR: [f32; 4] = [0.6, 0.8, 1.0, 0.9];

/// Outline thickness, in pixels. Matches the other overlays.
const WIDTH: f32 = 1.0;

/// A box being drawn, from where its drag started.
#[derive(Debug, Default)]
pub(crate) struct Marquee {
    /// The corner the drag started at, in the world; `None` with no box.
    from: Option<Vec2>,
    /// The corner under the pointer now, in the world.
    to: Vec2,
}

/// A box let go of: what it took in, the topmost last, and whether to add
/// it to the selection rather than replace it.
pub(crate) struct Taken {
    pub(crate) entities: Vec<Entity>,
    pub(crate) extend: bool,
}

impl Marquee {
    /// Starts a box on a left drag over `scene` that `consumed` (the gizmo)
    /// did not take, follows it, and on the release answers what it took.
    pub(crate) fn update(
        &mut self,
        scene: &Response,
        frame: &UiFrame<'_>,
        consumed: bool,
    ) -> Option<Taken> {
        let pointer = scene.interact_pointer_pos().or(scene.hover_pos());
        if self.from.is_none() {
            if consumed || !scene.drag_started_by(PointerButton::Primary) {
                return None;
            }
            // From where the press was, not where the drag threshold was
            // crossed a few pixels on.
            let pressed = scene.ctx.input(|i| i.pointer.press_origin()).or(pointer)?;
            let at = world_at(scene, pressed, frame);
            self.from = Some(at);
            self.to = at;
        }
        if let Some(pointer) = pointer {
            self.to = world_at(scene, pointer, frame);
        }
        if scene.is_pointer_button_down_on() {
            return None;
        }
        let from = self.from.take()?;
        let modifiers = scene.ctx.input(|i| i.modifiers);
        Some(Taken {
            entities: pick::entities_within(frame.world, from, self.to, frame.sheets()),
            extend: modifiers.shift || modifiers.command,
        })
    }

    /// Lets go of the box without taking anything — the gizmo or a mode took
    /// the pointer.
    pub(crate) fn cancel(&mut self) {
        self.from = None;
    }

    /// The box being drawn, if any, into `lines`.
    pub(crate) fn draw(&self, lines: &mut LineBatch) {
        let Some(from) = self.from else {
            return;
        };
        let (min, max) = (from.min(self.to), from.max(self.to));
        let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
        for i in 0..4 {
            lines.push(corners[i], corners[(i + 1) % 4], WIDTH, COLOR);
        }
    }
}
