//! How a 3D camera projects: in perspective, or orthographically.

use voltra_render::glam::Affine3A;
use voltra_render::render3d::View3D;

/// A [`Camera3D`](super::Camera3D)'s projection, in the units an author
/// types: degrees and metres.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Projection3D {
    /// Things shrink with distance.
    Perspective {
        /// The view's height as an angle, in degrees. 60 by default
        /// (Unity's; Godot uses 75, Bevy 45).
        #[serde(default = "default_fov")]
        fov_y_degrees: f32,
        /// The nearest distance drawn, metres; 0.1 by default.
        #[serde(default = "default_near")]
        near: f32,
        /// The furthest distance drawn, metres, or `None` — the default —
        /// for no far plane at all.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        far: Option<f32>,
    },
    /// Size does not change with distance: a strategy map, a technical view.
    Orthographic {
        /// Half the height it shows, metres, as
        /// [`Camera2D::size`](crate::Camera2D::size) is.
        size: f32,
        /// The nearest distance drawn, metres.
        #[serde(default = "default_near")]
        near: f32,
        /// The furthest distance drawn, metres: an orthographic view has
        /// no infinite far plane.
        #[serde(default = "default_ortho_far")]
        far: f32,
    },
}

fn default_fov() -> f32 {
    Projection3D::DEFAULT_FOV_Y_DEGREES
}

fn default_near() -> f32 {
    View3D::DEFAULT_NEAR
}

fn default_ortho_far() -> f32 {
    Projection3D::DEFAULT_ORTHOGRAPHIC_FAR
}

impl Default for Projection3D {
    fn default() -> Self {
        Self::Perspective {
            fov_y_degrees: Self::DEFAULT_FOV_Y_DEGREES,
            near: View3D::DEFAULT_NEAR,
            far: None,
        }
    }
}

impl Projection3D {
    /// The field of view a perspective camera has unless told otherwise.
    pub const DEFAULT_FOV_Y_DEGREES: f32 = 60.0;
    /// The far plane an orthographic camera has unless told otherwise.
    pub const DEFAULT_ORTHOGRAPHIC_FAR: f32 = 1000.0;
    /// The size an orthographic camera has when switched to: 5 m above and
    /// below, Unity's.
    pub const DEFAULT_ORTHOGRAPHIC_SIZE: f32 = 5.0;

    /// An orthographic projection `size` metres above and below the
    /// centre, near and far at their defaults.
    pub fn orthographic(size: f32) -> Self {
        Self::Orthographic {
            size,
            near: View3D::DEFAULT_NEAR,
            far: Self::DEFAULT_ORTHOGRAPHIC_FAR,
        }
    }

    /// What is wrong with it, if anything: a field the view will replace
    /// with something drawable ([`View3D`]'s rules), said in words.
    pub fn problem(&self) -> Option<&'static str> {
        match *self {
            Self::Perspective {
                fov_y_degrees,
                near,
                far,
            } => {
                if !(fov_y_degrees > 0.0 && fov_y_degrees < 180.0) {
                    Some("the field of view must be between 0° and 180°")
                } else if !(near > 0.0 && near.is_finite()) {
                    Some("a perspective near plane must be more than 0")
                } else if far.is_some_and(|far| !(far > near && far.is_finite())) {
                    Some("the far plane must be past the near plane")
                } else {
                    None
                }
            }
            Self::Orthographic { size, near, far } => {
                if !(size > 0.0 && size.is_finite()) {
                    Some("an orthographic size must be more than 0")
                } else if !(near >= 0.0 && near.is_finite()) {
                    Some("an orthographic near plane must be 0 or more")
                } else if !(far > near && far.is_finite()) {
                    Some("the far plane must be past the near plane")
                } else {
                    None
                }
            }
        }
    }

    /// The view from `placement` (the camera's world matrix; its scale is
    /// ignored) at `aspect`, width over height.
    pub fn view(&self, placement: Affine3A, aspect: f32) -> View3D {
        match *self {
            Self::Perspective {
                fov_y_degrees,
                near,
                far,
            } => View3D::perspective(placement, fov_y_degrees.to_radians(), aspect, near, far),
            Self::Orthographic { size, near, far } => {
                View3D::orthographic(placement, size, aspect, near, far)
            }
        }
    }
}
