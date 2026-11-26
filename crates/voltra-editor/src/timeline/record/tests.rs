//! Recording: a move keyed at the playhead, the scene left as authored,
//! the key undone and redone, and keys pasted at the playhead.

use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::{resolve_parents, set_parent};
use voltra_scene::keyframes::Track;
use voltra_scene::{Name, SceneId, Transform};

use super::*;
use crate::undo::shortcut::UndoAction;

struct Scene {
    world: World,
    clips: AnimationClips,
    path: AssetPath,
    root: Entity,
    arm: Entity,
    timeline: Timeline,
}

/// A player at (1, 1) as authored, its clip moving it from (0, 0) to
/// (4, 0) over a second; an arm under it the clip does not animate.
fn scene() -> Scene {
    // Never read from: the clip is made in memory.
    let mut clips = AnimationClips::new("no-such-root");
    let path = AssetPath::new("walk.anim.ron").expect("valid");
    let handle = clips.load(&path);
    let mut track = Track::new("", Property::Translation);
    track.set_key(0.0, Value::Vec2(Vec2::ZERO));
    track.set_key(1.0, Value::Vec2(Vec2::new(4.0, 0.0)));
    clips.get_mut(handle).tracks = vec![track];

    let mut world = World::new();
    let root = world.spawn();
    world.insert(root, SceneId::new());
    world.insert(root, Transform::from_translation(Vec2::new(1.0, 1.0)));
    world.insert(root, AnimationPlayer::new(path.clone()));
    let arm = world.spawn();
    world.insert(arm, SceneId::new());
    world.insert(arm, Name::new("arm"));
    world.insert(arm, Transform::default());
    set_parent(&mut world, arm, root).expect("a plain reparent");
    resolve_parents(&mut world);

    let mut timeline = Timeline {
        playhead: 0.5,
        preview: true,
        record: true,
        ..Timeline::default()
    };
    timeline.open(&path);
    let clip = clips.get(handle);
    timeline.settle(&path, clip, true);
    Scene {
        world,
        clips,
        path,
        root,
        arm,
        timeline,
    }
}

impl Scene {
    fn clip(&mut self) -> &AnimationClip {
        let handle = self.clips.load(&self.path);
        self.clips.get(handle)
    }

    fn transform(&mut self, entity: Entity) -> &mut Transform {
        self.world
            .get_mut::<Transform>(entity)
            .expect("a transform")
    }

    /// One editor frame: the pose lifted, laid for the editing panels,
    /// `edit` as a panel, the moves keyed, and the clip's edit settled.
    fn frame(&mut self, edit: impl FnOnce(&mut World, Entity, Entity)) -> usize {
        self.timeline.lift(&mut self.world);
        let root = Some(self.root);
        self.timeline
            .begin_record(&mut self.world, &mut self.clips, root);
        edit(&mut self.world, self.root, self.arm);
        let keyed = self.timeline.end_record(&mut self.world, &mut self.clips);
        let handle = self.clips.load(&self.path);
        let path = self.path.clone();
        self.timeline.settle(&path, self.clips.get(handle), true);
        keyed
    }
}

#[test]
fn a_recorded_move_keys_the_pose_and_leaves_the_scene_as_authored() {
    let mut scene = scene();
    let keyed = scene.frame(|world, root, arm| {
        let t = world.get_mut::<Transform>(root).expect("t");
        assert_eq!(
            t.translation,
            Vec2::new(2.0, 0.0),
            "the panels see the pose"
        );
        t.translation += Vec2::new(1.0, 1.0);
        // A gizmo's round trip: a hair off on what it did not mean to move.
        t.scale.x += 1e-7;
        let arm = world.get_mut::<Transform>(arm).expect("t");
        arm.rotation = 0.7;
    });
    assert_eq!(keyed, 2, "the move and the arm, not the hair");
    let clip = scene.clip().clone();
    let root = &clip.tracks[0];
    assert_eq!(root.keys.len(), 3);
    assert_eq!(root.keys[1].time, 0.5);
    assert_eq!(root.keys[1].value, Value::Vec2(Vec2::new(3.0, 1.0)));
    let arm = &clip.tracks[1];
    assert_eq!(
        (arm.target.as_str(), arm.property),
        ("arm", Property::Rotation)
    );
    let keys: Vec<(f32, Value)> = arm.keys.iter().map(|k| (k.time, k.value)).collect();
    assert_eq!(
        keys,
        [(0.0, Value::Float(0.0)), (0.5, Value::Float(0.7))],
        "a new track starts from where the arm stood"
    );
    assert!(clip.tracks.iter().all(|t| t.property != Property::Scale));
    assert_eq!(
        scene.transform(scene.root).translation,
        Vec2::new(1.0, 1.0),
        "authored"
    );
    assert_eq!(scene.transform(scene.arm).rotation, 0.0, "authored");
    assert!(scene.timeline.dirty.contains(&scene.path));
}

#[test]
fn a_recorded_key_is_undone_and_redone_through_the_clip_s_own_history() {
    let mut scene = scene();
    let original = scene.clip().clone();
    // A drag over three frames: one entry once let go.
    for x in [0.25, 0.5, 0.75] {
        scene.timeline.lift(&mut scene.world);
        let root = Some(scene.root);
        scene
            .timeline
            .begin_record(&mut scene.world, &mut scene.clips, root);
        scene.transform(scene.root).translation.y += x;
        scene
            .timeline
            .end_record(&mut scene.world, &mut scene.clips);
        let handle = scene.clips.load(&scene.path);
        let path = scene.path.clone();
        scene.timeline.settle(&path, scene.clips.get(handle), false);
    }
    let recorded = scene.clip().clone();
    assert_ne!(recorded, original);
    assert!(scene.timeline.undo_redo(UndoAction::Undo, &mut scene.clips));
    assert_eq!(*scene.clip(), original, "the whole drag, at once");
    assert!(!scene.timeline.undo_redo(UndoAction::Undo, &mut scene.clips));
    assert!(scene.timeline.undo_redo(UndoAction::Redo, &mut scene.clips));
    assert_eq!(*scene.clip(), recorded);
}

#[test]
fn nothing_is_keyed_without_record_or_preview() {
    let mut scene = scene();
    scene.timeline.record = false;
    let keyed = scene.frame(|world, root, _| {
        let t = world.get_mut::<Transform>(root).expect("t");
        assert_eq!(t.translation, Vec2::new(1.0, 1.0), "the authored scene");
        t.translation.x = 9.0;
    });
    assert_eq!(keyed, 0);
    assert_eq!(
        scene.transform(scene.root).translation.x,
        9.0,
        "a scene edit"
    );
    assert_eq!(scene.clip().tracks[0].keys.len(), 2);
}

#[test]
fn a_key_copied_is_pasted_at_the_playhead_and_undone() {
    let mut scene = scene();
    scene.timeline.selected = Some((0, 1));
    assert_eq!(scene.timeline.copy_keys(&mut scene.clips), 1);
    scene.timeline.playhead = 2.5;
    assert!(scene.timeline.paste_keys(&mut scene.clips));
    assert_eq!(scene.timeline.selected, Some((0, 2)));
    let key = scene.clip().tracks[0].keys[2];
    assert_eq!(
        (key.time, key.value),
        (2.5, Value::Vec2(Vec2::new(4.0, 0.0)))
    );
    let handle = scene.clips.load(&scene.path);
    let path = scene.path.clone();
    scene.timeline.settle(&path, scene.clips.get(handle), true);
    assert!(scene.timeline.undo_redo(UndoAction::Undo, &mut scene.clips));
    assert_eq!(scene.clip().tracks[0].keys.len(), 2);
    assert_eq!(scene.timeline.selected, None, "the pasted key is gone");
}
