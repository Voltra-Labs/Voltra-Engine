//! What is mixed and what is partial.

use voltra_scene::format::serial_tree::Step;
use voltra_scene::format::ComponentRegistry;
use voltra_scene::{Collider2D, Sprite2D};

use super::{begin, color, three, BLUE, GREEN, RED};

#[test]
fn equal_fields_are_not_mixed_and_differing_ones_are() {
    let registry = ComponentRegistry::with_defaults();
    let (world, selected) = three([RED, RED, RED]);
    let edit = begin(&world, &registry, &selected);
    assert!(edit.is_multi());
    assert!(edit.mixed("Sprite").is_empty());
    assert!(!edit.is_mixed("Sprite", &color()));

    let (world, selected) = three([RED, GREEN, RED]);
    let edit = begin(&world, &registry, &selected);
    // The two channels that differ, nested in the colour, and nothing else.
    assert_eq!(
        edit.mixed("Sprite"),
        [
            vec![Step::Field("color"), Step::Index(0)],
            vec![Step::Field("color"), Step::Index(1)],
        ]
    );
    assert!(edit.is_mixed("Sprite", &color()), "under the colour");
    assert!(edit.is_mixed("Sprite", &[]), "the sprite as a whole");
    assert!(!edit.is_mixed("Sprite", &[Step::Field("flip_x")]));
    assert!(!edit.is_mixed("Transform", &[]));
    assert!(edit.mixed("Transform").is_empty());
}

#[test]
fn floats_are_mixed_by_their_exact_value_and_nan_matches_nan() {
    let registry = ComponentRegistry::with_defaults();
    let near = [1.0, 0.0, 0.0, 1.000_000_1];
    let (world, selected) = three([RED, near, RED]);
    assert!(begin(&world, &registry, &selected).is_mixed("Sprite", &color()));

    let nan = [f32::NAN, 0.0, 0.0, 1.0];
    let (world, selected) = three([nan, nan, nan]);
    assert!(!begin(&world, &registry, &selected).is_mixed("Sprite", &color()));
}

#[test]
fn a_component_only_some_have_is_partial_and_not_editable() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, RED, RED]);
    world.insert(selected[0], Collider2D::default());
    world.insert(selected[2], Collider2D::default());
    let edit = begin(&world, &registry, &selected);
    assert_eq!(edit.partial(), [("Collider", 2)]);
    assert!(!edit.editable(&["Collider"]));
    assert!(!edit.editable(&["Sprite", "Collider"]));
    assert!(edit.editable(&["Sprite", "Transform"]));
    // None has a camera, so one can be added to all.
    assert!(edit.editable(&["Camera"]));
}

#[test]
fn one_entity_is_not_a_multi_edit() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, GREEN, BLUE]);
    let edit = begin(&world, &registry, &selected[2..]);
    assert!(!edit.is_multi());
    assert!(edit.mixed("Sprite").is_empty());
    assert!(edit.editable(&["Collider"]));
    if let Some(s) = world.get_mut::<Sprite2D>(selected[2]) {
        s.flip_x = true;
    }
    assert!(edit.finish(&mut world, &registry).is_empty());
    assert!(!world.get::<Sprite2D>(selected[0]).expect("s").flip_x);
}

#[test]
fn one_sprite_on_another_layer_mixes_the_layer_alone() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, RED, RED]);
    world
        .get_mut::<Sprite2D>(selected[0])
        .expect("s")
        .sorting_layer = "Front".into();
    let edit = begin(&world, &registry, &selected);
    assert_eq!(edit.mixed("Sprite"), [vec![Step::Field("sorting_layer")]]);
    assert!(!edit.is_mixed("Sprite", &color()));
    assert!(!edit.is_mixed("Sprite", &[Step::Field("flip_x")]));
}
