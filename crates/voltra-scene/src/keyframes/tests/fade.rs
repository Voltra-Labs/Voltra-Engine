//! A player fading from one clip into the next: each property written once
//! with their mix.

use std::f32::consts::TAU;

use voltra_ecs::World;
use voltra_render::glam::Vec2;

use super::*;
use crate::hierarchy::despawn_recursive;
use crate::{PointLight2D, Sprite, Transform};

/// A clip holding the player at `x` and its lamp at `intensity`.
fn hold(x: f32, intensity: f32) -> String {
    format!(
        "(looping: true, length: 1.0, tracks: [
            (property: Translation, keys: [(time: 0.0, value: Vec2(({x}, 0.0)))]),
            (target: \"lamp\", property: LightIntensity, keys: [(time: 0.0, value: Float({intensity}))]),
        ])"
    )
}

/// A store holding `files`.
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

fn fading(world: &mut World, root: voltra_ecs::Entity, clip: &str, duration: f32) {
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .cross_fade(path(clip), duration);
}

fn two_holds() -> AnimationClips {
    store(&[
        ("a.anim.ron", hold(0.0, 0.0)),
        ("b.anim.ron", hold(4.0, 2.0)),
    ])
}

#[test]
fn halfway_through_a_fade_each_property_is_the_mix() {
    let mut clips = two_holds();
    let mut world = World::new();
    let (root, lamp) = rig(&mut world, "a.anim.ron");
    advance(&mut world, &mut clips, 0.1);
    fading(&mut world, root, "b.anim.ron", 1.0);

    advance(&mut world, &mut clips, 0.5);
    assert!((x(&world, root) - 2.0).abs() < 1e-5, "{}", x(&world, root));
    let glow = world.get::<PointLight2D>(lamp).expect("l").intensity;
    assert!((glow - 1.0).abs() < 1e-5, "{glow}");
    let player = world.get::<AnimationPlayer>(root).expect("p");
    assert_eq!(player.fade_weight(), Some(0.5));

    advance(&mut world, &mut clips, 0.5);
    assert_eq!(x(&world, root), 4.0, "the end is the new clip exactly");
    assert_eq!(
        world.get::<AnimationPlayer>(root).expect("p").fade_weight(),
        None
    );
}

#[test]
fn the_clip_faded_out_keeps_playing_from_where_it_was() {
    let ramp = "(looping: true, tracks: [(property: Translation, keys: [
        (time: 0.0, value: Vec2((0.0, 0.0))), (time: 1.0, value: Vec2((1.0, 0.0))),
    ])])";
    let mut clips = store(&[
        ("ramp.anim.ron", ramp.into()),
        ("b.anim.ron", hold(0.0, 0.0)),
    ]);
    let mut world = World::new();
    let (root, _) = rig(&mut world, "ramp.anim.ron");
    advance(&mut world, &mut clips, 0.5);
    fading(&mut world, root, "b.anim.ron", 1.0);
    advance(&mut world, &mut clips, 0.25);
    // The ramp at 0.75, weighing three quarters; the hold at 0.
    assert!(
        (x(&world, root) - 0.5625).abs() < 1e-5,
        "{}",
        x(&world, root)
    );
}

#[test]
fn a_rotation_fades_the_short_way_round() {
    let spin = |deg: f32| {
        format!(
            "(tracks: [(property: Rotation, keys: [(time: 0.0, value: Float({}))])])",
            deg.to_radians()
        )
    };
    let mut clips = store(&[("a.anim.ron", spin(350.0)), ("b.anim.ron", spin(10.0))]);
    let mut world = World::new();
    let (root, _) = rig(&mut world, "a.anim.ron");
    advance(&mut world, &mut clips, 0.0);
    fading(&mut world, root, "b.anim.ron", 0.4);
    advance(&mut world, &mut clips, 0.2);
    let turned = world
        .get::<Transform>(root)
        .expect("t")
        .rotation
        .rem_euclid(TAU);
    assert!(
        turned < 1e-4 || TAU - turned < 1e-4,
        "through 0, not 180: {}",
        turned.to_degrees()
    );
    advance(&mut world, &mut clips, 0.5);
    let end = world.get::<Transform>(root).expect("t").rotation;
    assert!(
        (end - 10f32.to_radians()).abs() < 1e-6,
        "ends on b's own value"
    );
}

#[test]
fn a_sprite_frame_switches_at_half_way() {
    let frame = |n: i64| {
        format!("(tracks: [(property: SpriteFrame, keys: [(time: 0.0, value: Int({n}))])])")
    };
    let mut clips = store(&[("a.anim.ron", frame(2)), ("b.anim.ron", frame(7))]);
    let mut world = World::new();
    let (root, _) = rig(&mut world, "a.anim.ron");
    world.insert(root, Sprite::default());
    advance(&mut world, &mut clips, 0.0);
    fading(&mut world, root, "b.anim.ron", 1.0);
    advance(&mut world, &mut clips, 0.4);
    assert_eq!(world.get::<Sprite>(root).expect("s").frame, 2);
    advance(&mut world, &mut clips, 0.15);
    assert_eq!(
        world.get::<Sprite>(root).expect("s").frame,
        7,
        "past half way"
    );
}

#[test]
fn a_property_only_the_clip_faded_out_animates_holds_its_value() {
    let bare = "(looping: true, tracks: [(property: Translation, keys: [(time: 0.0, value: Vec2((4.0, 0.0)))])])";
    let mut clips = store(&[("a.anim.ron", hold(0.0, 3.0)), ("b.anim.ron", bare.into())]);
    let mut world = World::new();
    let (root, lamp) = rig(&mut world, "a.anim.ron");
    advance(&mut world, &mut clips, 0.0);
    fading(&mut world, root, "b.anim.ron", 1.0);
    advance(&mut world, &mut clips, 0.75);
    assert_eq!(
        world.get::<PointLight2D>(lamp).expect("l").intensity,
        3.0,
        "full weight, not pulled towards nothing"
    );
    assert!((x(&world, root) - 3.0).abs() < 1e-5);
    advance(&mut world, &mut clips, 0.5);
    assert_eq!(
        world.get::<PointLight2D>(lamp).expect("l").intensity,
        3.0,
        "no snap at the end"
    );
}

#[test]
fn a_fade_interrupted_fades_from_the_mix_on_screen() {
    let mut clips = store(&[
        ("a.anim.ron", hold(0.0, 0.0)),
        ("b.anim.ron", hold(4.0, 0.0)),
        ("c.anim.ron", hold(10.0, 0.0)),
    ]);
    let mut world = World::new();
    let (root, _) = rig(&mut world, "a.anim.ron");
    advance(&mut world, &mut clips, 0.0);
    fading(&mut world, root, "b.anim.ron", 1.0);
    advance(&mut world, &mut clips, 0.5);
    assert!((x(&world, root) - 2.0).abs() < 1e-5);

    fading(&mut world, root, "c.anim.ron", 1.0);
    advance(&mut world, &mut clips, 0.0);
    assert!(
        (x(&world, root) - 2.0).abs() < 1e-5,
        "no jump: {}",
        x(&world, root)
    );
    advance(&mut world, &mut clips, 0.5);
    assert!((x(&world, root) - 6.0).abs() < 1e-5, "{}", x(&world, root));
    advance(&mut world, &mut clips, 0.5);
    assert_eq!(x(&world, root), 10.0);
}

#[test]
fn a_target_despawned_mid_fade_costs_only_its_own_values() {
    let mut clips = two_holds();
    let mut world = World::new();
    let (root, lamp) = rig(&mut world, "a.anim.ron");
    advance(&mut world, &mut clips, 0.0);
    fading(&mut world, root, "b.anim.ron", 1.0);
    advance(&mut world, &mut clips, 0.25);
    despawn_recursive(&mut world, lamp);
    advance(&mut world, &mut clips, 0.25);
    assert!((x(&world, root) - 2.0).abs() < 1e-5);
    let player = world.get::<AnimationPlayer>(root).expect("p");
    assert_eq!(player.reported, player.clip, "warned, once");
    advance(&mut world, &mut clips, 1.0);
    assert_eq!(x(&world, root), 4.0);
}

#[test]
fn a_clip_reloaded_mid_fade_is_mixed_as_it_now_reads() {
    let mut clips = two_holds();
    let mut world = World::new();
    let (root, _) = rig(&mut world, "a.anim.ron");
    advance(&mut world, &mut clips, 0.0);
    fading(&mut world, root, "b.anim.ron", 1.0);
    advance(&mut world, &mut clips, 0.25);

    std::fs::write(clips.root().join("a.anim.ron"), hold(8.0, 0.0)).expect("writes");
    assert!(clips.reload(&path("a.anim.ron")));
    advance(&mut world, &mut clips, 0.25);
    assert!((x(&world, root) - 6.0).abs() < 1e-5, "{}", x(&world, root));

    // A save caught half written keeps what was loaded; the fade runs on.
    std::fs::write(clips.root().join("b.anim.ron"), "(tracks: [").expect("writes");
    assert!(!clips.reload(&path("b.anim.ron")));
    advance(&mut world, &mut clips, 0.5);
    assert_eq!(x(&world, root), 4.0);
}

#[test]
fn a_paused_fade_holds_and_a_clip_that_ends_first_is_held_to_the_end() {
    let short = "(tracks: [(property: Translation, keys: [
        (time: 0.0, value: Vec2((4.0, 0.0))), (time: 0.1, value: Vec2((8.0, 0.0))),
    ])])";
    let mut clips = store(&[("a.anim.ron", hold(0.0, 0.0)), ("b.anim.ron", short.into())]);
    let mut world = World::new();
    let (root, _) = rig(&mut world, "a.anim.ron");
    let playing = |world: &World| world.get::<AnimationPlayer>(root).expect("p").playing;
    advance(&mut world, &mut clips, 0.0);
    fading(&mut world, root, "b.anim.ron", 1.0);
    advance(&mut world, &mut clips, 0.25);
    // b ended at 0.1 and holds 8; a quarter of the way.
    assert!((x(&world, root) - 2.0).abs() < 1e-5, "{}", x(&world, root));
    assert!(playing(&world), "ended, but playing until the fade is over");

    // A pause now holds the fade, though its clip has ended.
    world.get_mut::<AnimationPlayer>(root).expect("p").playing = false;
    advance(&mut world, &mut clips, 0.25);
    assert!(
        (x(&world, root) - 2.0).abs() < 1e-5,
        "paused: {}",
        x(&world, root)
    );
    assert_eq!(
        world.get::<AnimationPlayer>(root).expect("p").fade_weight(),
        Some(0.25)
    );
    world.get_mut::<AnimationPlayer>(root).expect("p").playing = true;
    advance(&mut world, &mut clips, 0.25);
    assert!((x(&world, root) - 4.0).abs() < 1e-5, "the fade runs on");
    advance(&mut world, &mut clips, 0.5);
    assert_eq!(x(&world, root), 8.0);
    assert!(!playing(&world), "the fade over, the ended clip stops");

    fading(&mut world, root, "a.anim.ron", 1.0);
    advance(&mut world, &mut clips, 0.0);
    world.get_mut::<AnimationPlayer>(root).expect("p").playing = false;
    world.get_mut::<Transform>(root).expect("t").translation = Vec2::new(-1.0, 0.0);
    advance(&mut world, &mut clips, 0.5);
    assert_eq!(x(&world, root), -1.0, "paused: nothing written");
    assert_eq!(
        world.get::<AnimationPlayer>(root).expect("p").fade_weight(),
        Some(0.0)
    );
}

#[test]
fn a_miss_of_the_side_faded_out_is_not_the_new_clip_s() {
    let bare = "(looping: true, tracks: [(property: Translation, keys: [(time: 0.0, value: Vec2((4.0, 0.0)))])])";
    let mut clips = store(&[("a.anim.ron", hold(0.0, 3.0)), ("b.anim.ron", bare.into())]);
    let mut world = World::new();
    let (root, lamp) = rig(&mut world, "a.anim.ron");
    advance(&mut world, &mut clips, 0.0);
    fading(&mut world, root, "b.anim.ron", 1.0);
    world.remove::<PointLight2D>(lamp);
    advance(&mut world, &mut clips, 0.5);
    let player = world.get::<AnimationPlayer>(root).expect("p");
    assert_eq!(player.reported, None, "b animates everything it names");
    assert!(
        player.fade.as_ref().is_some_and(|f| f.reported),
        "a's miss, said once"
    );
    assert!((x(&world, root) - 2.0).abs() < 1e-5);
}
