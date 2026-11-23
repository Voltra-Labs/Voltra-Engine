use voltra_ecs::{Entity, World};
use voltra_render::glam::{IVec2, Vec2};

use super::*;
use crate::format::to_scene_file;
use crate::hierarchy::set_parent;
use crate::{Collider, CollisionLayers, ComponentRegistry, PhysicsMaterial, SceneId, Transform};

fn solid_layer(world: &mut World, at: Vec2, cells: &[(i32, i32)]) -> Entity {
    let mut map = Tilemap::new(Vec2::splat(0.5));
    map.collision = true;
    for &(x, y) in cells {
        map.set(IVec2::new(x, y), Some(Tile::new(0)));
    }
    let layer = world.spawn();
    world.insert(layer, Transform::from_translation(at));
    world.insert(layer, map);
    layer
}

fn generated(world: &World) -> Vec<(Entity, Transform, Collider)> {
    let mut found: Vec<_> = world
        .query::<TileCollider>()
        .map(|(entity, _)| {
            (
                entity,
                *world.get::<Transform>(entity).expect("generated with one"),
                *world.get::<Collider>(entity).expect("generated with one"),
            )
        })
        .collect();
    found.sort_by_key(|(entity, _, _)| *entity);
    found
}

fn half(collider: &Collider) -> Vec2 {
    match collider {
        Collider::Box { half_extents } => *half_extents,
        other => panic!("tiles generate boxes, not a {}", other.kind()),
    }
}

fn row(n: i32) -> Vec<(i32, i32)> {
    (0..n).map(|x| (x, 0)).collect()
}

#[test]
fn a_solid_row_becomes_one_static_box_where_the_tiles_are() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::new(10.0, 2.0), &row(4));

    sync_colliders(&mut world);

    let boxes = generated(&world);
    assert_eq!(boxes.len(), 1);
    let (entity, transform, collider) = boxes[0];
    // Four half-unit tiles: two units wide, half a unit tall, lower-left on
    // the layer's origin.
    assert_eq!(transform.translation, Vec2::new(11.0, 2.25));
    assert_eq!(half(&collider), Vec2::new(1.0, 0.25));
    assert!(world
        .get::<crate::RigidBody>(entity)
        .is_some_and(|body| body.inverse_mass == 0.0));
    assert_eq!(
        world.get::<TileCollider>(entity).map(|c| c.layer),
        Some(layer)
    );
}

#[test]
fn a_layer_without_collision_generates_nothing() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(3));
    world.get_mut::<Tilemap>(layer).expect("a layer").collision = false;

    sync_colliders(&mut world);

    assert!(generated(&world).is_empty());
}

#[test]
fn an_unchanged_layer_keeps_its_colliders() {
    let mut world = World::new();
    solid_layer(&mut world, Vec2::ZERO, &row(3));

    sync_colliders(&mut world);
    let first = generated(&world);
    sync_colliders(&mut world);

    assert_eq!(generated(&world), first, "rebuilt with nothing changed");
}

#[test]
fn painting_a_tile_rebuilds_the_boxes() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    sync_colliders(&mut world);

    world
        .get_mut::<Tilemap>(layer)
        .expect("a layer")
        .set(IVec2::new(5, 0), Some(Tile::new(0)));
    sync_colliders(&mut world);

    assert_eq!(generated(&world).len(), 2);
}

#[test]
fn moving_the_layer_moves_its_boxes() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    sync_colliders(&mut world);

    world
        .get_mut::<Transform>(layer)
        .expect("a layer")
        .translation = Vec2::new(0.0, 3.0);
    sync_colliders(&mut world);

    let boxes = generated(&world);
    assert_eq!(boxes.len(), 1);
    assert_eq!(boxes[0].1.translation, Vec2::new(0.5, 3.25));
}

#[test]
fn a_parented_layer_puts_its_boxes_in_world_space() {
    // Physics reads a collider's own transform as world space, so a box built
    // from the layer's local one would sit wherever the parent is not.
    let mut world = World::new();
    let parent = world.spawn();
    world.insert(parent, Transform::from_translation(Vec2::new(100.0, 0.0)));
    world.insert(parent, SceneId::new());
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    set_parent(&mut world, layer, parent).expect("no cycle");

    sync_colliders(&mut world);

    assert_eq!(generated(&world)[0].1.translation, Vec2::new(100.5, 0.25));
}

#[test]
fn a_turned_and_scaled_layer_turns_and_scales_its_boxes() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    *world.get_mut::<Transform>(layer).expect("a layer") = Transform::default()
        .with_rotation(std::f32::consts::FRAC_PI_2)
        .with_scale(Vec2::splat(2.0));

    sync_colliders(&mut world);

    let (_, transform, collider) = generated(&world)[0];
    // The centre (0.5, 0.25) scaled by two and turned a quarter: (-0.5, 1.0).
    assert!((transform.translation - Vec2::new(-0.5, 1.0)).length() < 1e-5);
    assert!((transform.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert_eq!(half(&collider), Vec2::new(0.5, 0.25));
    assert!((collider.world_half_extents(&transform) - Vec2::new(1.0, 0.5)).length() < 1e-5);
}

#[test]
fn despawning_the_layer_despawns_its_boxes() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    sync_colliders(&mut world);

    world.despawn(layer);
    sync_colliders(&mut world);

    assert!(generated(&world).is_empty());
}

#[test]
fn turning_collision_off_despawns_the_boxes() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    sync_colliders(&mut world);

    world.get_mut::<Tilemap>(layer).expect("a layer").collision = false;
    sync_colliders(&mut world);

    assert!(generated(&world).is_empty());
}

#[test]
fn erasing_every_tile_leaves_no_boxes() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    sync_colliders(&mut world);

    world.get_mut::<Tilemap>(layer).expect("a layer").clear();
    sync_colliders(&mut world);

    assert!(generated(&world).is_empty());
}

#[test]
fn the_layers_filter_and_material_are_copied_onto_its_boxes() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    let layers = CollisionLayers {
        layers: 0b10,
        mask: 0b01,
    };
    let material = PhysicsMaterial {
        friction: 0.1,
        restitution: 0.9,
    };
    world.insert(layer, layers);
    world.insert(layer, material);
    sync_colliders(&mut world);
    let (entity, _, _) = generated(&world)[0];

    assert_eq!(world.get::<CollisionLayers>(entity), Some(&layers));
    assert_eq!(world.get::<PhysicsMaterial>(entity), Some(&material));

    // And a filter changed later reaches them too.
    world.remove::<CollisionLayers>(layer);
    sync_colliders(&mut world);
    let (entity, _, _) = generated(&world)[0];
    assert_eq!(world.get::<CollisionLayers>(entity), None);
}

#[test]
fn generated_colliders_are_not_saved() {
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    world.insert(layer, SceneId::new());
    sync_colliders(&mut world);

    let file = to_scene_file(&world, &ComponentRegistry::with_defaults()).expect("serializable");

    assert_eq!(file.entities.len(), 1, "only the layer is scene content");
}

#[test]
fn a_layer_replaced_by_an_equal_copy_is_rebuilt() {
    // What an undo does: the component is replaced by a deserialized copy.
    // The copy's revision is new, so the boxes are rebuilt rather than trusted.
    let mut world = World::new();
    let layer = solid_layer(&mut world, Vec2::ZERO, &row(2));
    sync_colliders(&mut world);
    let before = generated(&world)[0].0;

    let text = ron::to_string(world.get::<Tilemap>(layer).expect("a layer")).expect("serializes");
    let copy: Tilemap = ron::from_str(&text).expect("deserializes");
    world.insert(layer, copy);
    sync_colliders(&mut world);

    let after = generated(&world);
    assert_eq!(after.len(), 1);
    assert_ne!(after[0].0, before);
}
