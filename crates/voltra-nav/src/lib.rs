//! 2D navigation: where an agent can go, and the way there.
//!
//! A [`NavGrid`] is a grid of cells over part of the world, each free or
//! blocked, baked from the scene's static colliders — a solid tilemap's
//! generated boxes among them — grown by the agent's size, so a path
//! through free cells is one the agent fits along. [`NavGrid::find_path`]
//! runs A* over it and pulls the result straight where the agent can see
//! ahead; a [`Path`] hands out its waypoints as the agent reaches them.
//!
//! A grid rather than a navigation mesh: a 2D level is usually a grid
//! already — tiles — and a grid bakes from colliders with nothing but
//! overlap tests. Godot's `AStarGrid2D` and Unity's 2D grid pathfinding
//! packages make the same choice; a mesh is a stage of its own when open
//! levels need one. The grid is the game's to own and rebake: navigation
//! is a question a game asks, not something every frame pays for.

pub mod grid;
pub mod path;

pub use grid::{NavGrid, NavSettings};
pub use path::Path;
