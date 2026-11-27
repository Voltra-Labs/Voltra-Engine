//! One animator's frame: the transition its parameters call for, taken,
//! and the state's motion played.

use voltra_ecs::{Entity, World};

use super::graph::Graph;
use super::machine::{Condition, Motion, ParamValue, State, StateMachine, Transition};
use super::progress::{progress, Progress};
use super::tree::BlendTree;
use super::Animator;
use crate::keyframes::AnimationClips;
use crate::AnimationPlayer;

/// Which of an animator's layers a step moves: layer 0 — the machine's own
/// states, played by the entity's player — or one above it, played by that
/// player's layer of the same place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Slot {
    /// The machine's own states.
    Base,
    /// `layers[i]` of the machine.
    Layer(usize),
}

/// Moves one layer of the animator on `entity` along `graph` — the
/// transition its parameters call for, taken, and the state entered — or,
/// staying, has a tree state's weights follow the parameters.
pub(super) fn step(
    world: &mut World,
    machine: &StateMachine,
    graph: Graph<'_>,
    clips: &mut AnimationClips,
    entity: Entity,
    slot: Slot,
) {
    let Some(animator) = world.get::<Animator>(entity) else {
        return;
    };
    let current = current(animator, slot).and_then(|name| graph.state(name));
    let next = match current {
        // Started, restarted, given another machine, or its state is gone
        // from a reloaded one: the start state.
        None => graph.start_state().map(|s| (s, Vec::new(), 0.0)),
        Some(state) => {
            let progress = progress(player(world, entity, slot), clips, state);
            graph
                .transitions
                .iter()
                .find(|t| leaves(t, &state.name) && passes(t, machine, animator, progress))
                .and_then(|t| {
                    let to = graph.state(&t.to)?;
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
            drive(world, entity, slot, tree, input);
        }
        return;
    };
    let input = match state.motion() {
        Motion::Tree(tree) => input(tree, machine, animator),
        _ => [0.0; 2],
    };
    if let Some(animator) = world.get_mut::<Animator>(entity) {
        set_current(animator, slot, &state.name);
        for name in spent {
            animator.reset_trigger(&name);
        }
    }
    if slot == Slot::Base && world.get::<AnimationPlayer>(entity).is_none() {
        world.insert(entity, AnimationPlayer::default());
    }
    if let Some(player) = player_mut(world, entity, slot) {
        enter(player, state, input, duration);
    }
}

/// The state `slot` of `animator` is in, by name.
fn current(animator: &Animator, slot: Slot) -> Option<&str> {
    match slot {
        Slot::Base => animator.state.current.as_deref(),
        Slot::Layer(i) => animator.state.layers.get(i)?.current.as_deref(),
    }
}

fn set_current(animator: &mut Animator, slot: Slot, name: &str) {
    let current = match slot {
        Slot::Base => &mut animator.state.current,
        Slot::Layer(i) => match animator.state.layers.get_mut(i) {
            Some(layer) => &mut layer.current,
            None => return,
        },
    };
    *current = Some(name.to_owned());
}

/// What plays `slot`: the entity's player, or its layer of that place.
fn player(world: &World, entity: Entity, slot: Slot) -> Option<&AnimationPlayer> {
    let player = world.get::<AnimationPlayer>(entity)?;
    match slot {
        Slot::Base => Some(player),
        Slot::Layer(i) => player.layers().get(i).map(|l| &l.player),
    }
}

fn player_mut(world: &mut World, entity: Entity, slot: Slot) -> Option<&mut AnimationPlayer> {
    let player = world.get_mut::<AnimationPlayer>(entity)?;
    match slot {
        Slot::Base => Some(player),
        Slot::Layer(i) => player.layers_mut().get_mut(i).map(|l| &mut l.player),
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
fn drive(world: &mut World, entity: Entity, slot: Slot, tree: &BlendTree, input: [f32; 2]) {
    let Some(set) = player_mut(world, entity, slot).and_then(AnimationPlayer::synced_mut) else {
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
