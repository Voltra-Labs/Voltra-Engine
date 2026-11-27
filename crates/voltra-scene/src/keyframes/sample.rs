//! One player's frame sampled into buffers reused across players: its
//! motion, the side a fade fades out, and their mix.

use super::blend::blend_into;
use super::composite::{pad, pad_owned, subtract};
use super::fade::Outgoing;
use super::frame::{Frame, Motion, Source};
use super::player::AnimationPlayer;
use super::pose::Pose;
use super::store::AnimationClips;
use super::synced::Pick;

/// The poses a frame samples and mixes into, reused player after player.
#[derive(Default)]
pub(super) struct Buffers<'c> {
    pub incoming: Pose<'c>,
    outgoing: Pose<'c>,
    pub mixed: Pose<'c>,
    synced: Vec<Pose<'c>>,
    /// An additive clip's first frame, which its samples are taken from.
    reference: Pose<'c>,
}

/// What [`sample_frame`] left in the buffers.
pub(super) struct Sampled {
    /// Whether it fades: the result is `mixed`, not `incoming`.
    pub fading: bool,
    /// The held pose a fade fades out of, lent by the fade and to be given
    /// back ([`keep`]).
    held: Option<Pose<'static>>,
}

impl<'c> Buffers<'c> {
    /// What `sampled` comes to.
    pub fn result(&self, sampled: &Sampled) -> &Pose<'c> {
        if sampled.fading {
            &self.mixed
        } else {
            &self.incoming
        }
    }
}

/// Samples `frame` into the buffers: its motion into `incoming` and, while
/// it fades, the mix of the side faded out and it into `mixed`. In an
/// `additive` layer each clip is sampled as its change from its first
/// frame, and a property one side of a mix lacks counts as no change.
pub(super) fn sample_frame<'c>(
    clips: &'c AnimationClips,
    picks: &[Pick],
    buffers: &mut Buffers<'c>,
    frame: Frame,
    additive: bool,
) -> Sampled {
    let Buffers {
        incoming,
        outgoing,
        mixed,
        synced,
        reference,
    } = buffers;
    match &frame.motion {
        Some(motion) => sample(clips, motion, picks, synced, reference, additive, incoming),
        None => incoming.clear(),
    }
    let Some((from, weight)) = frame.fade else {
        return Sampled {
            fading: false,
            held: None,
        };
    };
    let mut held = None;
    match from {
        Source::Motion(motion) => {
            sample(clips, &motion, picks, synced, reference, additive, outgoing);
        }
        Source::Pose(pose) => held = Some(pose),
    }
    let from: &Pose<'c> = match &mut held {
        Some(pose) => {
            if additive {
                pad_owned(pose, incoming);
                pad(incoming, pose);
            }
            pose
        }
        None => {
            if additive {
                pad(outgoing, incoming);
                pad(incoming, outgoing);
            }
            outgoing
        }
    };
    blend_into(&[(from, 1.0 - weight), (incoming, weight)], mixed);
    Sampled { fading: true, held }
}

/// Keeps what a fade needs of the frame: the mix it wrote, which a fade
/// interrupting it starts from, and the held pose it lent.
pub(super) fn keep(player: &mut AnimationPlayer, sampled: &mut Sampled, mixed: &Pose<'_>) {
    if let (true, Some(fade)) = (sampled.fading, player.fade.as_mut()) {
        fade.last.get_or_insert_with(Pose::new).copy_from(mixed);
        if let Some(pose) = sampled.held.take() {
            fade.from = Outgoing::Pose(pose);
        }
    }
}

/// `motion` sampled into `out`; synced clips each into a buffer of
/// `synced`, then mixed by their weights. `additive`, each clip's change
/// from its first frame (sampled into `reference`).
fn sample<'c>(
    clips: &'c AnimationClips,
    motion: &Motion,
    picks: &[Pick],
    synced: &mut Vec<Pose<'c>>,
    reference: &mut Pose<'c>,
    additive: bool,
    out: &mut Pose<'c>,
) {
    match motion {
        Motion::Clip(handle, time) => {
            let clip = clips.get(*handle);
            clip.sample_into(*time, out);
            if additive {
                clip.sample_into(0.0, reference);
                subtract(out, reference);
            }
        }
        Motion::Synced(range) => {
            let picks = &picks[range.clone()];
            if synced.len() < picks.len() {
                synced.resize_with(picks.len(), Pose::new);
            }
            for (pose, pick) in synced.iter_mut().zip(picks) {
                let clip = clips.get(pick.handle);
                clip.sample_into(pick.time, pose);
                if additive {
                    clip.sample_into(0.0, reference);
                    subtract(pose, reference);
                }
            }
            if additive {
                let used = &mut synced[..picks.len()];
                for i in 0..used.len() {
                    for j in 0..used.len() {
                        if i != j {
                            let (into, from) = pair(used, i, j);
                            pad(into, from);
                        }
                    }
                }
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

/// Entry `i` to change and entry `j` to read, `i != j`.
fn pair<T>(items: &mut [T], i: usize, j: usize) -> (&mut T, &T) {
    if i < j {
        let (low, high) = items.split_at_mut(j);
        (&mut low[i], &high[0])
    } else {
        let (low, high) = items.split_at_mut(i);
        (&mut high[0], &low[j])
    }
}
