//! What a `.animator.ron` file holds: parameters, states and the
//! transitions between them.

use serde::{Deserialize, Serialize};
use voltra_assets::AssetPath;

use super::tree::BlendTree;

/// A state machine: the states an [`Animator`](super::Animator) moves
/// between, each playing a clip, and the conditions on its parameters that
/// move it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StateMachine {
    /// The values a game sets, with the ones they start at.
    pub parameters: Vec<Parameter>,
    /// The state it starts in, by name; empty is the first.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub start: String,
    /// The states the machine can be in.
    pub states: Vec<State>,
    /// Tried in order; the first whose conditions hold is taken.
    pub transitions: Vec<Transition>,
}

/// A named value a game sets and the transitions read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// The name transitions and the game refer to it by.
    pub name: String,
    /// Its kind, and the value it starts at.
    pub value: ParamValue,
}

/// A parameter's value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    /// A number, tested with greater-than and less-than.
    Float(f32),
    /// A flag, tested for true or false.
    Bool(bool),
    /// Set by a game, and spent by the transition it lets through — a jump
    /// pressed, a hit taken. Unity's trigger.
    Trigger(bool),
}

impl ParamValue {
    /// Whether two values are of one kind.
    pub fn same_kind(self, other: Self) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

/// One state: a clip or a blend tree, played at a speed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// The name transitions refer to it by.
    pub name: String,
    /// The `.anim.ron` it plays — looping or not as the clip says. Written
    /// bare (`clip: Path("idle.anim.ron")`), as before trees could stand in
    /// for it, and left out for a state that plays a tree.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "super::bare")]
    pub clip: Option<AssetPath>,
    /// The clips it plays together instead, mixed by parameters — written
    /// bare too: `tree: Blend1D(parameter: "speed", children: [...])`.
    /// A state with both plays the tree.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "super::bare")]
    pub tree: Option<BlendTree>,
    /// A multiple of the clip's speed.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub speed: f32,
}

/// What a state plays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motion<'a> {
    /// One clip.
    Clip(&'a AssetPath),
    /// A blend tree with at least one clip.
    Tree(&'a BlendTree),
    /// Nothing: no clip, or an empty tree.
    Nothing,
}

impl State {
    /// What it plays: its tree when it has one — nothing when that tree
    /// is empty — else its clip.
    pub fn motion(&self) -> Motion<'_> {
        match (&self.tree, &self.clip) {
            (Some(tree), _) if tree.is_empty() => Motion::Nothing,
            (Some(tree), _) => Motion::Tree(tree),
            (None, Some(clip)) => Motion::Clip(clip),
            (None, None) => Motion::Nothing,
        }
    }
}

fn one() -> f32 {
    1.0
}

fn is_one(value: &f32) -> bool {
    *value == 1.0
}

/// A move from one state — or from any — to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    /// The state it leaves, by name; `None` leaves any state but `to`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// The state it enters, by name.
    pub to: String,
    /// All must hold. None at all, with no `exit_time`, moves at once.
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// How much of the state's clip, as a fraction of its length, must have
    /// played first. Below one, the point in each loop: `0.5` is half way
    /// through any loop. One or more counts loops on a clip that loops —
    /// `2.5` waits for two loops and a half — and is the end of one that
    /// does not. A clip of no length has nothing to wait for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_time: Option<f32>,
    /// Seconds to crossfade into `to`'s clip, the one leaving still playing
    /// meanwhile; `0`, the default, cuts. Per transition, with no machine-
    /// wide default: one place to look.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub duration: f32,
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

/// A test of one parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// A bool that is true, or a trigger that is set.
    If(String),
    /// A bool that is false, or a trigger that is not set.
    IfNot(String),
    /// A float above the value.
    Greater(String, f32),
    /// A float below the value.
    Less(String, f32),
}

impl Condition {
    /// The parameter it reads.
    pub fn parameter(&self) -> &str {
        match self {
            Self::If(name) | Self::IfNot(name) | Self::Greater(name, _) | Self::Less(name, _) => {
                name
            }
        }
    }
}

impl StateMachine {
    /// The state named `name`.
    pub fn state(&self, name: &str) -> Option<&State> {
        self.states.iter().find(|s| s.name == name)
    }

    /// The state it starts in: `start`, or the first.
    pub fn start_state(&self) -> Option<&State> {
        self.state(&self.start).or(self.states.first())
    }

    /// The parameter named `name`'s starting value.
    pub fn parameter(&self, name: &str) -> Option<ParamValue> {
        self.parameters
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.value)
    }
}
