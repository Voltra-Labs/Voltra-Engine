//! Where a thing is in the 3D world, how big it is, and which way it faces.
//!
//! The twin of [`Transform2D`](crate::Transform2D), not a widening of it:
//! the two dimensions are parallel subsystems (ARCHITECTURE, "3D: a parallel
//! subsystem"), and an entity carries one or the other. Y is up, the world is
//! right-handed, forward is −Z and a unit is a metre — glTF's convention, so
//! an imported model needs no axis conversion.

mod file;

use voltra_render::glam::{Affine3A, EulerRot, Mat3, Quat, Vec3};

/// A 3D position, rotation and scale, local to the entity's parent.
///
/// The rotation is stored as a quaternion: exact, no gimbal lock, and no
/// order to agree on. Euler angles are a view of it ([`euler_degrees`]),
/// which is what the inspector shows.
///
/// Read from a file through a repair: a quaternion that is not unit length
/// is normalised, and one that cannot be (zero, or not finite) is the
/// identity; a translation or scale that is not finite is reset. Each is
/// warned about, once per run.
///
/// [`euler_degrees`]: Self::euler_degrees
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "file::Stored")]
pub struct Transform3D {
    /// Where the entity is, in metres.
    pub translation: Vec3,
    /// Which way it faces; unit length.
    pub rotation: Quat,
    /// Per axis; `1` leaves the size.
    pub scale: Vec3,
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            // As 2D's: a default entity should be visible, not a point.
            scale: Vec3::ONE,
        }
    }
}

impl Transform3D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Transform3D";

    /// A transform at `translation`, unrotated and unscaled.
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Default::default()
        }
    }

    /// A transform turned by Euler angles in degrees, unmoved and unscaled.
    ///
    /// `degrees` is `(x, y, z)`, applied in Godot's YXZ order: the matrix is
    /// `Ry · Rx · Rz`, so a yaw about Y is the outermost turn and the angles
    /// read as yaw, pitch and roll. What the inspector types into.
    pub fn from_euler_degrees(degrees: Vec3) -> Self {
        Self {
            rotation: Quat::from_euler(
                EulerRot::YXZ,
                degrees.y.to_radians(),
                degrees.x.to_radians(),
                degrees.z.to_radians(),
            ),
            ..Default::default()
        }
    }

    /// The rotation as Euler angles in degrees, `(x, y, z)` in YXZ order —
    /// the inverse of [`from_euler_degrees`](Self::from_euler_degrees) up to
    /// the equivalent angles one rotation has (180° of yaw reads back as
    /// 180° of pitch and of roll, near enough). The editor keeps what was
    /// typed beside it for that reason.
    pub fn euler_degrees(&self) -> Vec3 {
        let (y, x, z) = self.rotation.to_euler(EulerRot::YXZ);
        Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
    }

    /// Sets the rotation.
    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    /// Sets the scale.
    pub fn with_scale(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
    }

    /// This transform turned so its forward (−Z) points at `target`, its up
    /// as near `up` as that allows. Translation and scale are kept.
    ///
    /// Left as it is when `target` is where it stands — there is no
    /// direction to face. When `up` lies along the direction (looking
    /// straight up or down), any perpendicular is taken as up instead of
    /// producing NaN.
    pub fn looking_at(self, target: Vec3, up: Vec3) -> Self {
        let Some(forward) = (target - self.translation).try_normalize() else {
            return self;
        };
        let right = forward
            .cross(up)
            .try_normalize()
            .unwrap_or_else(|| forward.any_orthonormal_vector());
        let up = right.cross(forward);
        Self {
            rotation: Quat::from_mat3(&Mat3::from_cols(right, up, -forward)).normalize(),
            ..self
        }
    }

    /// The way the entity faces: its −Z, in its parent's space.
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    /// Its +X, in its parent's space.
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    /// Its +Y, in its parent's space.
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// Local space to parent space.
    ///
    /// Scale, then rotate, then translate, as 2D's.
    pub fn matrix(&self) -> Affine3A {
        Affine3A::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    /// The transform an affine `matrix` describes.
    ///
    /// The inverse of [`matrix`](Self::matrix) for anything this type can
    /// produce. **Lossy for shear**, as 2D's
    /// [`from_matrix`](crate::Transform2D::from_matrix) is: a rotated child
    /// under a non-uniformly scaled parent shears, and three fields cannot
    /// say so — the rotation comes back as the nearest one, normalised.
    /// A matrix collapsed flat (a zero scale on some axis) has no rotation
    /// to recover: it comes back unrotated, with the lengths of its axes as
    /// the scale, rather than as NaN.
    pub fn from_matrix(matrix: Affine3A) -> Self {
        const COLLAPSED: f32 = 1e-12;
        let translation = Vec3::from(matrix.translation);
        if matrix.matrix3.determinant().abs() < COLLAPSED {
            let scale = Vec3::new(
                matrix.matrix3.x_axis.length(),
                matrix.matrix3.y_axis.length(),
                matrix.matrix3.z_axis.length(),
            );
            return Self {
                translation,
                rotation: Quat::IDENTITY,
                scale,
            };
        }
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        Self {
            translation,
            rotation: rotation.normalize(),
            scale,
        }
    }
}

#[cfg(test)]
mod tests;
