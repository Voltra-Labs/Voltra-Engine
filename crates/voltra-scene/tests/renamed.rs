//! The 0.4 names still name the 0.5 types, on every path 0.4 code wrote
//! them, for 0.5 only: an out-of-tree game compiles, warned.
#![allow(deprecated)]

use std::any::TypeId;

use voltra_ecs::World;

fn same<A: 'static, B: 'static>() -> bool {
    TypeId::of::<A>() == TypeId::of::<B>()
}

#[test]
fn every_old_path_names_the_new_type() {
    use voltra_scene as s;
    assert!(same::<s::Transform, s::Transform2D>());
    assert!(same::<s::transform::Transform, s::Transform2D>());
    assert!(same::<s::Sprite, s::Sprite2D>());
    assert!(same::<s::sprite::Sprite, s::Sprite2D>());
    assert!(same::<s::Camera, s::Camera2D>());
    assert!(same::<s::camera::Camera, s::Camera2D>());
    assert!(same::<s::RigidBody, s::RigidBody2D>());
    assert!(same::<s::body::RigidBody, s::RigidBody2D>());
    assert!(same::<s::Collider, s::Collider2D>());
    assert!(same::<s::collider::Collider, s::Collider2D>());
    assert!(same::<s::WorldTransforms, s::WorldTransforms2D>());
    assert!(same::<s::hierarchy::WorldTransforms, s::WorldTransforms2D>());
    assert!(same::<
        s::hierarchy::world_transform::WorldTransforms,
        s::WorldTransforms2D,
    >());
    assert!(same::<voltra_render::Vertex, voltra_render::Vertex2D>());
    assert!(same::<voltra_render::mesh::Vertex, voltra_render::Vertex2D>());
    assert!(same::<voltra_render::Mesh, voltra_render::Mesh2D>());
    assert!(same::<voltra_render::mesh::Mesh, voltra_render::Mesh2D>());
}

#[test]
fn the_old_hierarchy_functions_answer_as_the_new() {
    use voltra_scene::hierarchy;
    let mut world = World::new();
    let e = world.spawn();
    let at = voltra_scene::Transform2D::from_translation(voltra_render::glam::Vec2::new(1.0, 2.0));
    world.insert(e, at);
    assert_eq!(
        hierarchy::world_matrix(&world, e),
        hierarchy::world_matrix_2d(&world, e)
    );
    assert_eq!(
        hierarchy::parent_matrix(&world, e),
        hierarchy::parent_matrix_2d(&world, e)
    );
    assert_eq!(
        hierarchy::world_transform(&world, e),
        hierarchy::world_transform_2d(&world, e)
    );
    assert!(hierarchy::set_world_transform(&mut world, e, &at));
    assert!(hierarchy::world_transform::set_world_transform(
        &mut world, e, &at
    ));
}
