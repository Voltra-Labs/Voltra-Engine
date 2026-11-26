//! Keys copied off a clip and pasted at the playhead.

use voltra_scene::keyframes::target::canonical;
use voltra_scene::keyframes::{Ease, Property, Track, Value};
use voltra_scene::{AnimationClip, AnimationClips};

use super::Timeline;

/// Keys closer than this to the playhead are "at" it, for a copy.
const AT: f32 = 0.005;

/// One key on the timeline's clipboard: which track it came from and what
/// it held. Every key copied is at one time — the selected key, or a column
/// at the playhead — so a paste puts them all at the playhead.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CopiedKey {
    /// The track's target, as the track named it.
    pub target: String,
    /// The track's property.
    pub property: Property,
    /// What it held.
    pub value: Value,
    /// Its ease.
    pub ease: Ease,
}

impl Timeline {
    /// Copies the open clip's selected key — or, none selected, every key
    /// at the playhead. Answers how many.
    pub fn copy_keys(&mut self, clips: &mut AnimationClips) -> usize {
        let Some(path) = self.open.clone() else {
            return 0;
        };
        let handle = clips.load(&path);
        let copied = copy(clips.get(handle), self.selected, self.playhead);
        if copied.is_empty() {
            log::info!("no key selected, nor at the playhead, to copy");
            return 0;
        }
        self.copied = copied;
        self.copied.len()
    }

    /// Pastes the keys copied at the playhead of the open clip, selecting
    /// the last. Answers whether it pasted any.
    pub fn paste_keys(&mut self, clips: &mut AnimationClips) -> bool {
        let Some(path) = self.open.clone() else {
            return false;
        };
        if self.copied.is_empty() {
            return false;
        }
        let handle = clips.load(&path);
        let into = self.selected.map(|(t, _)| t);
        let landed = paste(clips.get_mut(handle), &self.copied, self.playhead, into);
        self.selected = landed.last().copied();
        self.dirty.insert(path);
        true
    }

    /// Deletes the open clip's selected key. Answers whether there was one.
    pub fn delete_key(&mut self, clips: &mut AnimationClips) -> bool {
        let (Some(path), Some((t, k))) = (self.open.clone(), self.selected) else {
            return false;
        };
        let handle = clips.load(&path);
        let Some(track) = clips.get_mut(handle).tracks.get_mut(t) else {
            return false;
        };
        if k >= track.keys.len() {
            return false;
        }
        track.keys.remove(k);
        self.selected = None;
        self.dirty.insert(path);
        true
    }
}

/// The keys to copy: the selected key, or with none selected every key at
/// the playhead — a column of the pose there. Empty when there is none.
pub(crate) fn copy(
    clip: &AnimationClip,
    selected: Option<(usize, usize)>,
    playhead: f32,
) -> Vec<CopiedKey> {
    let mut picked: Vec<(&Track, usize)> = Vec::new();
    match selected {
        Some((t, k)) => {
            if let Some(track) = clip.tracks.get(t).filter(|track| k < track.keys.len()) {
                picked.push((track, k));
            }
        }
        None => {
            for track in &clip.tracks {
                if let Some(k) = track
                    .keys
                    .iter()
                    .position(|key| (key.time - playhead).abs() < AT)
                {
                    picked.push((track, k));
                }
            }
        }
    }
    picked
        .into_iter()
        .map(|(track, k)| {
            let key = &track.keys[k];
            CopiedKey {
                target: track.target.clone(),
                property: track.property,
                value: key.value,
                ease: key.ease,
            }
        })
        .collect()
}

/// Pastes `copied` at `playhead`, replacing a key already there. One key
/// goes into the `into` track when that track takes its kind of value —
/// a position copied off one entity pasted onto another's; otherwise each
/// goes into the track of its own target and property, made if the clip has
/// none. Answers where each landed, as (track, key).
pub(crate) fn paste(
    clip: &mut AnimationClip,
    copied: &[CopiedKey],
    playhead: f32,
    into: Option<usize>,
) -> Vec<(usize, usize)> {
    let single = match (copied, into) {
        ([one], Some(t)) => clip
            .tracks
            .get(t)
            .filter(|track| track.property.kind() == one.property.kind())
            .map(|_| t),
        _ => None,
    };
    let time = playhead.max(0.0);
    let mut landed = Vec::with_capacity(copied.len());
    for key in copied {
        let t = single.unwrap_or_else(|| track_for(clip, &key.target, key.property));
        let track = &mut clip.tracks[t];
        let k = track.set_key(time, key.value);
        track.keys[k].ease = key.ease;
        landed.push((t, k));
    }
    landed
}

/// The track of `target`'s `property`, made at the end if there is none.
pub(crate) fn track_for(clip: &mut AnimationClip, target: &str, property: Property) -> usize {
    let wanted = canonical(target);
    match clip
        .tracks
        .iter()
        .position(|t| t.property == property && canonical(&t.target) == wanted)
    {
        Some(t) => t,
        None => {
            clip.tracks.push(Track::new(wanted.into_owned(), property));
            clip.tracks.len() - 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;

    fn clip() -> AnimationClip {
        let mut arm = Track::new("arm", Property::Rotation);
        arm.set_key(0.0, Value::Float(0.0));
        arm.set_key(0.5, Value::Float(1.0));
        let mut root = Track::new("", Property::Translation);
        root.set_key(0.5, Value::Vec2(Vec2::new(2.0, 0.0)));
        root.set_key(1.0, Value::Vec2(Vec2::ZERO));
        AnimationClip {
            tracks: vec![arm, root],
            ..AnimationClip::default()
        }
    }

    #[test]
    fn a_key_pasted_at_the_playhead_lands_there_in_its_own_track() {
        let mut clip = clip();
        let copied = copy(&clip, Some((0, 1)), 0.0);
        assert_eq!(copied.len(), 1);
        let landed = paste(&mut clip, &copied, 1.25, None);
        assert_eq!(landed, [(0, 2)]);
        let key = clip.tracks[0].keys[2];
        assert_eq!((key.time, key.value), (1.25, Value::Float(1.0)));
    }

    #[test]
    fn the_keys_at_the_playhead_copy_as_a_column_and_paste_keeping_their_tracks() {
        let mut clip = clip();
        let copied = copy(&clip, None, 0.5);
        assert_eq!(copied.len(), 2, "the arm's and the root's at 0.5");
        paste(&mut clip, &copied, 2.0, Some(0));
        assert_eq!(clip.tracks[0].keys.last().map(|k| k.time), Some(2.0));
        assert_eq!(
            clip.tracks[1].keys.last().map(|k| k.value),
            Some(Value::Vec2(Vec2::new(2.0, 0.0))),
            "two keys go to their own tracks, whatever is selected"
        );
    }

    #[test]
    fn one_key_goes_into_the_selected_track_of_its_kind_or_makes_its_own() {
        let mut clip = clip();
        let copied = copy(&clip, Some((1, 0)), 0.0);
        let mut lamp = Track::new("lamp", Property::Translation);
        lamp.set_key(0.0, Value::Vec2(Vec2::ZERO));
        clip.tracks.push(lamp);
        assert_eq!(paste(&mut clip, &copied, 0.75, Some(2)), [(2, 1)]);
        assert_eq!(
            paste(&mut clip, &copied, 0.75, Some(0)),
            [(1, 1)],
            "a rotation takes no position"
        );

        let mut empty = AnimationClip::default();
        assert_eq!(paste(&mut empty, &copied, 0.0, None), [(0, 0)]);
        assert_eq!(empty.tracks[0].property, Property::Translation);
        assert!(copy(&empty, None, 9.0).is_empty());
    }
}
