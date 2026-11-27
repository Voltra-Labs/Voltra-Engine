use serde::{Deserialize, Serialize};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::format::serial_tree::Step;
use voltra_scene::format::{entity_with_id, ComponentRegistry};
use voltra_scene::{Collider, SceneId, Sprite, Transform};

use super::MultiEdit;
use crate::gizmo::Gizmo;
use crate::selection::Selection;
use crate::undo::{History, SceneView, UndoContext, UndoHost};

fn sprite(world: &mut World, color: [f32; 4]) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Transform::default());
    world.insert(entity, Sprite::new(color));
    entity
}

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Three sprites, the last the primary.
fn three(colors: [[f32; 4]; 3]) -> (World, Vec<Entity>) {
    let mut world = World::new();
    let entities = colors.iter().map(|&c| sprite(&mut world, c)).collect();
    (world, entities)
}

fn begin(world: &World, registry: &ComponentRegistry, selected: &[Entity]) -> MultiEdit {
    let primary = *selected.last().expect("something selected");
    MultiEdit::begin(world, registry, primary, selected.iter().copied())
}

fn color() -> Vec<Step> {
    vec![Step::Field("color")]
}

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
    world.insert(selected[0], Collider::default());
    world.insert(selected[2], Collider::default());
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
    if let Some(s) = world.get_mut::<Sprite>(selected[2]) {
        s.flip_x = true;
    }
    assert!(edit.finish(&mut world, &registry).is_empty());
    assert!(!world.get::<Sprite>(selected[0]).expect("s").flip_x);
}

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

/// An [`UndoHost`] with no `App` and no GPU behind it.
struct Host(World);

impl UndoHost for Host {
    fn world(&mut self) -> &mut World {
        &mut self.0
    }
    fn reset_physics(&mut self) {}
    fn resolve_scene_assets(&mut self) {}
}

#[test]
fn a_drag_across_three_entities_is_one_undo_step() {
    let registry = ComponentRegistry::with_defaults();
    let (world, selected) = three([RED, GREEN, BLUE]);
    let mut host = Host(world);
    let mut selection = Selection::default();
    selected.iter().for_each(|&e| selection.add(e));
    let ids: Vec<SceneId> = selection.ids(&host.0);
    let mut history = History::default();

    // Three frames of one drag on the primary's sort order.
    for order in 1..=3 {
        let view = SceneView {
            world: &host.0,
            registry: &registry,
            selected: ids.clone(),
        };
        history.watch(view, ids.iter().copied());
        let edit = begin(&host.0, &registry, &selected);
        host.0.get_mut::<Sprite>(selected[2]).expect("s").sort_order = order;
        edit.finish(&mut host.0, &registry);
        history.claim("Set sort order");
        let view = SceneView {
            world: &host.0,
            registry: &registry,
            selected: ids.clone(),
        };
        history.end_frame(view);
    }
    // Let go: a frame with no claim closes it.
    let view = SceneView {
        world: &host.0,
        registry: &registry,
        selected: ids.clone(),
    };
    history.watch(view, ids.iter().copied());
    let view = SceneView {
        world: &host.0,
        registry: &registry,
        selected: ids.clone(),
    };
    history.end_frame(view);

    assert_eq!(history.len(), 1, "one step for the drag and the three");
    for &id in &ids {
        let entity = entity_with_id(&host.0, id).expect("there");
        assert_eq!(host.0.get::<Sprite>(entity).expect("s").sort_order, 3);
    }

    let mut gizmo = Gizmo::default();
    let ctx = UndoContext {
        registry: &registry,
        selected: &mut selection,
        gizmo: &mut gizmo,
    };
    assert!(history.undo(ctx, &mut host));
    for (&id, color) in ids.iter().zip([RED, GREEN, BLUE]) {
        let entity = entity_with_id(&host.0, id).expect("there");
        let sprite = host.0.get::<Sprite>(entity).expect("s");
        assert_eq!((sprite.sort_order, sprite.color), (0, color));
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
fn one_sprite_on_another_layer_mixes_the_layer_alone() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED, RED, RED]);
    world
        .get_mut::<Sprite>(selected[0])
        .expect("s")
        .sorting_layer = "Front".into();
    let edit = begin(&world, &registry, &selected);
    assert_eq!(edit.mixed("Sprite"), [vec![Step::Field("sorting_layer")]]);
    assert!(!edit.is_mixed("Sprite", &color()));
    assert!(!edit.is_mixed("Sprite", &[Step::Field("flip_x")]));
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

/// One frame of the editor: watched, edited by `edit` on the primary,
/// written across, recorded.
fn frame(
    history: &mut History,
    host: &mut Host,
    registry: &ComponentRegistry,
    selected: &[Entity],
    ids: &[SceneId],
    edit: impl FnOnce(&mut World, Entity),
) {
    let view = SceneView {
        world: &host.0,
        registry,
        selected: ids.to_vec(),
    };
    history.watch(view, ids.iter().copied());
    let multi = begin(&host.0, registry, selected);
    edit(&mut host.0, selected[2]);
    multi.finish(&mut host.0, registry);
    let view = SceneView {
        world: &host.0,
        registry,
        selected: ids.to_vec(),
    };
    history.end_frame(view);
}

#[test]
fn a_component_added_to_all_is_undone_from_all_and_redone_on_all() {
    let registry = ComponentRegistry::with_defaults();
    let (world, selected) = three([RED, GREEN, BLUE]);
    let mut host = Host(world);
    let mut selection = Selection::default();
    selected.iter().for_each(|&e| selection.add(e));
    let ids = selection.ids(&host.0);
    let mut history = History::default();
    let circle = Collider::Circle { radius: 1.5 };
    frame(
        &mut history,
        &mut host,
        &registry,
        &selected,
        &ids,
        |w, p| {
            w.insert(p, circle);
        },
    );
    assert_eq!(history.len(), 1);

    let mut gizmo = Gizmo::default();
    let held = |host: &Host| {
        ids.iter()
            .map(|&id| {
                let e = entity_with_id(&host.0, id).expect("there");
                host.0.get::<Collider>(e).copied()
            })
            .collect::<Vec<_>>()
    };
    let ctx = UndoContext {
        registry: &registry,
        selected: &mut selection,
        gizmo: &mut gizmo,
    };
    assert!(history.undo(ctx, &mut host));
    assert_eq!(held(&host), [None, None, None]);
    let ctx = UndoContext {
        registry: &registry,
        selected: &mut selection,
        gizmo: &mut gizmo,
    };
    assert!(history.redo(ctx, &mut host));
    assert_eq!(held(&host), [Some(circle); 3]);
}
