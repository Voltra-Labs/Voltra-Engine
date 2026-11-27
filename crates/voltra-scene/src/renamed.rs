//! The names 0.4 used, for out-of-tree code moving to 0.5.
//!
//! Each compiles with a warning naming its replacement (ARCHITECTURE,
//! "The migration: format version 2"), and goes in 0.6. Nothing in the tree
//! uses them: clippy's `-D warnings` keeps it that way. Scene files need
//! none of this — the loader renames what a version 1 file says.

/// Renamed [`Transform2D`](crate::Transform2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `Transform2D`")]
pub type Transform = crate::Transform2D;

/// Renamed [`Camera2D`](crate::Camera2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `Camera2D`")]
pub type Camera = crate::Camera2D;

/// Renamed [`Sprite2D`](crate::Sprite2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `Sprite2D`")]
pub type Sprite = crate::Sprite2D;

/// Renamed [`RigidBody2D`](crate::RigidBody2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `RigidBody2D`")]
pub type RigidBody = crate::RigidBody2D;

/// Renamed [`Collider2D`](crate::Collider2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `Collider2D`")]
pub type Collider = crate::Collider2D;

/// Renamed [`WorldTransforms2D`](crate::WorldTransforms2D) in 0.5.
#[deprecated(since = "0.5.0", note = "renamed `WorldTransforms2D`")]
pub type WorldTransforms = crate::WorldTransforms2D;
