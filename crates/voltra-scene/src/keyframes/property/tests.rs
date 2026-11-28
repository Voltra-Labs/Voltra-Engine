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
    for property in Property::ALL {
        let value = match property.kind() {
            ValueKind::Float => Value::Float(0.25),
            ValueKind::Vec2 => Value::Vec2(voltra_render::glam::Vec2::new(1.0, 2.0)),
            ValueKind::Color => Value::Color([0.1, 0.2, 0.3, 1.0]),
            ValueKind::Int => Value::Int(3),
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
