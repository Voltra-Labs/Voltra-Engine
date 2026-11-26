//! A crossfade in progress: what a player fades out, and how far it is.

use voltra_assets::AssetPath;

use super::pose::Pose;

/// A clip's clock outside a player: the side a fade fades out, which keeps
/// playing from where it was.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Playing {
    /// The clip.
    pub clip: AssetPath,
    /// Seconds into it.
    pub time: f32,
    /// The speed it was playing at.
    pub speed: f32,
    /// Whether its clock still runs; a clip that does not loop stops on its
    /// end and is held there.
    pub playing: bool,
}

/// What a fade fades out.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Outgoing {
    /// The clip played before, still playing.
    Clip(Playing),
    /// The mix that was on screen when a fade was interrupted, held still —
    /// Unity's interrupted transition: the new fade starts from what was
    /// shown, not from either clip of the old one.
    Pose(Pose<'static>),
}

/// A player's fade from what it played to its clip.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Fade {
    /// What is faded out.
    pub from: Outgoing,
    /// Seconds since it began; the player's speed does not scale it.
    elapsed: f32,
    /// Seconds it lasts, positive.
    duration: f32,
    /// The clip faded in does not loop and ended first: it holds its last
    /// frame and the fade runs on, though the player is no longer playing.
    pub held: bool,
    /// The mix last written — what a fade that interrupts this one starts
    /// from.
    pub last: Option<Pose<'static>>,
}

impl Fade {
    /// A fade out of `from`, `duration` seconds long (positive).
    pub fn new(from: Outgoing, duration: f32) -> Self {
        Self {
            from,
            elapsed: 0.0,
            duration,
            held: false,
            last: None,
        }
    }

    /// Runs it `delta` seconds.
    pub fn tick(&mut self, delta: f32) {
        if delta.is_finite() && delta > 0.0 {
            self.elapsed += delta;
        }
    }

    /// The weight of the clip faded in, `0..=1`, linear in time; the
    /// outgoing side weighs the rest.
    pub fn weight(&self) -> f32 {
        if self.duration > 0.0 {
            (self.elapsed / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// Whether it is over: the clip faded in is all there is.
    pub fn done(&self) -> bool {
        self.weight() >= 1.0
    }
}
