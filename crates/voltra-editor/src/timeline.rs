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

use std::collections::HashSet;

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};
use voltra_scene::keyframes::{target, Property, Value};
use voltra_scene::{AnimationClips, AnimationPlayer};

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
    /// The clip the panel last showed; another one starts it afresh.
    open: Option<AssetPath>,
    /// What the pose covered, to put back.
    covered: Vec<(Entity, Property, Value)>,
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
