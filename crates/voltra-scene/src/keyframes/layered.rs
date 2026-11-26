//! A frame of a player with layers: its own motion sampled as always, each
//! layer's over it, bottom up, and the result written once.

use std::borrow::Cow;

use voltra_ecs::{Entity, World};

use super::composite::composite_into;
use super::frame::{held, step as step_one, Frame};
use super::layer::LayerBlend;
use super::player::AnimationPlayer;
use super::pose::{Pose, Sample};
use super::property::Property;
use super::report::report;
use super::sample::{keep, sample_frame, Buffers as Shared};
use super::store::AnimationClips;
use super::synced::Pick;
use super::target::Children;

/// What one layered player writes this frame: its own motion's frame, and
/// each layer's, in order.
pub(super) struct Frames {
    base: Option<Frame>,
    layers: Vec<Option<Frame>>,
}

/// Runs the clocks of `player`'s motion and of each layer's. A layer that is
/// paused or has ended holds its pose, rather than vanishing from over the
/// motion as an ended player's values are left alone.
pub(super) fn step(
    player: &mut AnimationPlayer,
    clips: &mut AnimationClips,
    delta: f32,
    picks: &mut Vec<Pick>,
) -> Frames {
    let base = step_one(player, clips, delta, picks);
    let layers = player
        .layers
        .iter_mut()
        .map(|layer| {
            let player = &mut layer.player;
            step_one(player, clips, delta, picks).or_else(|| held(player, clips, picks))
        })
        .collect();
    Frames { base, layers }
}

/// The poses a frame of layered players samples into, beside the shared
/// ones, reused player after player.
#[derive(Default)]
pub(super) struct Buffers<'c> {
    /// The player's own motion, alone.
    base: Pose<'c>,
    /// What the motion fades into, kept to tell whose a missed value is.
    base_incoming: Pose<'c>,
    /// What the frame writes, built up layer by layer.
    acc: Pose<'c>,
    /// The next layer's result, swapped with `acc`.
    next: Pose<'c>,
    /// Every property a layer writes, once each, in order.
    keys: Vec<(Cow<'c, str>, Property)>,
    layers: Vec<Slot<'c>>,
}

/// One layer's frame, sampled and masked.
#[derive(Default)]
struct Slot<'c> {
    pose: Pose<'c>,
    /// What it fades into, kept to tell whose a missed value is.
    incoming: Pose<'c>,
    fading: bool,
    weight: f32,
    blend: LayerBlend,
}

/// Samples `frames`, composites each layer over the player's own motion —
/// over, for a property the motion does not animate, what is under the
/// layers ([`Underneath`](super::underneath::Underneath)) — writes the
/// result once, and warns of what found nothing to write to, under the
/// motion or layer it is from.
#[allow(clippy::too_many_arguments)]
pub(super) fn write<'c>(
    world: &mut World,
    clips: &'c AnimationClips,
    children: &Children,
    picks: &[Pick],
    shared: &mut Shared<'c>,
    buffers: &mut Buffers<'c>,
    entity: Entity,
    frames: Frames,
) {
    let Buffers {
        base,
        base_incoming,
        acc,
        next,
        keys,
        layers,
    } = buffers;
    let Some(player) = world.get_mut::<AnimationPlayer>(entity) else {
        return;
    };
    let mut motion = None;
    base.clear();
    if let Some(frame) = frames.base {
        let mut sampled = sample_frame(clips, picks, shared, frame, false);
        keep(player, &mut sampled, &shared.mixed);
        base.clone_from(shared.result(&sampled));
        if sampled.fading {
            base_incoming.clone_from(&shared.incoming);
        }
        motion = Some(sampled.fading);
    }
    if layers.len() < frames.layers.len() {
        layers.resize_with(frames.layers.len(), Slot::default);
    }
    for ((frame, slot), layer) in frames
        .layers
        .into_iter()
        .zip(layers.iter_mut())
        .zip(&mut player.layers)
    {
        slot.pose.clear();
        slot.fading = false;
        slot.weight = layer.effective_weight();
        slot.blend = layer.blend;
        let Some(frame) = frame else {
            continue;
        };
        let additive = layer.blend == LayerBlend::Additive;
        let mut sampled = sample_frame(clips, picks, shared, frame, additive);
        keep(&mut layer.player, &mut sampled, &shared.mixed);
        slot.pose.clone_from(shared.result(&sampled));
        slot.pose.retain(|s| layer.covers(&s.target));
        slot.fading = sampled.fading;
        if sampled.fading {
            slot.incoming.clone_from(&shared.incoming);
        }
    }
    let count = player.layers.len();
    let mut underneath = std::mem::take(&mut player.underneath);
    let layers = &layers[..count];

    // Every property a layer that weighs anything writes, once, in order.
    keys.clear();
    keys.extend(
        layers
            .iter()
            .filter(|slot| slot.weight > 0.0)
            .flat_map(|slot| {
                slot.pose
                    .samples()
                    .iter()
                    .map(|s| (s.target.clone(), s.property))
            }),
    );
    keys.sort_unstable();
    keys.dedup();
    acc.clone_from(base);
    underneath.fill(world, entity, children, keys, acc);
    for slot in layers.iter().filter(|slot| slot.weight > 0.0) {
        composite_into(acc, &slot.pose, slot.weight, slot.blend, next);
        std::mem::swap(acc, next);
    }
    let missed = acc.apply_through(world, entity, children);
    underneath.settle(world, entity, children);

    let Some(player) = world.get_mut::<AnimationPlayer>(entity) else {
        return;
    };
    player.underneath = underneath;
    if missed.is_empty() {
        return;
    }
    let holds = |pose: &Pose, s: &Sample| pose.get(&s.target, s.property).is_some();
    if let Some(fading) = motion {
        let its: Vec<&Sample> = missed.iter().copied().filter(|s| holds(base, s)).collect();
        report(player, fading, base_incoming, &its);
    }
    for (slot, layer) in layers.iter().zip(&mut player.layers) {
        if slot.weight <= 0.0 {
            continue;
        }
        let its: Vec<&Sample> = missed
            .iter()
            .copied()
            .filter(|s| holds(&slot.pose, s))
            .collect();
        if !its.is_empty() {
            report(&mut layer.player, slot.fading, &slot.incoming, &its);
        }
    }
}
