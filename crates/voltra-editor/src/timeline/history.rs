//! A clip's own undo history: snapshots of the clip, one per gesture.

use voltra_scene::AnimationClip;

/// How many edits back a clip can be undone — the Atlas Editor's depth.
const HISTORY: usize = 200;

/// The edits made to one clip in the timeline, to take back and put back.
///
/// **Its own history, not the scene's**, as the Atlas Editor's is: a clip is
/// a file no entity owns, shared by every player of it, and a key is not a
/// change to the scene — sharing the scene's entries would make a Ctrl+Z in
/// the viewport take back a key, or a key's undo move an entity. Unity's
/// Animation window records into the global undo only because Unity has one
/// undo for assets and scenes alike.
///
/// **Snapshots, settled per gesture**: a clip is a few tracks of a few keys,
/// and a copy per edit is simpler than an inverse per operation. A key
/// dragged, a value typed, a gizmo held while recording change the clip
/// every frame; [`settle`](Self::settle) records one entry once nothing is
/// held and nothing is being typed. A Revert, or a reload from disk, is an
/// edit like any other, so it can be undone too.
#[derive(Debug, Clone)]
pub(crate) struct ClipHistory {
    /// The clip as the history last recorded it: an edit is what differs.
    committed: AnimationClip,
    undo: Vec<AnimationClip>,
    redo: Vec<AnimationClip>,
}

impl ClipHistory {
    /// A history starting from `clip` as it is.
    pub fn new(clip: &AnimationClip) -> Self {
        Self {
            committed: clip.clone(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Records whatever changed in `clip` since the last entry as one
    /// entry, if `settled` — nothing held, nothing being typed. Answers
    /// whether it recorded one.
    pub fn settle(&mut self, clip: &AnimationClip, settled: bool) -> bool {
        if !settled || *clip == self.committed {
            return false;
        }
        let before = std::mem::replace(&mut self.committed, clip.clone());
        self.undo.push(before);
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        true
    }

    /// Takes back the last edit of `clip`. Answers whether there was one.
    pub fn undo(&mut self, clip: &mut AnimationClip) -> bool {
        self.settle(clip, true);
        let Some(before) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(clip, before.clone()));
        self.committed = before;
        true
    }

    /// Puts back the last edit undone. Answers whether there was one.
    pub fn redo(&mut self, clip: &mut AnimationClip) -> bool {
        self.settle(clip, true);
        let Some(after) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(clip, after.clone()));
        self.committed = after;
        true
    }

    /// Whether there is an edit of `clip` to undo.
    pub fn can_undo(&self, clip: &AnimationClip) -> bool {
        !self.undo.is_empty() || *clip != self.committed
    }

    /// Whether there is an edit to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_scene::keyframes::{Property, Track, Value};

    #[test]
    fn a_gesture_is_one_entry_and_redo_puts_it_back() {
        let mut clip = AnimationClip::default();
        let mut history = ClipHistory::new(&clip);
        clip.tracks.push(Track::new("", Property::Rotation2D));
        clip.tracks[0].set_key(0.0, Value::Float(1.0));
        assert!(!history.settle(&clip, false), "still held");
        clip.tracks[0].keys[0].value = Value::Float(2.0);
        assert!(history.settle(&clip, true));
        assert!(!history.settle(&clip, true), "nothing new");

        assert!(history.undo(&mut clip));
        assert!(clip.tracks.is_empty(), "the whole gesture");
        assert!(!history.undo(&mut clip));
        assert!(history.redo(&mut clip));
        assert_eq!(clip.tracks[0].keys[0].value, Value::Float(2.0));
        assert!(!history.can_redo());

        clip.length = 3.0;
        assert!(history.can_undo(&clip));
        assert!(
            history.undo(&mut clip),
            "an unsettled edit is settled first"
        );
        assert_eq!(clip.length, 0.0);
        assert!(history.can_redo());
        clip.looping = true;
        history.settle(&clip, true);
        assert!(!history.can_redo(), "a new edit drops what was undone");
    }
}
