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
    /// Each of the machine's layers above the first, by place: its name, and
    /// the state it is in. A layer whose name no longer matches its place
    /// (a reload that moved it) starts over.
    pub(super) layers: Vec<LayerState>,
    /// Layer weights the game set, by layer name, over the file's.
    pub(super) weights: HashMap<String, f32>,
}

/// Where one layer of an animator is.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct LayerState {
    /// The layer's name.
    pub name: String,
    /// The state it is in, once started.
    pub current: Option<String>,
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

    /// The state it is in, once it has started — the state of layer 0, the
    /// machine's own states.
    pub fn current_state(&self) -> Option<&str> {
        self.state.current.as_deref()
    }

    /// The state the layer named `layer` is in, once it has started.
    pub fn layer_state(&self, layer: &str) -> Option<&str> {
        self.state
            .layers
            .iter()
            .find(|l| l.name == layer)
            .and_then(|l| l.current.as_deref())
    }

    /// Sets the weight of the layer named `layer`, over the one its file
    /// gives: `0` hides it, `1` shows it whole — Unity's `SetLayerWeight`.
    /// A value that is not finite is ignored; one outside `0..=1` is
    /// clamped when it plays.
    pub fn set_layer_weight(&mut self, layer: &str, weight: f32) {
        if weight.is_finite() {
            self.state.weights.insert(layer.to_owned(), weight);
        }
    }

    /// The weight the game set for the layer named `layer`, if it set one.
    pub fn layer_weight(&self, layer: &str) -> Option<f32> {
        self.state.weights.get(layer).copied()
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

    /// Starts over from the machine's start state, every layer from its
    /// own, with its parameters' starting values and its layers' weights.
    pub fn restart(&mut self) {
        self.state = AnimatorState::default();
    }
}
