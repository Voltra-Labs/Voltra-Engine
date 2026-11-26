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
pub mod machine;
mod progress;
mod save;
mod step;
pub mod tree;

pub use component::{Animator, AnimatorState};
pub use machine::{Condition, Motion, ParamValue, Parameter, State, StateMachine, Transition};
pub use tree::{BlendTree, Child1D, Child2D};

use voltra_ecs::{Entity, World};

use crate::keyframes::AnimationClips;
use crate::ron_store::{RonAsset, RonStore};

use step::step;

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
/// transition's `duration`.
///
/// Once per frame, before the players advance, so a clip a transition
/// starts plays from this frame. At most one transition per animator per
/// frame: two states whose conditions both hold would otherwise trade
/// places for ever inside one frame. An animator whose machine has no
/// states — or will not read — does nothing.
pub fn evaluate(world: &mut World, machines: &mut StateMachines, clips: &mut AnimationClips) {
    let animators: Vec<Entity> = world
        .query::<Animator>()
        .filter(|(_, a)| a.machine.is_some())
        .map(|(entity, _)| entity)
        .collect();
    for entity in animators {
        step(world, machines, clips, entity);
    }
}

#[cfg(test)]
mod tests;
