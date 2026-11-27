//! A state's blend tree: clips placed along one parameter or over two, the
//! input's place among them choosing each clip's weight.

mod line;
mod plane;

use serde::{Deserialize, Serialize};
use voltra_assets::AssetPath;

use crate::keyframes::SyncedClips;

/// Clips a state plays together, mixed by where its parameters put the
/// input among them, and synced so they keep one phase.
///
/// Unity's 1D and 2D simple directional blend trees, Godot's
/// `BlendSpace1D` / `BlendSpace2D`: idle, walk and run along `speed`; four
/// walks and an idle over a stick's `x` and `y`. The parameters are the
/// machine's floats — the same values its transitions test — read every
/// frame, so the weights follow the game without a transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlendTree {
    /// Along one float: between the two children whose thresholds
    /// bracket it, linearly; past either end, the end child.
    Blend1D {
        /// The float parameter it reads.
        parameter: String,
        /// The clips, each at a threshold — in any order.
        children: Vec<Child1D>,
    },
    /// Over two floats: the triangle of children around the point, by
    /// barycentric weights; outside them all, the nearest edge's two.
    Blend2D {
        /// The float parameter for the horizontal axis.
        x: String,
        /// The float parameter for the vertical axis.
        y: String,
        /// The clips, each at a point.
        children: Vec<Child2D>,
    },
}

/// A clip of a 1D tree, at the value of the parameter where it alone plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Child1D {
    /// Where along the parameter it alone plays.
    pub threshold: f32,
    /// The `.anim.ron` it plays.
    pub clip: AssetPath,
}

/// A clip of a 2D tree, at the point where it alone plays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Child2D {
    /// `(x, y)`: where it alone plays.
    pub position: [f32; 2],
    /// The `.anim.ron` it plays.
    pub clip: AssetPath,
}

impl BlendTree {
    /// The float parameters it reads: one, or `x` then `y`.
    pub fn parameters(&self) -> Vec<&str> {
        match self {
            Self::Blend1D { parameter, .. } => vec![parameter],
            Self::Blend2D { x, y, .. } => vec![x, y],
        }
    }

    /// How many clips it has.
    pub fn len(&self) -> usize {
        match self {
            Self::Blend1D { children, .. } => children.len(),
            Self::Blend2D { children, .. } => children.len(),
        }
    }

    /// Whether it has no clips: a state of it plays nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Its clips, in order.
    pub fn clips(&self) -> impl Iterator<Item = &AssetPath> {
        let (one, two) = match self {
            Self::Blend1D { children, .. } => (children.as_slice(), &[][..]),
            Self::Blend2D { children, .. } => (&[][..], children.as_slice()),
        };
        one.iter()
            .map(|c| &c.clip)
            .chain(two.iter().map(|c| &c.clip))
    }

    /// Each clip's weight for `input` — the parameters' values, `x` then
    /// `y` (the second unread in 1D) — into `out`, one per clip in order.
    /// The weights sum to one, or are all zero when no child sits at a
    /// finite place; never a NaN.
    pub fn weights(&self, input: [f32; 2], out: &mut [f32]) {
        match self {
            Self::Blend1D { children, .. } => {
                line::weights(children.len(), |i| children[i].threshold, input[0], out)
            }
            Self::Blend2D { children, .. } => {
                plane::weights(children.len(), |i| children[i].position, input, out)
            }
        }
    }

    /// Its clips, weighed for `input`, ready for a player to play together.
    pub fn synced(&self, input: [f32; 2]) -> SyncedClips {
        let mut weights = vec![0.0; self.len()];
        self.weights(input, &mut weights);
        SyncedClips::new(self.clips().cloned().zip(weights))
    }
}
