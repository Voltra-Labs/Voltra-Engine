//! The transform gizmos: what is drawn over the selection, and what a drag does.
//!
//! Split four ways because the parts fail differently. [`handle`] is
//! screen-space hit testing, [`shape`] is the screen-space picture, [`drag`] is
//! world-space arithmetic, and this file is the frame around them: it reads the
//! pointer, decides whether a press begins a drag, and asks the world for the
//! transform. The first three are pure and unit-tested; this one needs egui and
//! a world, and is verified by driving the editor.
//!
//! One `Gizmo` serves all three tools rather than one type each. A press picks
//! the handle the active tool draws and a [`Drag`] carries that tool with it, so
//! the difference between moving, turning and scaling is which arithmetic runs —
//! not which object owns the pointer. Unity, Unreal and Godot all share one
//! manipulator this way; three would mean three copies of the grab, release and
//! despawned-mid-drag handling below.

pub mod drag;
pub mod handle;
pub mod shape;

use voltra_core::egui::{PointerButton, Response};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::Transform;

use crate::tool::Tool;
use drag::Drag;
use handle::{Axes, Handle, LINE_WIDTH};

/// The gizmo's state between frames.
#[derive(Debug, Default)]
pub struct Gizmo {
    /// `Some` only between a press on a handle and the release that ends it.
    drag: Option<Drag>,
}

/// What one frame of gizmo interaction did.
///
/// A struct rather than the bare `bool` this used to return: the caller needs
/// both whether the pointer was taken *and* which entity a drag is holding, and
/// the second is what tells the history the interaction is still live. The gizmo
/// itself stays free of undo — it reports, the panel records.
#[derive(Debug, Clone, Copy, Default)]
pub struct GizmoOutcome {
    /// Whether the pointer was consumed; the caller must not also select.
    pub consumed: bool,
    /// The entity a drag is holding, on every frame it holds one.
    pub dragging: Option<Entity>,
}

impl Gizmo {
    /// Abandons any grab in progress, leaving the entity where it is.
    ///
    /// For when the world changes under the drag rather than the pointer
    /// ending it: a play-mode Stop despawns and respawns every entity, so the
    /// `Entity` and the anchors a [`Drag`] holds are all stale.
    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }

    /// What a live drag is doing, if one is live.
    ///
    /// The tool of the *drag*, not the editor's: pressing another tool's key
    /// mid-drag changes what the next grab will do, and the history has to keep
    /// calling this one what it started as.
    pub fn active_tool(&self) -> Option<Tool> {
        self.drag.map(|drag| drag.tool)
    }

    /// Applies one frame of interaction, reporting what it did.
    ///
    /// The caller uses [`GizmoOutcome::consumed`] to decide whether the click
    /// was also a selection. A handle drawn on top of a sprite has to win, or
    /// the gizmo becomes unusable exactly when the sprite fills the viewport.
    pub fn update(
        &mut self,
        response: &Response,
        frame: &mut UiFrame<'_>,
        selected: Option<Entity>,
        tool: Tool,
    ) -> GizmoOutcome {
        let viewport = viewport_size(response);
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            // A minimised window. Every conversion below divides by this.
            self.drag = None;
            return GizmoOutcome::default();
        }

        // A release ends the drag wherever it happens, including outside the
        // viewport: the drag belongs to the gizmo, not to the cursor still
        // being over a handle.
        if self.drag.is_some() && !response.is_pointer_button_down_on() {
            self.drag = None;
            return GizmoOutcome {
                consumed: true,
                dragging: None,
            };
        }

        let Some(pointer) = response.interact_pointer_pos().or(response.hover_pos()) else {
            return GizmoOutcome {
                consumed: self.drag.is_some(),
                dragging: self.drag.map(|drag| drag.entity),
            };
        };
        // Global screen points; the camera works in viewport-local ones, so the
        // panel's own corner comes off first.
        let local = Vec2::new(
            pointer.x - response.rect.min.x,
            pointer.y - response.rect.min.y,
        );
        let cursor_world = frame.camera.viewport_to_world(local, viewport);

        if let Some(active) = self.drag.as_mut() {
            let entity = active.entity;
            let updated = active.transform(cursor_world);
            let Some(transform) = frame.world.get_mut::<Transform>(entity) else {
                // Despawned mid-drag, or its Transform removed. Ending the drag
                // is the whole response: there is nothing left to transform.
                self.drag = None;
                return GizmoOutcome {
                    consumed: true,
                    dragging: None,
                };
            };
            *transform = updated;
            return GizmoOutcome {
                consumed: true,
                dragging: Some(entity),
            };
        }

        if !response.drag_started_by(PointerButton::Primary) {
            return GizmoOutcome::default();
        }

        let Some(entity) = selected else {
            return GizmoOutcome::default();
        };
        let Some(transform) = frame.world.get::<Transform>(entity) else {
            return GizmoOutcome::default();
        };
        let start = *transform;
        let origin = frame.camera.world_to_viewport(start.translation, viewport);
        let Some(handle) = grab_handle(tool, local, origin, &start) else {
            return GizmoOutcome::default();
        };

        self.drag = Some(Drag::begin(entity, tool, handle, cursor_world, start));
        GizmoOutcome {
            consumed: true,
            dragging: Some(entity),
        }
    }

    /// Pushes this frame's segments for the selection, if there is one.
    pub fn draw(
        &self,
        response: &Response,
        frame: &mut UiFrame<'_>,
        selected: Option<Entity>,
        tool: Tool,
    ) {
        let viewport = viewport_size(response);
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            return;
        }
        let Some(entity) = selected else {
            return;
        };
        let Some(transform) = frame.world.get::<Transform>(entity).copied() else {
            return;
        };

        // A drag keeps drawing the gizmo it began with, so switching tools
        // mid-drag cannot leave the picture disagreeing with the arithmetic.
        let drawn = self.active_tool().unwrap_or(tool);

        // Laid out before `lines()` is called: that borrows the frame mutably,
        // and the layout needs the camera off the same frame.
        let segments = shape::segments(drawn, frame.camera, viewport, &transform);

        let lines = frame.lines();
        for (a, b, color) in segments {
            lines.push(a, b, LINE_WIDTH, color);
        }
    }
}

/// The handle `tool` offers at `cursor`, in viewport-local pixels.
fn grab_handle(tool: Tool, cursor: Vec2, origin: Vec2, transform: &Transform) -> Option<Handle> {
    match tool {
        Tool::Translate => Handle::on_arms(cursor, origin, Axes::WORLD),
        Tool::Rotate => Handle::on_ring(cursor, origin),
        Tool::Scale => Handle::on_arms(cursor, origin, Axes::from_rotation(transform.rotation)),
    }
}

fn viewport_size(response: &Response) -> Vec2 {
    Vec2::new(response.rect.width(), response.rect.height())
}

#[cfg(test)]
mod tests {
    use super::*;
    use handle::{AXIS_LENGTH, RING_RADIUS};
    use std::f32::consts::FRAC_PI_2;

    /// The gizmo's origin in these tests, in screen pixels.
    const O: Vec2 = Vec2::new(100.0, 100.0);

    fn transform() -> Transform {
        Transform::from_translation(Vec2::ZERO)
    }

    #[test]
    fn cancelling_ends_the_drag() {
        // What Stop needs: `Drag` holds an `Entity` and world-space anchors, and
        // both address a world that a restore is about to despawn and respawn.
        let mut world = voltra_ecs::World::new();
        let entity = world.spawn();
        let mut gizmo = Gizmo {
            drag: Some(Drag::begin(
                entity,
                Tool::Translate,
                Handle::Both,
                Vec2::ZERO,
                transform(),
            )),
        };

        gizmo.cancel_drag();

        assert!(gizmo.drag.is_none());
        assert_eq!(gizmo.active_tool(), None);
    }

    #[test]
    fn each_tool_grabs_its_own_handles() {
        let on_arm = O + Vec2::new(AXIS_LENGTH * 0.5, 0.0);
        let on_ring = O + Vec2::new(RING_RADIUS, 0.0);
        let t = transform();

        assert_eq!(grab_handle(Tool::Translate, on_arm, O, &t), Some(Handle::X));
        assert_eq!(grab_handle(Tool::Scale, on_arm, O, &t), Some(Handle::X));
        assert_eq!(
            grab_handle(Tool::Rotate, on_ring, O, &t),
            Some(Handle::Ring)
        );
    }

    #[test]
    fn a_tool_does_not_grab_another_tools_handle() {
        // The ring passes through empty space for the arm gizmos, and the arms
        // sit inside the ring's middle, which the rotate tool leaves clickable.
        let on_ring = O + Vec2::new(0.0, RING_RADIUS);
        let on_arm = O + Vec2::new(AXIS_LENGTH * 0.5, 0.0);
        let t = transform();

        assert_eq!(grab_handle(Tool::Translate, on_ring, O, &t), None);
        assert_eq!(grab_handle(Tool::Rotate, on_arm, O, &t), None);
    }

    #[test]
    fn the_scale_tool_grabs_the_arms_where_the_rotation_puts_them() {
        // The translate tool's arms stay on the world axes; the scale tool's
        // follow the entity. A quarter turn is where the two disagree.
        let turned = transform().with_rotation(FRAC_PI_2);
        let up = O + Vec2::new(0.0, -AXIS_LENGTH * 0.5);

        assert_eq!(grab_handle(Tool::Scale, up, O, &turned), Some(Handle::X));
        assert_eq!(
            grab_handle(Tool::Translate, up, O, &turned),
            Some(Handle::Y)
        );
    }

    #[test]
    fn a_live_drag_reports_its_own_tool() {
        let mut world = voltra_ecs::World::new();
        let gizmo = Gizmo {
            drag: Some(Drag::begin(
                world.spawn(),
                Tool::Rotate,
                Handle::Ring,
                Vec2::new(1.0, 0.0),
                transform(),
            )),
        };

        assert_eq!(gizmo.active_tool(), Some(Tool::Rotate));
    }
}
