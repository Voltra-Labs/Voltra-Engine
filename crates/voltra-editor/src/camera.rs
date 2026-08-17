//! Scene navigation: how the viewport drives the camera.
//!
//! This lives in the editor, not in `voltra-core`, because how a scene is
//! navigated is a property of the tool. Unity keeps its scene camera in the
//! `UnityEditor` assembly, Unreal in `FEditorViewportClient` and Godot in
//! `CanvasItemEditor`, all for the same reason: a shipped game moves its own
//! camera and must not inherit an editor's bindings.
//!
//! Input scoping is partly egui's job and partly ours. `smooth_scroll_delta`
//! arrives already scoped: whichever `ScrollArea` consumed it zeroes it out,
//! so a scroll over the hierarchy cannot also zoom the scene. Keys get no such
//! help — `keys_down` is populated from the raw key events regardless of which
//! widget holds focus — so `ViewportCamera::navigate` scopes them itself, by
//! gating on `response.hovered()` and on `Context::egui_wants_keyboard_input`,
//! so typing into a focused field cannot also pan the camera.
//!
//! **`WASD` pans only while the right mouse button is held**, and the camera
//! goes home on `F` rather than on `R`. Both were bare keys until the transform
//! tools arrived and `W`, `E` and `R` became Unity's, Unreal's and Godot's
//! binding for move, rotate and scale — a binding worth more than an unmodified
//! pan key, because it is the one every user of another editor already has in
//! their fingers. Unreal and Unity both gate `WASD` behind a held mouse button
//! for exactly this reason, and `F` is what both use to frame the view.

use voltra_core::egui::{Key, PointerButton, Response, Ui};
use voltra_render::glam::Vec2;
use voltra_render::Camera2D;

/// Bindings and tunables for navigating the scene viewport.
pub struct ViewportCamera {
    /// World units per second under keyboard pan, at `zoom == 1.0`.
    pub key_pan_speed: f32,
    /// Zoom multiplier applied per point of scroll.
    ///
    /// Multiplicative, so one notch feels the same at any magnification —
    /// the property Godot's `2^(index/12)` steps have and an additive step
    /// does not.
    pub zoom_per_scroll_point: f32,
    /// Where `F` sends the camera.
    pub home_position: Vec2,
    pub home_zoom: f32,
}

impl Default for ViewportCamera {
    fn default() -> Self {
        Self {
            key_pan_speed: 1.5,
            // A wheel notch is around 50 points, so this is ~1.1 per notch.
            zoom_per_scroll_point: 1.002,
            home_position: Vec2::ZERO,
            home_zoom: 1.0,
        }
    }
}

impl ViewportCamera {
    /// Applies one frame of navigation. `response` is the scene image's.
    pub fn navigate(&self, ui: &Ui, response: &Response, camera: &mut Camera2D) {
        let viewport = Vec2::new(response.rect.width(), response.rect.height());

        if response.dragged_by(PointerButton::Middle) {
            let delta = response.drag_delta();
            self.pan(camera, Vec2::new(delta.x, delta.y), viewport);
        }

        // `hover_pos` is in global screen points; the camera works in
        // viewport-local ones, so the panel's own corner comes off first.
        if let Some(pointer) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let local = pointer - response.rect.min;
                camera.zoom_around(
                    Vec2::new(local.x, local.y),
                    viewport,
                    self.zoom_per_scroll_point.powf(scroll),
                );
            }
        }

        if !response.hovered() || ui.ctx().egui_wants_keyboard_input() {
            return;
        }

        let (dt, axis, reset, panning) = ui.input(|i| {
            (
                // Clamped, so a stalled frame cannot teleport the camera.
                i.stable_dt.min(0.1),
                Vec2::new(
                    axis(i.key_down(Key::D), i.key_down(Key::A)),
                    axis(i.key_down(Key::W), i.key_down(Key::S)),
                ),
                i.key_pressed(Key::F),
                // The held button that turns `WASD` from tool keys into pan
                // keys. Read off the pointer rather than off `response`, which
                // reports a *drag*: a pan that holds the button still and only
                // presses keys never moves the pointer at all.
                i.pointer.button_down(PointerButton::Secondary),
            )
        });

        if panning && axis != Vec2::ZERO {
            // Normalised so diagonal movement is not faster than axis-aligned,
            // and scaled by the visible height so a keypress covers the same
            // fraction of the screen at any zoom.
            let speed = self.key_pan_speed * camera.half_extents().y;
            camera.position += axis.normalize() * speed * dt;
        }

        if reset {
            camera.position = self.home_position;
            camera.set_zoom(self.home_zoom);
            log::info!("camera reset");
        }
    }

    /// Grab-and-drag: the camera travels opposite the pointer.
    fn pan(&self, camera: &mut Camera2D, delta: Vec2, viewport: Vec2) {
        // Anchoring on the world point under the pointer rather than scaling by
        // a hand-derived factor means pan and zoom cannot disagree about what a
        // point is worth.
        let centre = viewport * 0.5;
        let from = camera.viewport_to_world(centre, viewport);
        let to = camera.viewport_to_world(centre - delta, viewport);
        camera.position += to - from;
    }
}

/// `1.0`, `-1.0` or `0.0` from a pair of opposing keys.
fn axis(positive: bool, negative: bool) -> f32 {
    f32::from(positive) - f32::from(negative)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Not square and not a round number, so an axis mix-up cannot pass by
    /// coincidence.
    fn viewport() -> Vec2 {
        Vec2::new(800.0, 600.0)
    }

    #[test]
    fn pan_moves_the_camera_opposite_the_drag_x() {
        let nav = ViewportCamera::default();
        let vp = viewport();
        let mut camera = Camera2D::new(Vec2::ZERO, 1.0, vp.x / vp.y);

        // Dragging the pointer right must move the camera left.
        nav.pan(&mut camera, Vec2::new(10.0, 0.0), vp);
        assert!(
            camera.position.x < 0.0,
            "expected position.x negative after a rightward drag, got {}",
            camera.position.x
        );
    }

    #[test]
    fn pan_moves_the_camera_opposite_the_drag_y() {
        let nav = ViewportCamera::default();
        let vp = viewport();
        let mut camera = Camera2D::new(Vec2::ZERO, 1.0, vp.x / vp.y);

        // The drag delta is Y-down (screen space); world space is Y-up, so
        // dragging the pointer down must move the camera's position.y
        // *positive*.
        nav.pan(&mut camera, Vec2::new(0.0, 10.0), vp);
        assert!(
            camera.position.y > 0.0,
            "expected position.y positive after a downward drag, got {}",
            camera.position.y
        );
    }

    #[test]
    fn pan_covers_half_the_world_distance_at_twice_the_zoom() {
        let nav = ViewportCamera::default();
        let vp = viewport();
        let drag = Vec2::new(20.0, 0.0);

        let mut base = Camera2D::new(Vec2::ZERO, 1.0, vp.x / vp.y);
        nav.pan(&mut base, drag, vp);

        let mut doubled = Camera2D::new(Vec2::ZERO, 2.0, vp.x / vp.y);
        nav.pan(&mut doubled, drag, vp);

        assert!(
            (doubled.position.length() - base.position.length() * 0.5).abs() < 1e-4,
            "expected half the world distance at double zoom: {} vs {}",
            base.position.length(),
            doubled.position.length()
        );
    }

    #[test]
    fn axis_reads_a_key_pair_as_a_signed_direction() {
        assert_eq!(axis(true, false), 1.0);
        assert_eq!(axis(false, true), -1.0);
        assert_eq!(axis(false, false), 0.0);
    }

    #[test]
    fn axis_cancels_when_both_keys_are_held() {
        assert_eq!(axis(true, true), 0.0);
    }
}
