//! Clips played from a file onto an entity and its children.

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::*;
use crate::hierarchy::{resolve_parents, set_parent};
use crate::name::Name;
use crate::scene_id::SceneId;
use crate::{PointLight2D, Sprite, Transform};

const BOB: &str = r#"(
    looping: true,
    tracks: [
        (property: Translation, keys: [
            (time: 0.0, value: Vec2((0.0, 0.0)), ease: EaseIn),
            (time: 1.0, value: Vec2((0.0, 2.0))),
            (time: 2.0, value: Vec2((0.0, 0.0))),
        ]),
        (target: "lamp", property: LightIntensity, keys: [
            (time: 0.0, value: Float(0.0)),
            (time: 2.0, value: Float(4.0)),
        ]),
    ],
)"#;

fn path(name: &str) -> AssetPath {
    AssetPath::new(name).expect("valid")
}

fn store_with(name: &str, text: &str) -> AnimationClips {
    let root = voltra_testkit::scratch_root();
    std::fs::write(root.join(name), text).expect("writes");
    AnimationClips::new(root)
}

/// A player with a lamp under it.
fn rig(world: &mut World, clip: &str) -> (Entity, Entity) {
    let root = world.spawn();
    world.insert(root, SceneId::new());
    world.insert(root, Transform::default());
    world.insert(root, AnimationPlayer::new(path(clip)));
    let lamp = world.spawn();
    world.insert(lamp, SceneId::new());
    world.insert(lamp, Name::new("lamp"));
    world.insert(lamp, PointLight2D::default());
    set_parent(world, lamp, root).expect("a plain reparent");
    resolve_parents(world);
    (root, lamp)
}

#[test]
fn a_clip_moves_its_player_and_the_child_it_names() {
    let mut clips = store_with("bob.anim.ron", BOB);
    let mut world = World::new();
    let (root, lamp) = rig(&mut world, "bob.anim.ron");

    advance(&mut world, &mut clips, 0.5);
    let at = world.get::<Transform>(root).expect("t").translation;
    assert!((at - Vec2::new(0.0, 0.5)).length() < 1e-5, "eased: {at}");
    assert!((world.get::<PointLight2D>(lamp).expect("l").intensity - 1.0).abs() < 1e-5);

    advance(&mut world, &mut clips, 2.0);
    let player = world.get::<AnimationPlayer>(root).expect("p");
    assert!((player.time() - 0.5).abs() < 1e-5, "looped");
    assert!(player.playing);
}

#[test]
fn a_paused_player_leaves_its_entity_alone() {
    let mut clips = store_with("bob.anim.ron", BOB);
    let mut world = World::new();
    let (root, _) = rig(&mut world, "bob.anim.ron");
    world.get_mut::<AnimationPlayer>(root).expect("p").playing = false;
    world.get_mut::<Transform>(root).expect("t").translation = Vec2::new(9.0, 9.0);
    advance(&mut world, &mut clips, 0.5);
    assert_eq!(
        world.get::<Transform>(root).expect("t").translation,
        Vec2::new(9.0, 9.0)
    );
}

#[test]
fn a_missing_child_or_component_costs_only_its_own_track() {
    let mut clips = store_with("bob.anim.ron", BOB);
    let mut world = World::new();
    let (root, lamp) = rig(&mut world, "bob.anim.ron");
    world.remove::<PointLight2D>(lamp);
    advance(&mut world, &mut clips, 1.0);
    assert!((world.get::<Transform>(root).expect("t").translation.y - 2.0).abs() < 1e-5);
    let player = world.get::<AnimationPlayer>(root).expect("p");
    assert_eq!(player.reported, player.clip, "warned, once");
}

#[test]
fn bad_keys_are_dropped_and_the_rest_play() {
    let text = r#"(tracks: [(property: Rotation, keys: [
        (time: 1.0, value: Float(2.0)),
        (time: 0.0, value: Vec2((1.0, 1.0))),
        (time: 0.0, value: Float(0.0)),
    ])])"#;
    let mut clips = store_with("spin.anim.ron", text);
    let handle = clips.load(&path("spin.anim.ron"));
    let clip = clips.get(handle);
    assert_eq!(clip.tracks[0].keys.len(), 2, "the Vec2 went");
    assert_eq!(clip.tracks[0].keys[0].time, 0.0, "sorted");
    assert_eq!(clip.duration(), 1.0, "from the last key");

    let mut world = World::new();
    let (root, _) = rig(&mut world, "spin.anim.ron");
    advance(&mut world, &mut clips, 5.0);
    assert_eq!(world.get::<Transform>(root).expect("t").rotation, 2.0);
    assert!(
        !world.get::<AnimationPlayer>(root).expect("p").playing,
        "not looping: ended on its last key"
    );
}

#[test]
fn a_clip_that_will_not_read_animates_nothing_until_it_is_fixed() {
    let mut clips = store_with("bob.anim.ron", "(tracks: [");
    let mut world = World::new();
    let (root, _) = rig(&mut world, "bob.anim.ron");
    advance(&mut world, &mut clips, 0.5);
    assert_eq!(
        world.get::<Transform>(root).expect("t").translation,
        Vec2::ZERO
    );

    std::fs::write(clips.root().join("bob.anim.ron"), BOB).expect("writes");
    assert!(clips.reload(&path("bob.anim.ron")));
    advance(&mut world, &mut clips, 0.5);
    assert!(world.get::<Transform>(root).expect("t").translation.y > 0.0);
}

#[test]
fn a_saved_clip_reads_back_as_it_was() {
    let mut clips = store_with("bob.anim.ron", BOB);
    let handle = clips.load(&path("bob.anim.ron"));
    clips.get_mut(handle).tracks[0].set_key(0.5, Value::Vec2(Vec2::ONE));
    clips.save(&path("bob.anim.ron")).expect("saves");
    let mut fresh = AnimationClips::new(clips.root());
    let again = fresh.load(&path("bob.anim.ron"));
    assert_eq!(fresh.get(again), clips.get(handle));
}

#[test]
fn sprite_frames_count_through_as_a_flipbook() {
    let text = r#"(looping: true, length: 1.0, tracks: [(property: SpriteFrame, keys: [
        (time: 0.0, value: Int(0)), (time: 1.0, value: Int(4)),
    ])])"#;
    let mut clips = store_with("walk.anim.ron", text);
    let mut world = World::new();
    let (root, _) = rig(&mut world, "walk.anim.ron");
    world.insert(root, Sprite::default());
    advance(&mut world, &mut clips, 0.6);
    assert_eq!(world.get::<Sprite>(root).expect("s").frame, 2);
}

#[test]
fn the_shipped_clip_reads_whole() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut clips = AnimationClips::new(root);
    let handle = clips.load(&path("animations/pulse.anim.ron"));
    let clip = clips.get(handle);
    assert_eq!(clip.tracks.len(), 2);
    assert!(clip.tracks.iter().all(|t| t.keys.len() == 3));
    assert_eq!(clip.duration(), 1.5);
}

mod fade;
