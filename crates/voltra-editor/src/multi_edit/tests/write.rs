//! What is written across, and what is left alone.

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;
use voltra_scene::format::serial_tree::Step;
use voltra_scene::format::ComponentRegistry;
use voltra_scene::{Collider, SceneId, Sprite, Transform};

use super::{begin, three, BLUE, GREEN, RED};

#[test]
fn editing_one_field_writes_only_that_field_to_every_entity() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, GREEN, BLUE]);
    let edit = begin(&world, &registry, &selected);
    world.get_mut::<Sprite>(selected[2]).expect("s").flip_x = true;

    let written = edit.finish(&mut world, &registry);

    assert_eq!(written, [selected[0], selected[1]]);
    for (&entity, color) in selected.iter().zip([RED, GREEN, BLUE]) {
        let sprite = world.get::<Sprite>(entity).expect("s");
        assert!(sprite.flip_x, "flipped");
        assert_eq!(sprite.color, color, "each keeps its own colour");
    }
}

#[test]
fn one_channel_of_a_mixed_colour_is_written_alone() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, GREEN, BLUE]);
    let edit = begin(&world, &registry, &selected);
    world.get_mut::<Sprite>(selected[2]).expect("s").color[3] = 0.5;

    edit.finish(&mut world, &registry);

    for (&entity, mut color) in selected.iter().zip([RED, GREEN, BLUE]) {
        color[3] = 0.5;
        assert_eq!(world.get::<Sprite>(entity).expect("s").color, color);
    }
}

#[test]
fn a_field_an_entity_does_not_have_is_left_alone_and_a_new_shape_is_written_whole() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, RED, RED]);
    let boxed = Collider::Box {
        half_extents: Vec2::ONE,
    };
    let circle = Collider::Circle { radius: 2.0 };
    world.insert(selected[0], circle);
    world.insert(selected[1], boxed);
    world.insert(selected[2], boxed);

    let edit = begin(&world, &registry, &selected);
    assert!(edit.is_mixed("Collider", &[]), "two shapes");
    let grown = Collider::Box {
        half_extents: Vec2::new(3.0, 1.0),
    };
    world.insert(selected[2], grown);
    edit.finish(&mut world, &registry);
    assert_eq!(world.get::<Collider>(selected[0]), Some(&circle), "no box");
    assert_eq!(world.get::<Collider>(selected[1]), Some(&grown));

    // Turning the primary into a circle is a change of the shape itself.
    let edit = begin(&world, &registry, &selected);
    let small = Collider::Circle { radius: 0.5 };
    world.insert(selected[2], small);
    edit.finish(&mut world, &registry);
    assert_eq!(world.get::<Collider>(selected[0]), Some(&small));
    assert_eq!(world.get::<Collider>(selected[1]), Some(&small));
}

#[test]
fn a_component_added_or_removed_on_the_primary_is_on_all_or_on_none() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, GREEN, BLUE]);
    let edit = begin(&world, &registry, &selected);
    world.insert(selected[2], Collider::Circle { radius: 3.0 });
    edit.finish(&mut world, &registry);
    for &entity in &selected {
        assert_eq!(
            world.get::<Collider>(entity),
            Some(&Collider::Circle { radius: 3.0 })
        );
    }

    let edit = begin(&world, &registry, &selected);
    world.remove::<Collider>(selected[2]);
    edit.finish(&mut world, &registry);
    assert!(selected.iter().all(|&e| world.get::<Collider>(e).is_none()));
}

#[test]
fn a_component_done_for_each_is_not_copied_from_the_primary() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, GREEN, BLUE]);
    let mut edit = begin(&world, &registry, &selected);
    world.get_mut::<Sprite>(selected[2]).expect("s").flip_y = true;
    edit.done_for_each("Sprite");
    assert!(edit.finish(&mut world, &registry).is_empty());
    assert!(!world.get::<Sprite>(selected[0]).expect("s").flip_y);
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
struct Coin {
    value: u32,
    label: String,
}

#[test]
fn a_component_the_game_registered_is_edited_across_the_selection() {
    let mut registry = ComponentRegistry::with_defaults();
    registry.register::<Coin>("Coin").with_template();
    let (mut world, selected) = three([RED, RED, RED]);
    for (&entity, label) in selected.iter().zip(["a", "b", "c"]) {
        world.insert(
            entity,
            Coin {
                value: 1,
                label: label.into(),
            },
        );
    }
    let edit = begin(&world, &registry, &selected);
    assert_eq!(edit.mixed("Coin"), [vec![Step::Field("label")]]);
    world.get_mut::<Coin>(selected[2]).expect("c").value = 10;
    edit.finish(&mut world, &registry);
    for (&entity, label) in selected.iter().zip(["a", "b", "c"]) {
        let coin = world.get::<Coin>(entity).expect("c");
        assert_eq!((coin.value, coin.label.as_str()), (10, label));
    }
}

#[test]
fn a_sorting_layer_set_on_the_primary_is_the_only_thing_written() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, GREEN, BLUE]);
    for (&entity, flip) in selected.iter().zip([false, true, false]) {
        world.get_mut::<Sprite>(entity).expect("s").flip_x = flip;
    }
    world.get_mut::<Sprite>(selected[2]).expect("s").frame = 3;
    let edit = begin(&world, &registry, &selected);
    world
        .get_mut::<Sprite>(selected[2])
        .expect("s")
        .sorting_layer = "Front".into();

    edit.finish(&mut world, &registry);

    for (&entity, (color, flip)) in
        selected
            .iter()
            .zip([(RED, false), (GREEN, true), (BLUE, false)])
    {
        let sprite = world.get::<Sprite>(entity).expect("s");
        assert_eq!(sprite.sorting_layer, "Front");
        assert_eq!((sprite.color, sprite.flip_x), (color, flip), "its own");
    }
    assert_eq!(world.get::<Sprite>(selected[0]).expect("s").frame, 0);

    // And back to the default layer, which a file leaves out: out of all.
    let edit = begin(&world, &registry, &selected);
    let default = Sprite::default().sorting_layer;
    world
        .get_mut::<Sprite>(selected[2])
        .expect("s")
        .sorting_layer = default.clone();
    edit.finish(&mut world, &registry);
    for (&entity, color) in selected.iter().zip([RED, GREEN, BLUE]) {
        let sprite = world.get::<Sprite>(entity).expect("s");
        assert_eq!(
            (sprite.sorting_layer.as_str(), sprite.color),
            (default.as_str(), color)
        );
    }
}

#[test]
fn an_optional_joint_field_set_on_one_copies_nothing_else() {
    use voltra_scene::Joint2D;
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, RED, RED]);
    let partner = SceneId::new();
    for (&entity, x) in selected.iter().zip([1.0, 2.0, 3.0]) {
        let joint = Joint2D {
            anchor_a: Vec2::new(x, 0.0),
            b: (x == 3.0).then_some(partner),
            ..Joint2D::default()
        };
        world.insert(entity, joint);
    }
    let edit = begin(&world, &registry, &selected);
    world
        .get_mut::<Joint2D>(selected[2])
        .expect("j")
        .break_force = Some(50.0);
    edit.finish(&mut world, &registry);
    for (&entity, x) in selected.iter().zip([1.0, 2.0, 3.0]) {
        let joint = world.get::<Joint2D>(entity).expect("j");
        assert_eq!(joint.break_force, Some(50.0));
        assert_eq!(joint.anchor_a, Vec2::new(x, 0.0), "its own anchor");
        assert_eq!(joint.b, (x == 3.0).then_some(partner), "its own body");
    }

    // Unset again on the primary: unset on all.
    let edit = begin(&world, &registry, &selected);
    world
        .get_mut::<Joint2D>(selected[2])
        .expect("j")
        .break_force = None;
    edit.finish(&mut world, &registry);
    assert!(selected
        .iter()
        .all(|&e| world.get::<Joint2D>(e).expect("j").break_force.is_none()));
}

#[test]
fn a_touched_field_is_written_even_as_the_primary_had_it() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, GREEN, BLUE]);
    world.get_mut::<Sprite>(selected[0]).expect("s").sort_order = 4;
    let mut edit = begin(&world, &registry, &selected);
    // The primary's own value, chosen again: nothing moved on it.
    edit.touch("Sprite", vec![Step::Field("sort_order")]);
    edit.touch("Parent", vec![]);
    let written = edit.finish(&mut world, &registry);
    assert_eq!(written, [selected[0]], "only the one that differed");
    for (&entity, color) in selected.iter().zip([RED, GREEN, BLUE]) {
        let sprite = world.get::<Sprite>(entity).expect("s");
        assert_eq!((sprite.sort_order, sprite.color), (0, color));
    }

    // A field left out by the primary's value, touched: left out of all.
    world
        .get_mut::<Sprite>(selected[0])
        .expect("s")
        .sorting_layer = "Front".into();
    let mut edit = begin(&world, &registry, &selected);
    edit.touch("Sprite", vec![Step::Field("sorting_layer")]);
    edit.finish(&mut world, &registry);
    let default = Sprite::default().sorting_layer;
    assert_eq!(
        world.get::<Sprite>(selected[0]).expect("s").sorting_layer,
        default
    );
}

#[test]
fn a_primary_gone_by_the_end_writes_nothing() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, GREEN, BLUE]);
    let edit = begin(&world, &registry, &selected);
    world.despawn(selected[2]);
    assert!(edit.finish(&mut world, &registry).is_empty());
    assert!(world.get::<Sprite>(selected[0]).is_some(), "kept");
    assert!(world.get::<Transform>(selected[1]).is_some(), "kept");
}
