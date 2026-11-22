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
pub mod follow;
pub mod handle;
pub mod shape;

use voltra_core::egui::{PointerButton, Response};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::{hierarchy, Transform};

use crate::selection::Selection;
use crate::tool::Tool;
use drag::Drag;
use handle::{Axes, Handle, LINE_WIDTH};

/// The gizmo's state between frames.
#[derive(Debug, Default)]
pub struct Gizmo {
    /// `Some` only between a press on a handle and the release that ends it.
    drag: Option<Drag>,
    /// The rest of the selection, carried by the drag, with their world
    /// transforms when it began. See [`follow`].
    followers: Vec<(Entity, Transform)>,
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
        self.followers.clear();
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
        selection: &Selection,
        tool: Tool,
    ) -> GizmoOutcome {
        let viewport = viewport_size(response);
        if viewport.x <= 0.0 || viewport.y <= 0.0 {
            // A minimised window. Every conversion below divides by this.
            self.cancel_drag();
            return GizmoOutcome::default();
        }

        // A release ends the drag wherever it happens, including outside the
        // viewport: the drag belongs to the gizmo, not to the cursor still
        // being over a handle.
        if self.drag.is_some() && !response.is_pointer_button_down_on() {
            self.cancel_drag();
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
            // The drag works in world space, because that is where the cursor
            // is; the entity stores a transform local to its parent. This is
            // the one conversion between the two, and it is why the drag
            // arithmetic needs to know nothing about the hierarchy.
            if !hierarchy::set_world_transform(frame.world, entity, &updated) {
                // Despawned mid-drag, its Transform removed, or its parent
                // collapsed to a point. Ending the drag is the whole response:
                // there is nothing left to transform.
                self.cancel_drag();
                return GizmoOutcome {
                    consumed: true,
                    dragging: None,
                };
            }
            let (tool, start) = (active.tool, active.start);
            for &(follower, from) in &self.followers {
                // One that went away mid-drag is simply left behind.
                let to = follow::follow(tool, &start, &updated, &from);
                hierarchy::set_world_transform(frame.world, follower, &to);
            }
            return GizmoOutcome {
                consumed: true,
                dragging: Some(entity),
            };
        }

        if !response.drag_started_by(PointerButton::Primary) {
            return GizmoOutcome::default();
        }

        let Some(entity) = selection.live_primary(frame.world) else {
            return GizmoOutcome::default();
        };
        if frame.world.get::<Transform>(entity).is_none() {
            return GizmoOutcome::default();
        }
        // Where the entity is drawn, not what it stores: the handles are over
        // the picture, and so is the grab that lands on one.
        let start = hierarchy::world_transform(frame.world, entity);
        let origin = frame.camera.world_to_viewport(start.translation, viewport);
        let Some(handle) = grab_handle(tool, local, origin, &start) else {
            return GizmoOutcome::default();
        };

        self.drag = Some(Drag::begin(entity, tool, handle, cursor_world, start));
        self.followers = follow::followers(frame.world, entity, selection.iter());
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
        if frame.world.get::<Transform>(entity).is_none() {
            return;
        }
        let transform = hierarchy::world_transform(frame.world, entity);

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
mod tests;
