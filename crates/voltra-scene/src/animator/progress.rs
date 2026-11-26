//! How far through its state's motion an animator's entity is: what an
//! exit time waits for.

use voltra_ecs::{Entity, World};

use super::machine::{Motion, State};
use crate::keyframes::AnimationClips;
use crate::AnimationPlayer;

/// How far through its clip the entity's player is in a state.
#[derive(Debug, Clone, Copy)]
pub(super) struct Progress {
    /// Through the loop it is in, `0..=1`: one for a clip that has ended,
    /// or has no length.
    phase: f32,
    /// Lengths played since it started, through every loop.
    played: f32,
    /// Whether the clip loops — and so whether an exit time from one up
    /// counts loops, or is its end.
    looping: bool,
}

impl Progress {
    /// Before anything has played: no player yet.
    pub(super) const NONE: Self = Self {
        phase: 0.0,
        played: 0.0,
        looping: false,
    };

    /// Over, or nothing to wait for: every exit time reached.
    const ENDED: Self = Self {
        phase: 1.0,
        played: 1.0,
        looping: false,
    };

    /// Whether an exit time of `at` has been reached: below one, a point in
    /// each loop; from one up, a count of loops of a clip that loops, and
    /// the end of one that does not.
    pub(super) fn reached(self, at: f32) -> bool {
        if at < 1.0 {
            self.phase >= at
        } else if self.looping {
            self.played >= at
        } else {
            self.phase >= 1.0
        }
    }
}

/// A motion of no length has nothing to wait for: it counts as ended, so
/// every exit time has been reached — one that loops included, since
/// counting loops of nothing would never end. So does a state that plays
/// nothing. A tree counts its synced clips' phase and loops, in lengths of
/// their weighted average.
pub(super) fn progress(
    world: &World,
    clips: &mut AnimationClips,
    entity: Entity,
    state: &State,
) -> Progress {
    let Some(player) = world.get::<AnimationPlayer>(entity) else {
        return Progress::NONE;
    };
    let (phase, played, looping) = match state.motion() {
        Motion::Nothing => return Progress::ENDED,
        Motion::Clip(path) => {
            let handle = clips.load(path);
            let clip = clips.get(handle);
            let duration = clip.duration();
            if duration <= 0.0 {
                return Progress::ENDED;
            }
            (
                player.time() / duration,
                player.played() / duration,
                clip.looping,
            )
        }
        Motion::Tree(_) => {
            let Some(set) = player.synced() else {
                return Progress::NONE;
            };
            if set.length(clips) <= 0.0 {
                return Progress::ENDED;
            }
            (set.phase(), set.played(), set.looping(clips))
        }
    };
    let ended = !looping && !player.playing && phase >= 1.0;
    Progress {
        phase: if ended { 1.0 } else { phase.clamp(0.0, 1.0) },
        played,
        looping,
    }
}
