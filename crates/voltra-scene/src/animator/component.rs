//! The component that runs a state machine on its entity.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use voltra_assets::AssetPath;

use super::machine::ParamValue;

/// Moves its entity between the states of [`machine`](Self::machine) as a
/// game sets its parameters, and plays each state's clip on the entity's
/// [`AnimationPlayer`](crate::AnimationPlayer) — one is added if it has
/// none.
///
/// Unity's `Animator` over an `AnimatorController`, Godot's
/// `AnimationTree` state machine: the graph is a file shared by every
/// entity that runs it, and what each entity is doing — its state, its
/// parameters — is its own. Logic only: playing a clip stays the player's.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Animator {
    /// The `.animator.ron` it runs; `None` runs nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<AssetPath>,
    /// Where it is and what it was told. Never saved.
    #[serde(skip)]
    pub state: AnimatorState,
}

/// An animator's state and parameters.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimatorState {
    /// The state it is in, by name — a name rather than an index, so a
    /// reloaded machine that moved its states keeps it.
    pub(super) current: Option<String>,
    /// The machine `current` belongs to.
    pub(super) machine: Option<AssetPath>,
    /// What the game set; a parameter it never set has the machine's
    /// starting value.
    pub(super) params: HashMap<String, ParamValue>,
}

impl Animator {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Animator";

    /// Running `machine`.
    pub fn new(machine: AssetPath) -> Self {
        Self {
            machine: Some(machine),
            ..Self::default()
        }
    }

    /// The state it is in, once it has started.
    pub fn current_state(&self) -> Option<&str> {
        self.state.current.as_deref()
    }

    /// Sets a float parameter. A value that is not finite is ignored.
    pub fn set_float(&mut self, name: &str, value: f32) {
        if value.is_finite() {
            self.state
                .params
                .insert(name.to_owned(), ParamValue::Float(value));
        }
    }

    /// Sets a bool parameter.
    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.state
            .params
            .insert(name.to_owned(), ParamValue::Bool(value));
    }

    /// Sets a trigger: it stays set until a transition it lets through
    /// spends it.
    pub fn trigger(&mut self, name: &str) {
        self.state
            .params
            .insert(name.to_owned(), ParamValue::Trigger(true));
    }

    /// Unsets a trigger nothing spent.
    pub fn reset_trigger(&mut self, name: &str) {
        self.state
            .params
            .insert(name.to_owned(), ParamValue::Trigger(false));
    }

    /// What the game set `name` to, if it set it.
    pub fn parameter(&self, name: &str) -> Option<ParamValue> {
        self.state.params.get(name).copied()
    }

    /// Starts over from the machine's start state, with its parameters'
    /// starting values.
    pub fn restart(&mut self) {
        self.state = AnimatorState::default();
    }
}
