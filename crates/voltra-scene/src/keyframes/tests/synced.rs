//! A player playing several clips at once, synced and mixed by weight, and
//! fading between such a blend and a clip.

use voltra_ecs::World;

use super::*;
use crate::Transform;

/// A looping clip `length` long moving the player from `0` to `to` in x.
fn sweep(length: f32, to: f32) -> String {
    format!(
        "(looping: true, length: {length}, tracks: [
            (property: Translation, keys: [(time: 0.0, value: Vec2((0.0, 0.0))), (time: {length}, value: Vec2(({to}, 0.0)))]),
        ])"
    )
}

fn store(files: &[(&str, String)]) -> AnimationClips {
    let root = voltra_testkit::scratch_root();
    for (name, text) in files {
        std::fs::write(root.join(name), text).expect("writes");
    }
    AnimationClips::new(root)
}

fn x(world: &World, entity: voltra_ecs::Entity) -> f32 {
    world.get::<Transform>(entity).expect("t").translation.x
}

fn walk_and_run() -> AnimationClips {
    store(&[
        ("walk.anim.ron", sweep(1.0, 4.0)),
        ("run.anim.ron", sweep(0.5, 8.0)),
        ("hold.anim.ron", sweep(1.0, 0.0)),
    ])
}

fn set(weights: [f32; 2]) -> SyncedClips {
    SyncedClips::new([
        (path("walk.anim.ron"), weights[0]),
        (path("run.anim.ron"), weights[1]),
    ])
}

#[test]
fn synced_clips_are_sampled_at_one_phase_and_mixed_by_weight() {
    let mut clips = walk_and_run();
    let mut world = World::new();
    let (root, _) = rig(&mut world, "walk.anim.ron");
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .play_synced(set([0.5, 0.5]));
    // Three quarters of a second long: half way after 0.375 s, where the
    // walk is at 2 and the run at 4.
    advance(&mut world, &mut clips, 0.375);
    assert!((x(&world, root) - 3.0).abs() < 1e-4, "{}", x(&world, root));

    let player = world.get_mut::<AnimationPlayer>(root).expect("p");
    assert!(player.clip.is_none());
    player
        .synced_mut()
        .expect("a blend")
        .weights_mut()
        .copy_from_slice(&[1.0, 0.0]);
    advance(&mut world, &mut clips, 0.0);
    assert!(
        (x(&world, root) - 2.0).abs() < 1e-4,
        "all walk, still half way"
    );

    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .play(path("hold.anim.ron"));
    advance(&mut world, &mut clips, 0.1);
    assert_eq!(x(&world, root), 0.0, "a clip played cuts the blend");
    assert!(world
        .get::<AnimationPlayer>(root)
        .expect("p")
        .synced()
        .is_none());
}

#[test]
fn a_blend_fades_into_a_clip_and_a_clip_into_a_blend() {
    let mut clips = walk_and_run();
    let mut world = World::new();
    let (root, _) = rig(&mut world, "hold.anim.ron");
    advance(&mut world, &mut clips, 0.1);
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .cross_fade_synced(set([1.0, 0.0]), 1.0);
    advance(&mut world, &mut clips, 0.5);
    // The walk half way (2), the hold at 0, each at half.
    assert!((x(&world, root) - 1.0).abs() < 1e-4, "{}", x(&world, root));

    advance(&mut world, &mut clips, 0.5);
    let player = world.get_mut::<AnimationPlayer>(root).expect("p");
    assert_eq!(player.fade_weight(), None, "over");
    player.cross_fade(path("hold.anim.ron"), 1.0);
    // The blend, faded out, keeps playing: from a whole loop to half way.
    advance(&mut world, &mut clips, 0.5);
    assert!((x(&world, root) - 1.0).abs() < 1e-4, "{}", x(&world, root));
    advance(&mut world, &mut clips, 0.5);
    assert_eq!(x(&world, root), 0.0, "the clip alone");
}

#[test]
fn a_blend_of_nothing_that_reads_writes_nothing_and_warns_of_nothing() {
    let mut clips = walk_and_run();
    let mut world = World::new();
    let (root, _) = rig(&mut world, "walk.anim.ron");
    world.get_mut::<Transform>(root).expect("t").translation.x = 9.0;
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .play_synced(SyncedClips::new([(path("missing.anim.ron"), 1.0)]));
    advance(&mut world, &mut clips, 0.3);
    assert_eq!(x(&world, root), 9.0);
    let player = world.get_mut::<AnimationPlayer>(root).expect("p");
    assert_eq!(player.synced().expect("kept").phase(), 0.0, "no clock");
    player.stop();
    assert!(player.synced().is_none() && player.clip.is_none());
}
