//! What a joint carries from one step to the next.

use voltra_render::glam::Vec2;

/// The impulses a joint accumulated, carried from one step to the next.
///
/// Each is one sub-step's: the solver warm starts every sub-step with them,
/// so a constraint's force is its impulse over the sub-step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct JointImpulses {
    /// Holding the anchors together (revolute, weld), or pulling one to the
    /// other (mouse).
    pub linear: Vec2,
    /// Holding the angle (weld, prismatic).
    pub angular: f32,
    /// Along the axis: the rod or the spring (distance).
    pub axial: f32,
    /// Across the axis: keeping the anchor on its line (wheel, prismatic).
    pub perp: f32,
    /// The spring's, pulling towards its rest (revolute, wheel, prismatic).
    pub spring: f32,
    /// The motor's.
    pub motor: f32,
    /// The lower and upper limits' — angle, distance or slide.
    pub lower: f32,
    /// The upper limit's.
    pub upper: f32,
}
