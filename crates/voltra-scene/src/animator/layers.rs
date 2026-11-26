//! Keeping an animator's layers and its player's in step with its machine's:
//! one of each per layer, by place, with the layer's weight, mask and blend.

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use super::component::LayerState;
use super::machine::StateMachine;
use super::Animator;
use crate::keyframes::PlayerLayer;
use crate::AnimationPlayer;

/// Starts the animator on `entity` over if it was running another machine,
/// or none: every layer from its start state.
pub(super) fn begin(world: &mut World, entity: Entity, path: &AssetPath) {
    let Some(animator) = world.get_mut::<Animator>(entity) else {
        return;
    };
    if animator.state.machine.as_ref() != Some(path) {
        animator.state.current = None;
        animator.state.layers.clear();
        animator.state.machine = Some(path.clone());
    }
}

/// Gives the animator on `entity`, and its player, one slot per layer of
/// `machine` — a layer whose name moved (a reload reordered them) starts
/// over, one the machine lost is taken out, handing back what it wrote —
/// and each player layer the machine's weight (or the one the game set),
/// mask and blend. Answers whether there are layers to step.
pub(super) fn sync(world: &mut World, entity: Entity, machine: &StateMachine) -> bool {
    let Some(animator) = world.get_mut::<Animator>(entity) else {
        return false;
    };
    let had = animator.state.layers.len();
    if machine.layers.is_empty() && had == 0 {
        // A machine of one layer leaves the player's layers to the game.
        return false;
    }
    let mut moved = Vec::new();
    animator.state.layers.truncate(machine.layers.len());
    for (i, layer) in machine.layers.iter().enumerate() {
        match animator.state.layers.get_mut(i) {
            Some(slot) if slot.name == layer.name => {}
            Some(slot) => {
                *slot = LayerState {
                    name: layer.name.clone(),
                    current: None,
                };
                moved.push(i);
            }
            None => animator.state.layers.push(LayerState {
                name: layer.name.clone(),
                current: None,
            }),
        }
    }
    let weights: Vec<f32> = machine
        .layers
        .iter()
        .map(|l| animator.layer_weight(&l.name).unwrap_or(l.weight))
        .collect();
    if world.get::<AnimationPlayer>(entity).is_none() {
        world.insert(entity, AnimationPlayer::default());
    }
    let Some(player) = world.get_mut::<AnimationPlayer>(entity) else {
        return false;
    };
    let slots = player.layers_mut();
    slots.truncate(machine.layers.len());
    slots.resize_with(machine.layers.len(), PlayerLayer::default);
    for i in moved {
        slots[i] = PlayerLayer::default();
    }
    for ((slot, layer), weight) in slots.iter_mut().zip(&machine.layers).zip(weights) {
        slot.weight = weight;
        slot.blend = layer.blend;
        if slot.mask != layer.mask {
            slot.mask.clone_from(&layer.mask);
        }
    }
    !machine.layers.is_empty()
}
