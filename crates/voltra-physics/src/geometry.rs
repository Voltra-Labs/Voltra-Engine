//! The convex geometry under the narrow phase and the queries: a placed
//! shape with its edge normals, the closest points of two segments, and
//! the distance between two convex shapes.
//!
//! Box2D v3's shape model — every shape a convex hull grown by a radius —
//! so each answer here is written once for all of them.

mod distance;
mod hull;
mod segment;

pub(crate) use distance::closest;
pub(crate) use hull::{nearest_on_segment, Hull};
pub(crate) use segment::segment_distance;
