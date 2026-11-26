//! One player's clocks run for a frame — its clip's and, fading, the
//! outgoing one's — and what it is to write, found without writing.

use voltra_assets::Handle;

use super::clip::AnimationClip;
use super::fade::Outgoing;
use super::player::{self, AnimationPlayer};
use super::pose::Pose;
use super::store::AnimationClips;

/// What one player writes this frame, found while its clocks run.
pub(super) struct Frame {
    /// Its clip, and the time to sample it at; `None` for one that will not
    /// read.
    pub clip: Option<(Handle<AnimationClip>, f32)>,
    /// While it fades: what it fades out, and its clip's weight.
    pub fade: Option<(Source, f32)>,
}

/// The outgoing side of a fade, ready to sample.
pub(super) enum Source {
    /// A clip, at a time.
    Clip(Handle<AnimationClip>, f32),
    /// A held pose, lent by the fade for the frame and given back.
    Pose(Pose<'static>),
}

/// Runs `player`'s clocks — its clip's and, fading, the outgoing one's —
/// and answers what to write, if anything.
pub(super) fn step(
    player: &mut AnimationPlayer,
    clips: &mut AnimationClips,
    delta: f32,
) -> Option<Frame> {
    let incoming = player
        .clip
        .as_ref()
        .map(|path| clips.load(path))
        .filter(|&handle| !clips.get(handle).is_empty());
    let Some(mut fade) = player.fade.take() else {
        let handle = incoming?;
        let clip = clips.get(handle);
        return player
            .step(clip.duration(), clip.looping, delta)
            .then(|| Frame {
                clip: Some((handle, player.time())),
                fade: None,
            });
    };
    // Paused mid-fade: everything holds, the fade too.
    if !player.playing {
        player.fade = Some(fade);
        return None;
    }
    if let Some(handle) = incoming {
        let clip = clips.get(handle);
        player.step(clip.duration(), clip.looping, delta);
        if !player.playing {
            // It ended, not paused: held on its end, and kept playing so a
            // pause can still be told apart, until the fade is over.
            fade.ended = true;
            player.playing = true;
        }
    }
    fade.tick(delta);
    let clip = incoming.map(|handle| (handle, player.time()));
    if fade.done() {
        // The last frame of the fade is the clip alone, and the fade ends —
        // as does a clip that ended during it.
        if fade.ended {
            player.playing = false;
        }
        return clip.map(|clip| Frame {
            clip: Some(clip),
            fade: None,
        });
    }
    let from = match &mut fade.from {
        Outgoing::Clip(out) => {
            let handle = clips.load(&out.clip);
            let source = clips.get(handle);
            player::run_clock(
                &mut out.time,
                &mut out.playing,
                out.speed,
                source.duration(),
                source.looping,
                delta,
            );
            Source::Clip(handle, out.time)
        }
        Outgoing::Pose(pose) => Source::Pose(std::mem::take(pose)),
    };
    let weight = fade.weight();
    player.fade = Some(fade);
    Some(Frame {
        clip,
        fade: Some((from, weight)),
    })
}
