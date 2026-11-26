//! Record mode: an edit to an entity under the previewing player keys
//! itself at the playhead instead of changing the scene.
//!
//! While recording, the pose is laid over the scene for the panels that edit
//! entities — the inspector and the viewport — so the gizmo and the fields
//! show, and start from, what the viewport shows. Every animatable value of
//! every named entity under the player is read then; once those panels have
//! run, each value that moved is keyed at the playhead — into its track, or
//! a new one — and the scene gets its authored value back before the scene's
//! history looks. So a recorded move is one clip edit (the clip's own undo
//! takes it back) and no scene edit. Unity's record mode, without Unity's
//! single undo for assets and scenes.

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};
use voltra_scene::keyframes::{target, Property, Value};
use voltra_scene::{AnimationClip, AnimationClips, AnimationPlayer};

use super::clipboard::track_for;
use super::Timeline;

/// A frame being recorded.
#[derive(Debug)]
pub(super) struct Recording {
    /// The player whose clip is keyed.
    root: Entity,
    /// Its clip.
    clip: AssetPath,
    /// Every value an edit could key, as the pose left it: the entity's
    /// path under the player, the entity, the property, its value.
    before: Vec<(String, Entity, Property, Value)>,
}

impl Timeline {
    /// Lays the pose for the editing panels and reads what they can change,
    /// if recording, with a preview, of a player naming a clip. Answers
    /// whether it did; [`end_record`](Self::end_record) closes it.
    pub fn begin_record(
        &mut self,
        world: &mut World,
        clips: &mut AnimationClips,
        root: Option<Entity>,
    ) -> bool {
        self.recording = None;
        if !(self.record && self.preview) {
            return false;
        }
        let Some(root) = root else {
            return false;
        };
        let Some(clip) = world
            .get::<AnimationPlayer>(root)
            .and_then(|p| p.clip.clone())
        else {
            return false;
        };
        self.pose(world, clips, Some(root));
        let mut before = Vec::new();
        for (path, entity) in target::paths(world, root) {
            for property in Property::ALL {
                if let Some(value) = property.read(world, entity) {
                    before.push((path.clone(), entity, property, value));
                }
            }
        }
        self.recording = Some(Recording { root, clip, before });
        true
    }

    /// Keys at the playhead every value the editing panels moved since
    /// [`begin_record`](Self::begin_record), then puts the scene back as
    /// authored. Answers how many it keyed. Called early — before an action
    /// that reads the scene as authored, such as a prefab's Apply — it
    /// closes the recording there; again later it does nothing.
    pub fn end_record(&mut self, world: &mut World, clips: &mut AnimationClips) -> usize {
        let Some(recording) = self.recording.take() else {
            return 0;
        };
        let mut moved = Vec::new();
        if world.is_alive(recording.root) {
            for (path, entity, property, was) in &recording.before {
                let Some(now) = property.read(world, *entity) else {
                    continue;
                };
                if changed(*was, now) {
                    moved.push((path.as_str(), *entity, *property, *was, now));
                }
            }
        }
        // What was authored: the pose read it before writing; a value the
        // pose did not cover was authored as the panels found it.
        let authored = |entity: Entity, property: Property, was: Value| {
            self.covered
                .iter()
                .find(|(e, p, _)| *e == entity && *p == property)
                .map_or(was, |(_, _, value)| *value)
        };
        let keys: Vec<(&str, Property, Value, Value)> = moved
            .iter()
            .map(|&(path, entity, property, was, now)| {
                (path, property, now, authored(entity, property, was))
            })
            .collect();
        if !keys.is_empty() {
            let handle = clips.load(&recording.clip);
            key(clips.get_mut(handle), &keys, self.playhead);
            self.dirty.insert(recording.clip.clone());
        }
        let uncovered: Vec<(Entity, Property, Value)> = moved
            .iter()
            .filter(|&&(_, entity, property, ..)| {
                !self
                    .covered
                    .iter()
                    .any(|(e, p, _)| *e == entity && *p == property)
            })
            .map(|&(_, entity, property, was, _)| (entity, property, was))
            .collect();
        self.lift(world);
        for (entity, property, was) in uncovered {
            property.write(world, entity, was);
        }
        keys.len()
    }
}

/// Keys each of `keys` — path, property, new value, authored value — at
/// `playhead`. A track made for it starts with the authored value at `0`,
/// so keying a move half way through a clip moves *from* where the entity
/// stands rather than holding the new value from the start.
pub(super) fn key(
    clip: &mut AnimationClip,
    keys: &[(&str, Property, Value, Value)],
    playhead: f32,
) {
    let time = playhead.max(0.0);
    for &(path, property, value, authored) in keys {
        let before = clip.tracks.len();
        let t = track_for(clip, path, property);
        let track = &mut clip.tracks[t];
        if t >= before && time > 0.0 {
            track.set_key(0.0, authored);
        }
        track.set_key(time, value);
    }
}

/// Whether a value moved: more than the float noise a gizmo's world-space
/// round trip leaves on the values it did not mean to change — a move's
/// rotation and scale come back a hair off.
fn changed(was: Value, now: Value) -> bool {
    let near = |a: f32, b: f32| (a - b).abs() <= 1e-5 * a.abs().max(b.abs()).max(1.0);
    match (was, now) {
        (Value::Float(a), Value::Float(b)) => !near(a, b),
        (Value::Vec2(a), Value::Vec2(b)) => !(near(a.x, b.x) && near(a.y, b.y)),
        (Value::Color(a), Value::Color(b)) => !a.iter().zip(b).all(|(a, b)| near(*a, b)),
        (a, b) => a != b,
    }
}

#[cfg(test)]
mod tests;
