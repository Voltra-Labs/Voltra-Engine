//! State machines: which clip an entity plays, chosen by parameters a game
//! sets.
//!
//! A [`StateMachine`] (a `*.animator.ron` file) holds parameters — floats,
//! bools, triggers — states, each playing an animation clip, and
//! transitions between them, each a list of conditions on the parameters
//! and an optional exit time. An [`Animator`] runs one on its entity: a
//! game writes `speed` and `grounded` and triggers `jump`, and the animator
//! moves to the state they call for and has the entity's
//! [`AnimationPlayer`] play its clip — cutting to it, or crossfading over
//! the transition's `duration`, which the player mixes. Unity's `Animator`
//! and `AnimatorController`; Godot's `AnimationTree` state machine.

mod check;
pub mod component;
pub mod machine;
mod progress;
mod save;

pub use component::{Animator, AnimatorState};
pub use machine::{Condition, ParamValue, Parameter, State, StateMachine, Transition};

use voltra_ecs::{Entity, World};

use crate::keyframes::AnimationClips;
use crate::ron_store::{RonAsset, RonStore};
use crate::AnimationPlayer;

use progress::{progress, Progress};

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

fn step(
    world: &mut World,
    machines: &mut StateMachines,
    clips: &mut AnimationClips,
    entity: Entity,
) {
    let Some(animator) = world.get::<Animator>(entity) else {
        return;
    };
    let Some(path) = animator.machine.clone() else {
        return;
    };
    let handle = machines.load(&path);
    let machine = machines.get(handle);
    let current = animator
        .state
        .current
        .as_deref()
        .filter(|_| animator.state.machine.as_ref() == Some(&path))
        .and_then(|name| machine.state(name));
    let next = match current {
        // Started, restarted, given another machine, or its state is gone
        // from a reloaded one: the start state.
        None => machine.start_state().map(|s| (s.clone(), Vec::new(), 0.0)),
        Some(state) => {
            let progress = progress(world, clips, entity, state);
            machine
                .transitions
                .iter()
                .find(|t| leaves(t, &state.name) && passes(t, machine, animator, progress))
                .and_then(|t| {
                    let to = machine.state(&t.to)?.clone();
                    let spent: Vec<String> = t
                        .conditions
                        .iter()
                        .map(|c| c.parameter().to_owned())
                        .filter(|name| {
                            matches!(machine.parameter(name), Some(ParamValue::Trigger(_)))
                        })
                        .collect();
                    Some((to, spent, t.duration))
                })
        }
    };
    let Some((state, spent, duration)) = next else {
        return;
    };
    if let Some(animator) = world.get_mut::<Animator>(entity) {
        animator.state.current = Some(state.name.clone());
        animator.state.machine = Some(path);
        for name in spent {
            animator.reset_trigger(&name);
        }
    }
    match world.get_mut::<AnimationPlayer>(entity) {
        // The player mixes; the animator only asks for a fade.
        Some(player) => {
            player.cross_fade(state.clip.clone(), duration);
            player.speed = state.speed;
        }
        None => {
            let mut player = AnimationPlayer::new(state.clip.clone());
            player.speed = state.speed;
            world.insert(entity, player);
        }
    }
}

/// Whether `transition` leaves the state named `current`: from it, or from
/// any state but the one it goes to.
fn leaves(transition: &Transition, current: &str) -> bool {
    match &transition.from {
        Some(from) => from == current,
        None => transition.to != current,
    }
}

fn passes(
    transition: &Transition,
    machine: &StateMachine,
    animator: &Animator,
    progress: Progress,
) -> bool {
    if transition.exit_time.is_some_and(|at| !progress.reached(at)) {
        return false;
    }
    transition.conditions.iter().all(|condition| {
        let name = condition.parameter();
        let value = animator
            .parameter(name)
            .filter(|v| machine.parameter(name).is_some_and(|m| m.same_kind(*v)))
            .or(machine.parameter(name));
        match (condition, value) {
            (Condition::If(_), Some(ParamValue::Bool(b) | ParamValue::Trigger(b))) => b,
            (Condition::IfNot(_), Some(ParamValue::Bool(b) | ParamValue::Trigger(b))) => !b,
            (Condition::Greater(_, at), Some(ParamValue::Float(v))) => v > *at,
            (Condition::Less(_, at), Some(ParamValue::Float(v))) => v < *at,
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests;
