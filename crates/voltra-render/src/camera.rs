//! 2D orthographic camera and its GPU binding.
//!
//! A view is a position, a zoom and a turn in the plane: still 2D. The turn
//! is about the view's own centre, as a `Camera2D`'s rotation is in Godot.

use glam::camera::rh;
use glam::{Mat4, Vec2, Vec3};

/// An orthographic camera looking straight down at the XY plane.
///
/// `zoom` is a scale factor, not a distance: at `zoom == 1.0` the camera shows
/// two world units vertically, matching the clip-space range geometry used
/// before there was a camera at all. Doubling `zoom` halves what fits on
/// screen, which is the direction people expect a zoom control to move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera2D {
    /// World position at the centre of the view.
    pub position: Vec2,
    /// Private because `half_extents` divides by it. Write through
    /// [`Self::set_zoom`], which clamps.
    zoom: f32,
    /// Viewport width divided by height. Keeps squares square.
    pub aspect: f32,
    /// How far the view is turned, counter-clockwise, in radians: the world
    /// on screen turns the other way. `0` — the default — is upright. A
    /// non-finite turn is read as upright, so it cannot poison the matrix.
    pub rotation: f32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            aspect: 1.0,
            rotation: 0.0,
        }
    }
}

impl Camera2D {
    /// A camera at `position`; `zoom` is clamped by [`Self::set_zoom`].
    pub fn new(position: Vec2, zoom: f32, aspect: f32) -> Self {
        let mut camera = Self {
            position,
            zoom: 1.0,
            aspect,
            rotation: 0.0,
        };
        camera.set_zoom(zoom);
        camera
    }

    /// The same view turned `radians` counter-clockwise; see
    /// [`Self::rotation`].
    pub fn with_rotation(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    /// The turn as the maths uses it: [`Self::rotation`], or `0` when that is
    /// not finite.
    pub fn turn(&self) -> f32 {
        if self.rotation.is_finite() {
            self.rotation
        } else {
            0.0
        }
    }

    /// The view's own axes in the world: its right, then its up.
    fn axes(&self) -> (Vec2, Vec2) {
        let (sin, cos) = self.turn().sin_cos();
        (Vec2::new(cos, sin), Vec2::new(-sin, cos))
    }

    /// The four world corners of what the view shows: bottom-left,
    /// bottom-right, top-right, top-left — turned with the view.
    pub fn corners(&self) -> [Vec2; 4] {
        let half = self.half_extents();
        let (right, up) = self.axes();
        let (x, y) = (right * half.x, up * half.y);
        [
            self.position - x - y,
            self.position + x - y,
            self.position + x + y,
            self.position - x + y,
        ]
    }

    /// The world box around what the view shows, `(min, max)`: the view's
    /// rect itself when upright, the box around the turned rect otherwise —
    /// what everything culled against the view is tested by. Conservative:
    /// a turned view's box holds corners the view does not show.
    pub fn visible_bounds(&self) -> (Vec2, Vec2) {
        let half = self.half_extents();
        let (sin, cos) = self.turn().sin_cos();
        let reach = Vec2::new(
            cos.abs() * half.x + sin.abs() * half.y,
            sin.abs() * half.x + cos.abs() * half.y,
        );
        (self.position - reach, self.position + reach)
    }

    /// Current zoom. Always within [`Self::MIN_ZOOM`]..=[`Self::MAX_ZOOM`].
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Half the world-space width and height covered by the viewport, along
    /// the view's own axes (a turned view covers them turned; see
    /// [`Self::visible_bounds`] for the world box).
    pub fn half_extents(&self) -> Vec2 {
        let half_height = 1.0 / self.zoom;
        Vec2::new(half_height * self.aspect, half_height)
    }

    /// Floor and ceiling for [`Self::set_zoom`].
    ///
    /// Unbounded zoom is not a theoretical problem: `half_extents` divides by
    /// `zoom`, so zero produces infinities and a large enough value produces
    /// denormals. Godot clamps its editor zoom for the same reason.
    pub const MIN_ZOOM: f32 = 1.0 / 128.0;
    /// The largest zoom [`Self::set_zoom`] allows.
    pub const MAX_ZOOM: f32 = 128.0;

    /// The world point under a viewport pixel.
    ///
    /// `point` is viewport-local and **Y-down**, which is what every UI toolkit
    /// reports; world space is Y-up, hence the flip on that axis alone.
    ///
    /// Assumes `self.aspect` matches `viewport`'s. `App` keeps it so; if it
    /// ever did not, the image on screen would already be stretched.
    pub fn viewport_to_world(&self, point: Vec2, viewport: Vec2) -> Vec2 {
        let viewport = Self::guard(viewport);
        let half = self.half_extents();
        // Normalised to 0..1 across the viewport, then to -1..1 about its
        // centre, then scaled by what the camera covers.
        let normalised = point / viewport;
        let centred = Vec2::new(normalised.x * 2.0 - 1.0, 1.0 - normalised.y * 2.0) * half;
        let (right, up) = self.axes();
        self.position + right * centred.x + up * centred.y
    }

    /// Inverse of [`Self::viewport_to_world`].
    pub fn world_to_viewport(&self, world: Vec2, viewport: Vec2) -> Vec2 {
        let viewport = Self::guard(viewport);
        let half = self.half_extents();
        let (right, up) = self.axes();
        let offset = world - self.position;
        let centred = Vec2::new(offset.dot(right), offset.dot(up)) / half;
        let normalised = Vec2::new(centred.x * 0.5 + 0.5, 0.5 - centred.y * 0.5);
        normalised * viewport
    }

    /// Sets `zoom`, clamped to [`Self::MIN_ZOOM`]..=[`Self::MAX_ZOOM`].
    ///
    /// `NaN` leaves the previous value alone: `f32::clamp` only panics when a
    /// bound is `NaN` or `min > max`, neither true here, so a `NaN` `self`
    /// would pass straight through unclamped. The guard exists anyway because
    /// without it `zoom` would silently become `NaN` and stay poisoned —
    /// every later frame's projection matrix depends on it.
    pub fn set_zoom(&mut self, zoom: f32) {
        if zoom.is_nan() {
            return;
        }
        self.zoom = zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
    }

    /// Scales the zoom by `factor`, keeping the world point under `anchor`
    /// under `anchor` afterwards.
    ///
    /// Measuring the same anchor before and after is what makes this correct at
    /// the clamp too: if `set_zoom` refused the change, the two samples are
    /// equal and the camera does not move.
    pub fn zoom_around(&mut self, anchor: Vec2, viewport: Vec2, factor: f32) {
        let before = self.viewport_to_world(anchor, viewport);
        self.set_zoom(self.zoom * factor);
        let after = self.viewport_to_world(anchor, viewport);
        self.position += before - after;
    }

    /// A viewport no smaller than one point per axis.
    ///
    /// A panel dragged shut reports zero. `viewport_to_world` divides by this;
    /// `world_to_viewport` multiplies by it, but calls this first so both
    /// conversions agree on the same guarded value.
    fn guard(viewport: Vec2) -> Vec2 {
        viewport.max(Vec2::ONE)
    }

    /// The matrix handed to the shader.
    ///
    /// The `directx` projection and not the `opengl` or `vulkan` one: glam
    /// names these after the clip space they emit, and WebGPU shares DirectX's
    /// — Z in [0, 1] with Y up. The OpenGL variant maps Z to [-1, 1] and
    /// throws half the depth range behind the near plane; the Vulkan variant
    /// is Y-down and renders everything upside down.
    pub fn view_projection(&self) -> Mat4 {
        let half = self.half_extents();
        let projection =
            rh::proj::directx::orthographic(-half.x, half.x, -half.y, half.y, -1.0, 1.0);
        // Turned back about the view's centre, after moving it to the
        // origin: the inverse of placing the view, turn then translation.
        let view = Mat4::from_rotation_z(-self.turn())
            * Mat4::from_translation(-Vec3::new(self.position.x, self.position.y, 0.0));
        projection * view
    }
}

/// The matrix for drawing in a target's own pixels: origin top-left, **y
/// down**, one unit per pixel - what screen-space UI is laid out in.
///
/// Not a [`Camera2D`]: its zoom is clamped for navigation, and a
/// 1080-pixel-tall target would need a zoom far below that floor. The same
/// `directx` projection as [`Camera2D::view_projection`], with bottom and top
/// swapped so y runs down.
pub fn screen_projection(width: f32, height: f32) -> Mat4 {
    rh::proj::directx::orthographic(0.0, width.max(1.0), height.max(1.0), 0.0, -1.0, 1.0)
}

mod binding;

pub use binding::{CameraBinding, CameraUniform};

#[cfg(test)]
mod tests;
