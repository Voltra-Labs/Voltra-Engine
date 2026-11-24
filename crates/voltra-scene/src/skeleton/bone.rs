//! One bone: how long it is and the pose it rests in.

use serde::{Deserialize, Serialize};

use crate::Transform;

/// A bone of a [`Skeleton2D`](super::Skeleton2D). Its pose is the entity's
/// own [`Transform`], relative to its parent bone; it points along its +x.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bone2D {
    /// From its origin to its tip along its +x, in its own units: where a
    /// child bone usually starts, and where IK measures a limb's reach.
    pub length: f32,
    /// The pose it is bound in, relative to its parent: what a skinned
    /// mesh's weights were painted against, and what *Reset pose* puts back.
    #[serde(default)]
    pub rest: Transform,
}

impl Default for Bone2D {
    /// Half a unit long, resting at its parent's origin.
    fn default() -> Self {
        Self {
            length: 0.5,
            rest: Transform::default(),
        }
    }
}

impl Bone2D {
    /// A bone `length` long resting in `rest`.
    pub fn new(length: f32, rest: Transform) -> Self {
        Self { length, rest }
    }
}
