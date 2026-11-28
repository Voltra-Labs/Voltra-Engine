//! The names 0.4 used, for out-of-tree code moving to 0.5.
//!
//! Each compiles with a warning naming its replacement (ARCHITECTURE,
//! "The migration: format version 2"), and goes in 0.6. Nothing in the tree
//! uses them: clippy's `-D warnings` keeps it that way. Scene files need
//! none of this — the loader renames what a version 1 file says.

/// Renamed [`Transform2D`](crate::Transform2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `Transform2D`; removed in 0.6")]
pub type Transform = crate::Transform2D;

/// Renamed [`Camera2D`](crate::Camera2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `Camera2D`; removed in 0.6")]
pub type Camera = crate::Camera2D;

/// Renamed [`Sprite2D`](crate::Sprite2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `Sprite2D`; removed in 0.6")]
pub type Sprite = crate::Sprite2D;

/// Renamed [`RigidBody2D`](crate::RigidBody2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `RigidBody2D`; removed in 0.6")]
pub type RigidBody = crate::RigidBody2D;

/// Renamed [`Collider2D`](crate::Collider2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `Collider2D`; removed in 0.6")]
pub type Collider = crate::Collider2D;

/// Renamed [`WorldTransforms2D`](crate::WorldTransforms2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `WorldTransforms2D`; removed in 0.6")]
pub type WorldTransforms = crate::WorldTransforms2D;

/// Renamed [`world_matrix_2d`](crate::hierarchy::world_matrix_2d) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `world_matrix_2d`; removed in 0.6")]
pub fn world_matrix(
    world: &voltra_ecs::World,
    entity: voltra_ecs::Entity,
) -> voltra_render::glam::Mat3 {
    crate::hierarchy::world_matrix_2d(world, entity)
}

/// Renamed [`parent_matrix_2d`](crate::hierarchy::parent_matrix_2d) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `parent_matrix_2d`; removed in 0.6")]
pub fn parent_matrix(
    world: &voltra_ecs::World,
    entity: voltra_ecs::Entity,
) -> voltra_render::glam::Mat3 {
    crate::hierarchy::parent_matrix_2d(world, entity)
}

/// Renamed [`world_transform_2d`](crate::hierarchy::world_transform_2d) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `world_transform_2d`; removed in 0.6")]
pub fn world_transform(
    world: &voltra_ecs::World,
    entity: voltra_ecs::Entity,
) -> crate::Transform2D {
    crate::hierarchy::world_transform_2d(world, entity)
}

/// Renamed [`set_world_transform_2d`](crate::hierarchy::set_world_transform_2d)
/// in 0.5.
#[deprecated(
    since = "0.5.0",
    note = "renamed `set_world_transform_2d`; removed in 0.6"
)]
pub fn set_world_transform(
    world: &mut voltra_ecs::World,
    entity: voltra_ecs::Entity,
    target: &crate::Transform2D,
) -> bool {
    crate::hierarchy::set_world_transform_2d(world, entity, target)
}
