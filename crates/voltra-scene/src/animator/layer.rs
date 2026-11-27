//! A machine's layer: a state machine of its own over the top-level one,
//! masked, weighted, overriding or adding.

use serde::{Deserialize, Serialize};

use super::graph::Graph;
use super::machine::{State, Transition};
use crate::keyframes::LayerBlend;

/// A state machine played over the machine's own — layer 0, the top-level
/// `states` and `transitions` — with its own current state, motion, fades
/// and clock, reading the machine's parameters. Unity's animator layer.
///
/// A file names them under `layers`, bottom up; each is evaluated over what
/// is under it: an upper-body wave masked to `"arm"`, an additive breath
/// over every state of the base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    /// What a game ([`Animator::set_layer_weight`](super::Animator::set_layer_weight))
    /// and the editor call it.
    pub name: String,
    /// How much of it shows, `0..=1` — clamped when it plays, written as
    /// given. One, the default, is not written.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub weight: f32,
    /// The targets it may write: entities' paths under the animator's
    /// entity, each with everything under it (`"arm"` is the arm and its
    /// hand). Empty, the default, is every target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mask: Vec<String>,
    /// Whether it replaces what is under it by its weight, or adds its
    /// clips' change from their first frame.
    #[serde(default, skip_serializing_if = "is_override")]
    pub blend: LayerBlend,
    /// The state it starts in, by name; empty is the first.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start: String,
    /// The states it can be in. A state with neither clip nor tree plays
    /// nothing — in a layer, the usual way to show nothing until a
    /// transition calls for something.
    #[serde(default)]
    pub states: Vec<State>,
    /// Tried in order; the first whose conditions hold is taken.
    #[serde(default)]
    pub transitions: Vec<Transition>,
}

impl Layer {
    /// Its states and transitions, as the machine's own are walked.
    pub fn graph(&self) -> Graph<'_> {
        Graph {
            start: &self.start,
            states: &self.states,
            transitions: &self.transitions,
        }
    }
}

fn one() -> f32 {
    1.0
}

fn is_one(value: &f32) -> bool {
    *value == 1.0
}

fn is_override(blend: &LayerBlend) -> bool {
    *blend == LayerBlend::Override
}
