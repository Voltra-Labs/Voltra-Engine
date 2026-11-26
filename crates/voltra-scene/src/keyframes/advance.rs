//! A frame of every player: clocks run, poses sampled, mixed and written.

use voltra_ecs::{Entity, World};

use super::blend::blend_into;
use super::fade::Outgoing;
use super::frame::{step, Frame, Motion, Source};
use super::player::AnimationPlayer;
use super::pose::{Pose, Sample};
use super::store::AnimationClips;
use super::synced::Pick;
use super::target::Children;

/// Runs every playing [`AnimationPlayer`] `delta` seconds and writes its
/// clip's values — or its synced clips' mix, or, while it fades, the mix of
/// what it plays and what it fades out — each property once.
///
/// Per frame, as `SpriteAnimation` is: what is animated is what is drawn.
/// A player applies nothing while it is not playing, so an entity whose clip
/// has ended — or was paused — can be moved by other code without the clip
/// putting it back. A dynamic body's transform is the simulation's: the
/// step after this overwrites it, so animate a kinematic body, or a child.
///
/// A frame samples into a few buffers reused across every player, and finds
/// targets through one index of the hierarchy, so a crowd of players
/// blending several clips each allocates little and scans the world once.
pub fn advance(world: &mut World, clips: &mut AnimationClips, delta: f32) {
    let mut due = Vec::new();
    let mut picks = Vec::new();
    for (entity, player) in world.query_mut::<AnimationPlayer>() {
        if let Some(frame) = step(player, clips, delta, &mut picks) {
            due.push((entity, frame));
        }
    }
    if due.is_empty() {
        return;
    }
    // Writing values moves no parent link: one index serves the frame.
    let children = Children::of(world);
    let clips = &*clips;
    let mut buffers = Buffers::default();
    for (entity, frame) in due {
        write(world, clips, &children, &picks, &mut buffers, entity, frame);
    }
}

/// The poses a frame samples and mixes into, reused player after player.
#[derive(Default)]
struct Buffers<'c> {
    incoming: Pose<'c>,
    outgoing: Pose<'c>,
    mixed: Pose<'c>,
    synced: Vec<Pose<'c>>,
}

/// Samples, mixes and writes one player's frame, and warns of what it
/// found nothing to write to.
fn write<'c>(
    world: &mut World,
    clips: &'c AnimationClips,
    children: &Children,
    picks: &[Pick],
    buffers: &mut Buffers<'c>,
    entity: Entity,
    frame: Frame,
) {
    let Buffers {
        incoming,
        outgoing,
        mixed,
        synced,
    } = buffers;
    match &frame.motion {
        Some(motion) => sample(clips, motion, picks, synced, incoming),
        None => incoming.clear(),
    }
    let mut held = None;
    let fading = match frame.fade {
        None => false,
        Some((from, weight)) => {
            match from {
                Source::Motion(motion) => sample(clips, &motion, picks, synced, outgoing),
                Source::Pose(pose) => held = Some(pose),
            }
            let from: &Pose<'c> = match &held {
                Some(pose) => pose,
                None => outgoing,
            };
            blend_into(&[(from, 1.0 - weight), (incoming, weight)], mixed);
            true
        }
    };
    let pose: &Pose<'c> = if fading { mixed } else { incoming };
    let missed = pose.apply_through(world, entity, children);

    let Some(player) = world.get_mut::<AnimationPlayer>(entity) else {
        return;
    };
    // A miss is what is faded in's when it animates that property, and
    // otherwise the side faded out's — each warned of under its own name.
    let faded_in = |s: &Sample| !fading || incoming.get(&s.target, s.property).is_some();
    if let (true, Some(fade)) = (fading, player.fade.as_mut()) {
        fade.last.get_or_insert_with(Pose::new).copy_from(mixed);
        if let Some(pose) = held {
            fade.from = Outgoing::Pose(pose);
        }
        if !fade.reported && missed.iter().any(|&s| !faded_in(s)) {
            fade.reported = true;
            let from = match &fade.from {
                Outgoing::Clip(out) => format!("animation {}", out.clip.as_str()),
                Outgoing::Synced { set, .. } => set.describe(),
                Outgoing::Pose(_) => "an interrupted fade".to_owned(),
            };
            log::warn!(
                "{from}, fading into {}: nothing to animate for {}",
                playing(player),
                list(missed.iter().filter(|&&s| !faded_in(s))),
            );
        }
    }
    if !missed.iter().any(|&s| faded_in(s)) {
        return;
    }
    let reported = match (&player.clip, &mut player.synced) {
        (Some(_), _) => player.reported == player.clip,
        (None, Some(set)) => std::mem::replace(&mut set.reported, true),
        (None, None) => true,
    };
    if reported {
        return;
    }
    if player.clip.is_some() {
        player.reported = player.clip.clone();
    }
    log::warn!(
        "{}: nothing to animate for {}",
        playing(player),
        list(missed.iter().filter(|&&s| faded_in(s))),
    );
}

/// `motion` sampled into `out`; synced clips each into a buffer of
/// `synced`, then mixed by their weights.
fn sample<'c>(
    clips: &'c AnimationClips,
    motion: &Motion,
    picks: &[Pick],
    synced: &mut Vec<Pose<'c>>,
    out: &mut Pose<'c>,
) {
    match motion {
        Motion::Clip(handle, time) => clips.get(*handle).sample_into(*time, out),
        Motion::Synced(range) => {
            let picks = &picks[range.clone()];
            if synced.len() < picks.len() {
                synced.resize_with(picks.len(), Pose::new);
            }
            for (pose, pick) in synced.iter_mut().zip(picks) {
                clips.get(pick.handle).sample_into(pick.time, pose);
            }
            let inputs: Vec<(&Pose<'c>, f32)> = synced
                .iter()
                .zip(picks)
                .map(|(p, k)| (p, k.weight))
                .collect();
            blend_into(&inputs, out);
        }
    }
}

/// What a player plays, by name, for a warning.
fn playing(player: &AnimationPlayer) -> String {
    match (&player.clip, &player.synced) {
        (Some(clip), _) => format!("animation {}", clip.as_str()),
        (None, Some(set)) => set.describe(),
        (None, None) => "nothing".to_owned(),
    }
}

/// Missed values, listed for a warning.
fn list<'s, 'a: 's>(missed: impl Iterator<Item = &'s &'s Sample<'a>>) -> String {
    let said: Vec<String> = missed
        .map(|s| format!("{:?} on {:?}", s.property, s.target))
        .collect();
    said.join(", ")
}
