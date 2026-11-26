//! State machines: which clip an entity plays, chosen by parameters a game
//! sets.
//!
//! A [`StateMachine`] (a `*.animator.ron` file) holds parameters — floats,
//! bools, triggers — states, each playing an animation clip or a
//! [`BlendTree`] of clips weighted by float parameters, and
//! transitions between them, each a list of conditions on the parameters
//! and an optional exit time. An [`Animator`] runs one on its entity: a
//! game writes `speed` and `grounded` and triggers `jump`, and the animator
//! moves to the state they call for and has the entity's
//! [`AnimationPlayer`] play its clip — or a tree's clips, synced, at the
//! weights the parameters give, every frame — cutting to it, or
//! crossfading over the transition's `duration`, which the player mixes.
//! Unity's `Animator` and `AnimatorController`; Godot's `AnimationTree`
//! state machine.

mod bare;
mod check;
pub mod component;
pub mod graph;
pub mod layer;
mod layers;
pub mod machine;
mod progress;
mod save;
mod step;
pub mod tree;

pub use crate::keyframes::LayerBlend;
pub use component::{Animator, AnimatorState};
pub use graph::Graph;
pub use layer::Layer;
pub use machine::{Condition, Motion, ParamValue, Parameter, State, StateMachine, Transition};
pub use tree::{BlendTree, Child1D, Child2D};

use voltra_ecs::{Entity, World};

use crate::keyframes::AnimationClips;
use crate::ron_store::{RonAsset, RonStore};

use step::{step, Slot};

/// Reads state machines from an asset root — a [`RonStore`].
pub type StateMachines = RonStore<StateMachine>;

impl RonAsset for StateMachine {
    const KIND: &'static str = "state machine";

    fn checked(self, source: &str) -> Self {
        StateMachine::checked(self, source)
    }
}

/// Moves every animator to the state its parameters call for, and starts
/// that state's clip — cutting to it, or fading into it over the
/// transition's `duration` — and so each of its machine's layers, played
/// by a layer of the entity's player at the layer's weight, mask and blend.
///
/// Once per frame, before the players advance, so a clip a transition
/// starts plays from this frame. At most one transition per animator per
/// frame and layer: two states whose conditions both hold would otherwise
/// trade places for ever inside one frame. Layer 0 first, then the layers
/// bottom up, so a trigger layer 0 spends is spent for the layers too. An
/// animator whose machine has no states — or will not read — does nothing.
pub fn evaluate(world: &mut World, machines: &mut StateMachines, clips: &mut AnimationClips) {
    let animators: Vec<Entity> = world
        .query::<Animator>()
        .filter(|(_, a)| a.machine.is_some())
        .map(|(entity, _)| entity)
        .collect();
    for entity in animators {
        run(world, machines, clips, entity);
    }
}

fn run(
    world: &mut World,
    machines: &mut StateMachines,
    clips: &mut AnimationClips,
    entity: Entity,
) {
    let Some(path) = world
        .get::<Animator>(entity)
        .and_then(|a| a.machine.clone())
    else {
        return;
    };
    let handle = machines.load(&path);
    let machine = machines.get(handle);
    layers::begin(world, entity, &path);
    step(world, machine, machine.graph(), clips, entity, Slot::Base);
    if !layers::sync(world, entity, machine) {
        return;
    }
    for (i, layer) in machine.layers.iter().enumerate() {
        step(world, machine, layer.graph(), clips, entity, Slot::Layer(i));
    }
}

#[cfg(test)]
mod tests;
