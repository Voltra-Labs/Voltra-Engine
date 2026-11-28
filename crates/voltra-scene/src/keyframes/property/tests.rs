use super::*;

#[test]
fn every_property_reads_back_what_it_wrote() {
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Transform2D::default());
    world.insert(entity, Sprite2D::default());
    world.insert(entity, Text::default());
    world.insert(entity, PointLight2D::default());
    world.insert(entity, GlobalLight2D::default());
    world.insert(entity, Tilemap::default());
    world.insert(entity, Transform3D::default());
    for property in Property::ALL {
        let value = match property.kind() {
            ValueKind::Float => Value::Float(0.25),
            ValueKind::Vec2 => Value::Vec2(voltra_render::glam::Vec2::new(1.0, 2.0)),
            ValueKind::Color => Value::Color([0.1, 0.2, 0.3, 1.0]),
            ValueKind::Int => Value::Int(3),
            ValueKind::Vec3 => Value::Vec3(voltra_render::glam::Vec3::new(1.0, 2.0, 3.0)),
            ValueKind::Quat => Value::Quat(voltra_render::glam::Quat::from_rotation_x(0.5)),
        };
        assert!(property.write(&mut world, entity, value), "{property:?}");
        assert_eq!(property.read(&world, entity), Some(value), "{property:?}");
        assert!(
            !property.write(&mut world, entity, Value::Int(i64::MIN))
                || property.kind() == ValueKind::Int,
            "{property:?} takes only its own kind"
        );
    }
    let bare = world.spawn();
    assert!(!Property::PointLight2DRadius.write(&mut world, bare, Value::Float(1.0)));
}

#[test]
fn a_clip_written_before_the_renames_still_reads() {
    // The six 0.4 names, each read as its 0.5 variant; a save writes
    // the new name, and the new name reads too.
    let renamed = [
        ("Translation", Property::Translation2D, "Translation2D"),
        ("Rotation", Property::Rotation2D, "Rotation2D"),
        ("Scale", Property::Scale2D, "Scale2D"),
        (
            "LightColor",
            Property::PointLight2DColor,
            "PointLight2DColor",
        ),
        (
            "LightIntensity",
            Property::PointLight2DIntensity,
            "PointLight2DIntensity",
        ),
        (
            "LightRadius",
            Property::PointLight2DRadius,
            "PointLight2DRadius",
        ),
    ];
    for (old, variant, new) in renamed {
        assert_eq!(ron::from_str::<Property>(old).expect(old), variant);
        assert_eq!(ron::from_str::<Property>(new).expect(new), variant);
        assert_eq!(ron::to_string(&variant).expect("writes"), new);
    }
}

#[test]
fn a_3d_rotation_is_written_unit_length_or_not_at_all() {
    use voltra_render::glam::Quat;
    let mut world = World::new();
    let entity = world.spawn();
    world.insert(entity, Transform3D::default());
    assert_eq!(Property::Rotation3D.mixing(), Mixing::Quaternion);
    let long = Quat::from_xyzw(0.0, 2.0, 0.0, 0.0);
    assert!(Property::Rotation3D.write(&mut world, entity, Value::Quat(long)));
    let written = world.get::<Transform3D>(entity).expect("t").rotation;
    assert!((written.length() - 1.0).abs() < 1e-6, "{written}");
    let zero = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
    assert!(!Property::Rotation3D.write(&mut world, entity, Value::Quat(zero)));
    assert_eq!(
        world.get::<Transform3D>(entity).expect("t").rotation,
        written
    );
    let flat = world.spawn();
    world.insert(flat, Transform2D::default());
    assert_eq!(
        Property::Translation3D.read(&world, flat),
        None,
        "2D has none"
    );
}
