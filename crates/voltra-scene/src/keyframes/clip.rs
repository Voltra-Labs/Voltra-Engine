//! A clip: tracks of keys, a length, and whether it loops.

use serde::{Deserialize, Serialize};
use voltra_ecs::{Entity, World};

use super::pose::Pose;
use super::target::canonical;
use super::track::Track;

/// What a `.anim.ron` file holds.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimationClip {
    /// Seconds. Zero or less — the default — is the last key's time, so a
    /// clip that is all keys does not have to say how long it is twice.
    #[serde(skip_serializing_if = "not_positive")]
    pub length: f32,
    /// Whether the end wraps to the start. Off, the player stops on the
    /// last frame: a door that opens, a hit flash.
    pub looping: bool,
    /// The properties it animates, one track each.
    pub tracks: Vec<Track>,
}

fn not_positive(length: &f32) -> bool {
    *length <= 0.0
}

impl AnimationClip {
    /// How long one play of it lasts, in seconds; zero for a clip whose keys
    /// are all at its start.
    pub fn duration(&self) -> f32 {
        if self.length.is_finite() && self.length > 0.0 {
            return self.length;
        }
        self.tracks.iter().map(Track::end).fold(0.0, f32::max)
    }

    /// Whether it has no keys at all — what a file that would not read
    /// loads as. A player of it neither moves its clock nor ends, so the
    /// fixed file plays from where it would have started.
    pub fn is_empty(&self) -> bool {
        self.tracks.iter().all(|t| t.keys.is_empty())
    }

    /// The clip as a sampler can trust it: every track's keys sorted by
    /// time, and the keys that could not be sampled — a time or a value
    /// that is not finite, a value of another kind than the property's —
    /// dropped, with a warning naming `source`.
    ///
    /// Dropped rather than refusing the file: an author mid-edit with one
    /// bad key should still see the rest move.
    pub fn checked(mut self, source: &str) -> Self {
        for track in &mut self.tracks {
            let before = track.keys.len();
            let kind = track.property.kind();
            track
                .keys
                .retain(|k| k.time.is_finite() && k.value.is_finite() && k.value.kind() == kind);
            if track.keys.len() < before {
                log::warn!(
                    "{source}: dropped {} key(s) of {:?} on {:?} that were not a finite {kind:?}",
                    before - track.keys.len(),
                    track.property,
                    track.target,
                );
            }
            // Stable, so two keys at one time keep the order the file gave:
            // the later one is the value from then on.
            track.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        }
        if !self.length.is_finite() {
            log::warn!("{source}: a length that is not finite; using the last key's time");
            self.length = 0.0;
        }
        self
    }

    /// Every track's value at `time`, written nowhere: what a blend mixes
    /// with another clip's before anything is written. A track with no keys
    /// holds nothing; of two tracks on one property, the later's value.
    pub fn sample(&self, time: f32) -> Pose<'_> {
        let mut pose = Pose::new();
        self.sample_into(time, &mut pose);
        pose
    }

    /// [`sample`](Self::sample) into `pose`, emptied first and its room
    /// reused: a frame of players samples through a few buffers, not one
    /// per clip.
    pub fn sample_into<'a>(&'a self, time: f32, pose: &mut Pose<'a>) {
        pose.clear();
        for track in &self.tracks {
            if let Some(value) = track.sample(time) {
                pose.set(track.target.as_str(), track.property, value);
            }
        }
    }

    /// Writes every track's value at `time` onto the entities it names under
    /// `root`. Answers the tracks that found nothing to write to — a target
    /// no child is named, or one without the property's component.
    pub fn apply(&self, world: &mut World, root: Entity, time: f32) -> Vec<&Track> {
        let pose = self.sample(time);
        let missed = pose.apply(world, root);
        self.tracks
            .iter()
            .filter(|track| {
                missed
                    .iter()
                    .any(|s| s.property == track.property && s.target == canonical(&track.target))
            })
            .collect()
    }
}
