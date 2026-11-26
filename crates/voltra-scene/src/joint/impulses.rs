//! What a joint carries from one step to the next.

use voltra_render::glam::Vec2;

/// The impulses a joint accumulated, carried from one step to the next.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct JointImpulses {
    /// Holding the anchors together (revolute, weld).
    pub linear: Vec2,
    /// Holding the angle (weld).
    pub angular: f32,
    /// Along the axis: the rod or the spring (distance).
    pub axial: f32,
    /// The motor's.
    pub motor: f32,
    /// The lower and upper limits' — angle or distance.
    pub lower: f32,
    /// The upper limit's.
    pub upper: f32,
}
