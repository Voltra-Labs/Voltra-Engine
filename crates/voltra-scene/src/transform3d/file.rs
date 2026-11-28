//! A [`Transform3D`] as a file says it, and the repair on the way in.
//!
//! A file is text a person can edit, and `rotation: (0.0, 0.7, 0.0, 0.7)` is
//! what typing a quaternion by hand produces: near unit, not unit. Composed
//! as it stands it would scale the entity as well as turn it, and a zero or
//! NaN one would remove it from the screen with no error anywhere. So the
//! value is repaired as it is read, and said so once per run — a scene with
//! a thousand of them gets one line, not a thousand.

use std::sync::atomic::{AtomicBool, Ordering};

use voltra_render::glam::{Quat, Vec3};

use super::Transform3D;

/// Whether a repaired value has been warned about this run.
static WARNED: AtomicBool = AtomicBool::new(false);

/// The fields as written. Any left out is the default's, so a hand-written
/// `(translation: (0.0, 1.0, 0.0))` reads.
#[derive(serde::Deserialize)]
#[serde(default)]
pub(super) struct Stored {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

impl Default for Stored {
    fn default() -> Self {
        let Transform3D {
            translation,
            rotation,
            scale,
        } = Transform3D::default();
        Self {
            translation,
            rotation,
            scale,
        }
    }
}

impl From<Stored> for Transform3D {
    fn from(stored: Stored) -> Self {
        let (transform, repaired) = repair(stored);
        if let Some(what) = repaired {
            if !WARNED.swap(true, Ordering::Relaxed) {
                log::warn!(
                    "a Transform3D read from a file had {what}; repaired (further ones are \
                     repaired without a word)"
                );
            }
        }
        transform
    }
}

/// `stored` made usable, and what was wrong with it, if anything.
///
/// A rotation within glam's `is_normalized` tolerance is kept to the bit, so
/// a file this build wrote reads back exactly and saves byte for byte.
fn repair(stored: Stored) -> (Transform3D, Option<&'static str>) {
    let mut what = None;
    let rotation = if !stored.rotation.is_finite() || stored.rotation.length_squared() == 0.0 {
        what = Some("a rotation that is not a rotation (zero or not finite)");
        Quat::IDENTITY
    } else if !stored.rotation.is_normalized() {
        what = Some("a rotation that is not unit length");
        stored.rotation.normalize()
    } else {
        stored.rotation
    };
    let translation = if stored.translation.is_finite() {
        stored.translation
    } else {
        what = Some("a translation that is not finite");
        Vec3::ZERO
    };
    let scale = if stored.scale.is_finite() {
        stored.scale
    } else {
        what = Some("a scale that is not finite");
        Vec3::ONE
    };
    (
        Transform3D {
            translation,
            rotation,
            scale,
        },
        what,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Transform3D {
        ron::from_str(text).expect("reads")
    }

    #[test]
    fn a_unit_rotation_is_kept_to_the_bit() {
        let turned = Transform3D::from_euler_degrees(Vec3::new(10.0, 33.0, -71.0));
        let text = ron::to_string(&turned).expect("writes");
        let back = read(&text);
        assert_eq!(
            back.rotation.to_array().map(f32::to_bits),
            turned.rotation.to_array().map(f32::to_bits)
        );
    }

    #[test]
    fn a_hand_typed_rotation_is_normalised() {
        // 0.7 for 1/√2: off by 2 % in length squared.
        let back = read("(rotation: (0.0, 0.7, 0.0, 0.7))");
        assert!((back.rotation.length() - 1.0).abs() < 1e-6);
        let expected = Quat::from_rotation_y(90f32.to_radians());
        assert!(back.rotation.abs_diff_eq(expected, 1e-6), "{back:?}");
    }

    #[test]
    fn a_zero_or_nan_rotation_is_the_identity() {
        assert_eq!(
            read("(rotation: (0.0, 0.0, 0.0, 0.0))").rotation,
            Quat::IDENTITY
        );
        let (repaired, what) = repair(Stored {
            rotation: Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0),
            ..Stored::default()
        });
        assert_eq!(repaired.rotation, Quat::IDENTITY);
        assert!(what.is_some());
    }

    #[test]
    fn a_non_finite_translation_or_scale_is_reset() {
        let (repaired, what) = repair(Stored {
            translation: Vec3::new(f32::INFINITY, 0.0, 0.0),
            scale: Vec3::splat(f32::NAN),
            ..Stored::default()
        });
        assert_eq!(repaired.translation, Vec3::ZERO);
        assert_eq!(repaired.scale, Vec3::ONE);
        assert!(what.is_some());
    }

    #[test]
    fn fields_left_out_are_the_defaults() {
        let back = read("(translation: (1.0, 2.0, 3.0))");
        assert_eq!(back.translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(back.rotation, Quat::IDENTITY);
        assert_eq!(back.scale, Vec3::ONE);
        assert_eq!(read("()"), Transform3D::default());
    }
}
