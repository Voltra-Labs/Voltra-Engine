//! One animator's frame: the transition its parameters call for, taken,
//! and the state's motion played.

use voltra_ecs::{Entity, World};

use super::machine::{Condition, Motion, ParamValue, State, StateMachine, Transition};
use super::progress::{progress, Progress};
use super::tree::BlendTree;
use super::{Animator, StateMachines};
use crate::keyframes::AnimationClips;
use crate::AnimationPlayer;

pub(super) fn step(
    world: &mut World,
    machines: &mut StateMachines,
    clips: &mut AnimationClips,
    entity: Entity,
) {
    let Some(animator) = world.get::<Animator>(entity) else {
        return;
    };
    let Some(path) = animator.machine.as_ref() else {
        return;
    };
    let handle = machines.load(path);
    let machine = machines.get(handle);
    let current = animator
        .state
        .current
        .as_deref()
        .filter(|_| animator.state.machine.as_ref() == Some(path))
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
        // Staying: a tree's weights follow its parameters every frame.
        if let Some(Motion::Tree(tree)) = current.map(State::motion) {
            let input = input(tree, machine, animator);
            drive(world, entity, tree, input);
        }
        return;
    };
    let input = match state.motion() {
        Motion::Tree(tree) => input(tree, machine, animator),
        _ => [0.0; 2],
    };
    let path = path.clone();
    if let Some(animator) = world.get_mut::<Animator>(entity) {
        animator.state.current = Some(state.name.clone());
        animator.state.machine = Some(path);
        for name in spent {
            animator.reset_trigger(&name);
        }
    }
    if world.get::<AnimationPlayer>(entity).is_none() {
        world.insert(entity, AnimationPlayer::default());
    }
    if let Some(player) = world.get_mut::<AnimationPlayer>(entity) {
        enter(player, &state, input, duration);
    }
}

/// Has `player` play `state`'s motion — cutting to it, or fading into it
/// over `duration` seconds. The player mixes; the animator only asks.
fn enter(player: &mut AnimationPlayer, state: &State, input: [f32; 2], duration: f32) {
    match state.motion() {
        Motion::Clip(clip) => player.cross_fade(clip.clone(), duration),
        Motion::Tree(tree) => player.cross_fade_synced(tree.synced(input), duration),
        Motion::Nothing => player.stop(),
    }
    player.speed = state.speed;
}

/// Weighs the player's synced clips for `input` as `tree` places them —
/// first giving it the tree's clips, keeping its phase, if a reload or an
/// edit changed them. A player told to play a clip of its own since is
/// left to it, as in a clip state.
fn drive(world: &mut World, entity: Entity, tree: &BlendTree, input: [f32; 2]) {
    let Some(set) = world
        .get_mut::<AnimationPlayer>(entity)
        .and_then(AnimationPlayer::synced_mut)
    else {
        return;
    };
    if set.clips().len() != tree.len() || !set.clips().iter().eq(tree.clips()) {
        set.replace(tree.clips().cloned().zip(std::iter::repeat(0.0)));
    }
    tree.weights(input, set.weights_mut());
}

/// The values of `tree`'s parameters: what the game set, else the
/// machine's starting value; zero for one that is not a float.
fn input(tree: &BlendTree, machine: &StateMachine, animator: &Animator) -> [f32; 2] {
    let float = |name: &str| match value(machine, animator, name) {
        Some(ParamValue::Float(v)) => v,
        _ => 0.0,
    };
    match tree {
        BlendTree::Blend1D { parameter, .. } => [float(parameter), 0.0],
        BlendTree::Blend2D { x, y, .. } => [float(x), float(y)],
    }
}

/// What `name` reads: the game's value when it is of the machine's kind,
/// else the machine's starting one.
fn value(machine: &StateMachine, animator: &Animator, name: &str) -> Option<ParamValue> {
    animator
        .parameter(name)
        .filter(|v| machine.parameter(name).is_some_and(|m| m.same_kind(*v)))
        .or(machine.parameter(name))
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
        match (condition, value(machine, animator, condition.parameter())) {
            (Condition::If(_), Some(ParamValue::Bool(b) | ParamValue::Trigger(b))) => b,
            (Condition::IfNot(_), Some(ParamValue::Bool(b) | ParamValue::Trigger(b))) => !b,
            (Condition::Greater(_, at), Some(ParamValue::Float(v))) => v > *at,
            (Condition::Less(_, at), Some(ParamValue::Float(v))) => v < *at,
            _ => false,
        }
    })
}
