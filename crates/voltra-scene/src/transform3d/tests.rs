use std::f32::consts::FRAC_PI_2;

use super::*;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-5
}

#[test]
fn the_default_is_the_identity() {
    let point = Vec3::new(3.0, -4.0, 5.0);
    assert!(close(
        Transform3D::default().matrix().transform_point3(point),
        point
    ));
}

#[test]
fn scale_then_rotate_then_translate() {
    // Scaled by two, a quarter turn about Y, moved ten along X: +X one
    // unit becomes −Z two units, then ten along X.
    let t = Transform3D::from_translation(Vec3::new(10.0, 0.0, 0.0))
        .with_rotation(Quat::from_rotation_y(FRAC_PI_2))
        .with_scale(Vec3::splat(2.0));
    assert!(close(
        t.matrix().transform_point3(Vec3::X),
        Vec3::new(10.0, 0.0, -2.0)
    ));
}

#[test]
fn forward_is_minus_z_and_turns_with_the_rotation() {
    let t = Transform3D::default();
    assert!(close(t.forward(), Vec3::NEG_Z));
    assert!(close(t.right(), Vec3::X));
    assert!(close(t.up(), Vec3::Y));
    // A yaw of +90° (counter-clockwise seen from above) faces −X.
    let yawed = Transform3D::from_euler_degrees(Vec3::new(0.0, 90.0, 0.0));
    assert!(close(yawed.forward(), Vec3::NEG_X), "{:?}", yawed.forward());
}

#[test]
fn euler_degrees_are_applied_y_then_x_then_z() {
    // YXZ: the matrix is Ry · Rx · Rz.
    let degrees = Vec3::new(30.0, 45.0, 60.0);
    let built = Transform3D::from_euler_degrees(degrees).rotation;
    let composed = Quat::from_rotation_y(45f32.to_radians())
        * Quat::from_rotation_x(30f32.to_radians())
        * Quat::from_rotation_z(60f32.to_radians());
    assert!(built.abs_diff_eq(composed, 1e-6));
    assert!(close(
        Transform3D::from_euler_degrees(degrees).euler_degrees(),
        degrees
    ));
}

#[test]
fn a_transform_survives_a_trip_through_its_matrix() {
    let t = Transform3D::from_translation(Vec3::new(-3.0, 7.0, 2.0))
        .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.3, -1.1, 2.0))
        .with_scale(Vec3::new(2.0, 5.0, 0.5));
    let back = Transform3D::from_matrix(t.matrix());
    assert!(close(back.translation, t.translation));
    assert!(back.rotation.abs_diff_eq(t.rotation, 1e-5));
    assert!(close(back.scale, t.scale));
}

#[test]
fn a_collapsed_matrix_comes_back_unrotated_not_nan() {
    let flat = Transform3D::default()
        .with_rotation(Quat::from_rotation_x(0.4))
        .with_scale(Vec3::new(2.0, 0.0, 1.0));
    let back = Transform3D::from_matrix(flat.matrix());
    assert_eq!(back.rotation, Quat::IDENTITY);
    assert!(back.scale.is_finite() && back.translation.is_finite());
    assert!((back.scale.x - 2.0).abs() < 1e-6);
}

#[test]
fn a_sheared_matrix_comes_back_as_a_unit_rotation() {
    // A turned child under a non-uniformly scaled parent: the shear is
    // dropped, and what is kept is still a rotation.
    let parent = Transform3D::default().with_scale(Vec3::new(3.0, 1.0, 1.0));
    let child = Transform3D::default().with_rotation(Quat::from_rotation_z(0.7));
    let back = Transform3D::from_matrix(parent.matrix() * child.matrix());
    assert!((back.rotation.length() - 1.0).abs() < 1e-6);
}

#[test]
fn looking_at_points_forward_at_the_target() {
    let eye = Transform3D::from_translation(Vec3::new(0.0, 2.0, 5.0));
    let looking = eye.looking_at(Vec3::new(4.0, 2.0, 5.0), Vec3::Y);
    assert!(close(looking.forward(), Vec3::X), "{:?}", looking.forward());
    assert!(close(looking.up(), Vec3::Y));
    assert_eq!(looking.translation, eye.translation);
}

#[test]
fn looking_straight_down_or_at_itself_stays_finite() {
    let eye = Transform3D::from_translation(Vec3::new(0.0, 10.0, 0.0));
    let down = eye.looking_at(Vec3::ZERO, Vec3::Y);
    assert!(down.rotation.is_finite());
    assert!(close(down.forward(), Vec3::NEG_Y), "{:?}", down.forward());
    assert_eq!(eye.looking_at(eye.translation, Vec3::Y), eye);
}

#[test]
fn a_scene_value_writes_and_reads_back_byte_for_byte() {
    let t = Transform3D::from_translation(Vec3::new(1.5, -2.25, 3.0))
        .with_rotation(Transform3D::from_euler_degrees(Vec3::new(12.0, 180.0, -7.5)).rotation)
        .with_scale(Vec3::new(1.0, 2.0, 0.5));
    let text = ron::ser::to_string_pretty(&t, ron::ser::PrettyConfig::default()).expect("writes");
    let back: Transform3D = ron::from_str(&text).expect("reads");
    assert_eq!(back, t);
    let again =
        ron::ser::to_string_pretty(&back, ron::ser::PrettyConfig::default()).expect("writes again");
    assert_eq!(again, text);
}
