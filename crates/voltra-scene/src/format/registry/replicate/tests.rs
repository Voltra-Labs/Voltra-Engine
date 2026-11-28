use super::*;
use crate::Transform2D;
use voltra_render::glam::Vec2;

#[derive(Debug, PartialEq, Serialize, serde::Deserialize)]
struct Health(u32);

#[test]
fn transform_is_replicated_and_interpolated_by_default() {
    let registry = ComponentRegistry::with_defaults();
    let replicated = registry.replicated();
    let transform = replicated
        .iter()
        .find(|c| c.name == "Transform2D")
        .expect("Transform replicates");
    assert!(transform.interpolated);
    let transform3d = replicated
        .iter()
        .find(|c| c.name == crate::Transform3D::NAME)
        .expect("Transform3D replicates");
    assert!(transform3d.interpolated);
}

#[test]
fn a_3d_transform_round_trips_and_slerps_on_the_wire() {
    use voltra_render::glam::{Quat, Vec3};
    let registry = ComponentRegistry::with_defaults();
    let transform = *registry
        .replicated()
        .iter()
        .find(|c| c.name == crate::Transform3D::NAME)
        .expect("replicated");
    let mut world = World::new();
    let a = world.spawn();
    let at = crate::Transform3D::from_translation(Vec3::new(2.0, 4.0, -6.0))
        .with_rotation(Quat::from_rotation_y(1.0));
    world.insert(a, at);
    let bytes = registry
        .encode_replicated(transform, &world, a)
        .expect("has one")
        .expect("encodes");
    assert_eq!(bytes.len(), 40, "ten floats, until its own wire form");
    let b = world.spawn();
    registry
        .apply_replicated(transform, &mut world, b, &bytes)
        .expect("decodes");
    assert_eq!(world.get::<crate::Transform3D>(b), Some(&at), "bit for bit");

    let zero = postcard::to_allocvec(&crate::Transform3D::default()).expect("encodes");
    let half = registry
        .lerp_replicated(transform, &zero, &bytes, 0.5)
        .expect("interpolated")
        .expect("lerps");
    let half: crate::Transform3D = decode(&half).expect("decodes");
    assert_eq!(half.translation, Vec3::new(1.0, 2.0, -3.0));
    let off = 2.0
        * (half.rotation * Quat::from_rotation_y(0.5).conjugate())
            .xyz()
            .length()
            .asin();
    assert!(off < 1e-5, "half of 1 rad about Y: {off} rad off");
}

#[test]
fn a_value_round_trips_and_interpolates() {
    let registry = ComponentRegistry::with_defaults();
    let transform = registry.replicated()[0];
    let mut world = World::new();
    let a = world.spawn();
    world.insert(a, Transform2D::from_translation(Vec2::new(2.0, 4.0)));

    let bytes = registry
        .encode_replicated(transform, &world, a)
        .expect("has one")
        .expect("encodes");
    assert_eq!(bytes.len(), 20, "five floats, nothing else");
    let b = world.spawn();
    registry
        .apply_replicated(transform, &mut world, b, &bytes)
        .expect("decodes");
    assert_eq!(world.get::<Transform2D>(b), world.get::<Transform2D>(a));

    let zero = postcard::to_allocvec(&Transform2D::default()).expect("encodes");
    let half = registry
        .lerp_replicated(transform, &zero, &bytes, 0.5)
        .expect("interpolated")
        .expect("lerps");
    let half: Transform2D = decode(&half).expect("decodes");
    assert_eq!(half.translation, Vec2::new(1.0, 2.0));
}

#[test]
fn a_type_must_be_registered_before_it_replicates() {
    let mut registry = ComponentRegistry::new();
    assert!(matches!(
        registry.replicate::<Health>(),
        Err(ReplicateError::NotRegistered(_))
    ));
    registry.register::<Health>("Health");
    registry.replicate::<Health>().expect("registered now");
    let health = registry.replicated()[0];
    assert_eq!(health.name, "Health");
    assert!(!health.interpolated);
    assert!(registry.lerp_replicated(health, &[1], &[2], 0.5).is_none());
}

#[test]
fn junk_and_trailing_bytes_change_nothing() {
    let mut registry = ComponentRegistry::new();
    registry.register::<Health>("Health").replicated();
    let health = registry.replicated()[0];
    let mut world = World::new();
    let e = world.spawn();
    for junk in [
        &[][..],
        &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF][..],
        &[3, 0][..],
    ] {
        assert!(registry
            .apply_replicated(health, &mut world, e, junk)
            .is_err());
    }
    assert!(world.get::<Health>(e).is_none());
}
