//! The timeline's state, and the preview pose it lays over the scene.
//!
//! **The pose is laid between frames, never inside one.** The scene is drawn
//! before the editor's UI runs, so [`Timeline::pose`] writes the clip's
//! values at the playhead as the editor's last act in a frame, and
//! [`Timeline::lift`] puts the authored values back as its first act in the
//! next. The viewport shows the pose; every panel, the undo history and a
//! save see only what was authored, so scrubbing is never an edit. Unity's
//! preview mode keeps the authored values aside for the same reason; Godot,
//! which writes the pose into the scene, needs a RESET track to undo it.
//! Record mode ([`record`]) is the one exception: while it is on, the pose
//! is laid again for the panels that edit entities, and what they move is
//! keyed instead of authored.
//!
//! A clip's edits have an undo history of their own ([`history`]), and keys
//! a clipboard of their own ([`clipboard`]).

pub(crate) mod clipboard;
mod history;
mod record;

use std::collections::{HashMap, HashSet};

use voltra_assets::AssetPath;
use voltra_core::egui::{Context, Rect};
use voltra_ecs::{Entity, World};
use voltra_scene::hierarchy::parent_of;
use voltra_scene::keyframes::{target, Property, Value};
use voltra_scene::{AnimationClip, AnimationClips, AnimationPlayer};

use crate::undo::shortcut::UndoAction;
use clipboard::CopiedKey;
use history::ClipHistory;
use record::Recording;

/// What the timeline remembers between frames.
#[derive(Default)]
pub(crate) struct Timeline {
    /// Seconds: where a key is added, and what the preview shows.
    pub playhead: f32,
    /// Whether the viewport shows the clip at the playhead.
    pub preview: bool,
    /// The selected key, as (track, key) indices into the open clip.
    pub selected: Option<(usize, usize)>,
    /// Clips edited since they were last saved or reverted.
    pub dirty: HashSet<AssetPath>,
    /// The "add track" row's choices.
    pub new_target: String,
    pub new_property: Option<Property>,
    /// Whether an edit to an entity under the previewing player — a gizmo
    /// move, an inspector field — keys itself at the playhead. Only with
    /// the preview on.
    pub record: bool,
    /// Keys copied, to paste at the playhead.
    pub copied: Vec<CopiedKey>,
    /// Where the panel was drawn last frame, if it was: the undo and
    /// clipboard keys over it are the clip's.
    pub rect: Option<Rect>,
    /// The clip the panel last showed; another one starts it afresh.
    open: Option<AssetPath>,
    /// Each clip's own undo history, by path, from when it was first shown.
    histories: HashMap<AssetPath, ClipHistory>,
    /// What the pose covered, to put back.
    covered: Vec<(Entity, Property, Value)>,
    /// The frame being recorded, between the editing panels.
    recording: Option<Recording>,
}

/// The entity whose player the timeline shows for `selected`: the selected
/// entity, or the nearest above it whose player names a clip — so selecting
/// an arm keeps its body's clip open, to key the arm in.
pub(crate) fn root(world: &World, selected: Option<Entity>) -> Option<Entity> {
    let mut at = selected?;
    loop {
        let plays = world
            .get::<AnimationPlayer>(at)
            .is_some_and(|p| p.clip.is_some());
        if plays {
            return Some(at);
        }
        at = parent_of(world, at)?;
    }
}

impl Timeline {
    /// Starts showing `clip`, unless it already is: a key selected in one
    /// clip means nothing in another.
    pub fn open(&mut self, clip: &AssetPath) {
        if self.open.as_ref() == Some(clip) {
            return;
        }
        self.open = Some(clip.clone());
        self.selected = None;
        self.new_target.clear();
        self.new_property = None;
    }

    /// Records the open clip's edits since the last entry as one entry, once
    /// `settled` — nothing held, nothing typed. Starts its history the first
    /// time it is shown.
    pub fn settle(&mut self, path: &AssetPath, clip: &AnimationClip, settled: bool) {
        self.histories
            .entry(path.clone())
            .or_insert_with(|| ClipHistory::new(clip))
            .settle(clip, settled);
    }

    /// Whether the open clip has an edit to undo, and one to redo.
    pub fn can_undo_redo(&self, path: &AssetPath, clip: &AnimationClip) -> (bool, bool) {
        self.histories
            .get(path)
            .map_or((false, false), |h| (h.can_undo(clip), h.can_redo()))
    }

    /// Takes back, or puts back, the open clip's last edit. Answers whether
    /// there was one.
    pub fn undo_redo(&mut self, action: UndoAction, clips: &mut AnimationClips) -> bool {
        let Some(path) = self.open.clone() else {
            return false;
        };
        let Some(history) = self.histories.get_mut(&path) else {
            return false;
        };
        let handle = clips.load(&path);
        let clip = clips.get_mut(handle);
        let done = match action {
            UndoAction::Undo => history.undo(clip),
            UndoAction::Redo => history.redo(clip),
        };
        if done {
            self.dirty.insert(path);
            let gone = self
                .selected
                .is_some_and(|(t, k)| clip.tracks.get(t).is_none_or(|track| k >= track.keys.len()));
            if gone {
                self.selected = None;
            }
        } else {
            log::info!(
                "nothing to {} in {}",
                match action {
                    UndoAction::Undo => "undo",
                    UndoAction::Redo => "redo",
                },
                path.as_str()
            );
        }
        done
    }

    /// Whether the undo and clipboard keys are the clip's: the pointer is
    /// over the panel — or, for undo, record is on, every edit then being
    /// a key.
    pub fn has_keys(&self, ctx: &Context, undo: bool) -> bool {
        let over = self
            .rect
            .zip(ctx.pointer_hover_pos())
            .is_some_and(|(rect, at)| rect.contains(at));
        self.rect.is_some() && (over || (undo && self.record && self.preview))
    }

    /// Puts back the authored values the last pose covered. The first thing
    /// the editor does in a frame.
    pub fn lift(&mut self, world: &mut World) {
        // In reverse, so a property two tracks cover ends on the value read
        // before either wrote.
        for (entity, property, value) in self.covered.drain(..).rev() {
            property.write(world, entity, value);
        }
    }

    /// Lays the clip `root` plays, at the playhead, over the scene — if the
    /// preview is on. The last thing the editor does in a frame.
    pub fn pose(&mut self, world: &mut World, clips: &mut AnimationClips, root: Option<Entity>) {
        if !self.preview {
            return;
        }
        let Some(root) = root else {
            return;
        };
        let Some(path) = world
            .get::<AnimationPlayer>(root)
            .and_then(|p| p.clip.clone())
        else {
            return;
        };
        let handle = clips.load(&path);
        let clip = clips.get(handle);
        // Every value read before any is written, so two tracks on one
        // property both remember the authored one.
        for track in &clip.tracks {
            let Some(entity) = target::resolve(world, root, &track.target) else {
                continue;
            };
            if let Some(value) = track.property.read(world, entity) {
                self.covered.push((entity, track.property, value));
            }
        }
        clip.apply(world, root, self.playhead);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_render::glam::Vec2;
    use voltra_scene::keyframes::Track;
    use voltra_scene::Transform;

    #[test]
    fn a_pose_is_lifted_back_to_what_was_authored() {
        // Never read from: the clip is made in memory.
        let mut clips = AnimationClips::new("no-such-root");
        let path = AssetPath::new("a.anim.ron").expect("valid");
        let handle = clips.load(&path);
        let mut track = Track::new("", Property::Translation);
        track.set_key(0.0, Value::Vec2(Vec2::ZERO));
        track.set_key(1.0, Value::Vec2(Vec2::new(4.0, 0.0)));
        clips.get_mut(handle).tracks = vec![track.clone(), track];

        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(Vec2::new(1.0, 1.0)));
        world.insert(entity, AnimationPlayer::new(path));
        let at = |world: &World| world.get::<Transform>(entity).expect("t").translation;

        let mut timeline = Timeline {
            playhead: 0.5,
            ..Timeline::default()
        };
        timeline.pose(&mut world, &mut clips, Some(entity));
        assert_eq!(at(&world), Vec2::new(1.0, 1.0), "no preview, no pose");

        timeline.preview = true;
        timeline.pose(&mut world, &mut clips, Some(entity));
        assert_eq!(at(&world), Vec2::new(2.0, 0.0));
        timeline.lift(&mut world);
        assert_eq!(at(&world), Vec2::new(1.0, 1.0));
        timeline.lift(&mut world);
        assert_eq!(at(&world), Vec2::new(1.0, 1.0), "lifting twice is once");
    }
}
