//! One player's clocks run for a frame — its clip's and, fading, the
//! outgoing one's — and what it is to write, found without writing.

use std::ops::Range;

use voltra_assets::Handle;

use super::clip::AnimationClip;
use super::clock;
use super::fade::Outgoing;
use super::player::AnimationPlayer;
use super::pose::Pose;
use super::store::AnimationClips;
use super::synced::Pick;

/// What one player writes this frame, found while its clocks run.
pub(super) struct Frame {
    /// What it plays, and when to sample it; `None` for nothing that reads.
    pub motion: Option<Motion>,
    /// While it fades: what it fades out, and its motion's weight.
    pub fade: Option<(Source, f32)>,
}

/// A clip, or synced clips, ready to sample.
pub(super) enum Motion {
    /// A clip, at a time.
    Clip(Handle<AnimationClip>, f32),
    /// Clips mixed by weight: these entries of the frame's picks.
    Synced(Range<usize>),
}

/// The outgoing side of a fade, ready to sample.
pub(super) enum Source {
    /// A clip or synced clips, still playing.
    Motion(Motion),
    /// A held pose, lent by the fade for the frame and given back.
    Pose(Pose<'static>),
}

/// What a player plays now, as the store has it.
enum Now {
    Clip(Handle<AnimationClip>),
    Synced,
}

/// Runs `player`'s clocks — its motion's and, fading, the outgoing one's —
/// and answers what to write, if anything. Synced clips to sample go onto
/// `picks`, which the frame's [`Motion::Synced`] ranges index.
pub(super) fn step(
    player: &mut AnimationPlayer,
    clips: &mut AnimationClips,
    delta: f32,
    picks: &mut Vec<Pick>,
) -> Option<Frame> {
    let now = now(player, clips);
    let Some(mut fade) = player.fade.take() else {
        let now = now?;
        return run(player, &now, clips, delta).then(|| Frame {
            motion: Some(pick(player, &now, clips, picks)),
            fade: None,
        });
    };
    // Paused mid-fade: everything holds, the fade too.
    if !player.playing {
        player.fade = Some(fade);
        return None;
    }
    if let Some(now) = &now {
        run(player, now, clips, delta);
        if !player.playing {
            // It ended, not paused: held on its end, and kept playing so a
            // pause can still be told apart, until the fade is over.
            fade.ended = true;
            player.playing = true;
        }
    }
    fade.tick(delta);
    let motion = now.map(|now| pick(player, &now, clips, picks));
    if fade.done() {
        // The last frame of the fade is the motion alone, and the fade
        // ends — as does a clip that ended during it.
        if fade.ended {
            player.playing = false;
        }
        return motion.map(|motion| Frame {
            motion: Some(motion),
            fade: None,
        });
    }
    let from = match &mut fade.from {
        Outgoing::Clip(out) => {
            let handle = clips.load(&out.clip);
            let source = clips.get(handle);
            clock::run_clock(
                &mut out.time,
                &mut out.playing,
                out.speed,
                source.duration(),
                source.looping,
                delta,
            );
            Source::Motion(Motion::Clip(handle, out.time))
        }
        Outgoing::Synced {
            set,
            speed,
            playing,
        } => {
            set.step(clips, playing, *speed, delta);
            let start = picks.len();
            set.pick(clips, picks);
            Source::Motion(Motion::Synced(start..picks.len()))
        }
        Outgoing::Pose(pose) => Source::Pose(std::mem::take(pose)),
    };
    let weight = fade.weight();
    player.fade = Some(fade);
    Some(Frame {
        motion,
        fade: Some((from, weight)),
    })
}

/// What a paused or ended `player` would write, its clocks left where they
/// are: a layer's held pose. `None` when it plays nothing.
pub(super) fn held(
    player: &mut AnimationPlayer,
    clips: &mut AnimationClips,
    picks: &mut Vec<Pick>,
) -> Option<Frame> {
    // A step of no time samples what is there without moving it; the flag
    // it would set on a clip at its end is the one already true.
    let was = player.playing;
    player.playing = true;
    let frame = step(player, clips, 0.0, picks);
    player.playing = was;
    frame
}

/// What `player` plays, if anything: a clip with keys, or synced clips.
fn now(player: &mut AnimationPlayer, clips: &mut AnimationClips) -> Option<Now> {
    if let Some(path) = &player.clip {
        // A clip written over a blend plays: the field is the older way in.
        player.synced = None;
        let handle = clips.load(path);
        return (!clips.get(handle).is_empty()).then_some(Now::Clip(handle));
    }
    // Whether it has anything to write is its clock's to find.
    player.synced.as_ref().map(|_| Now::Synced)
}

/// Runs the clock of what `player` plays. Answers whether to sample it.
fn run(player: &mut AnimationPlayer, now: &Now, clips: &mut AnimationClips, delta: f32) -> bool {
    match now {
        Now::Clip(handle) => {
            let clip = clips.get(*handle);
            player.step(clip.duration(), clip.looping, delta)
        }
        Now::Synced => match player.synced.as_mut() {
            Some(set) => set.step(clips, &mut player.playing, player.speed, delta),
            None => false,
        },
    }
}

/// What to sample of what `player` plays, its clock run.
fn pick(
    player: &AnimationPlayer,
    now: &Now,
    clips: &mut AnimationClips,
    picks: &mut Vec<Pick>,
) -> Motion {
    match now {
        Now::Clip(handle) => Motion::Clip(*handle, player.time()),
        Now::Synced => {
            let start = picks.len();
            if let Some(set) = &player.synced {
                set.pick(clips, picks);
            }
            Motion::Synced(start..picks.len())
        }
    }
}
