//! A clip keying a `Transform3D`: a rotation turning the short way between
//! keys, through a crossfade, and added by an additive layer.

use voltra_ecs::World;
use voltra_render::glam::{Quat, Vec3};

use super::*;
use crate::Transform3D;

/// The angle between two rotations, well conditioned near zero.
fn apart(a: Quat, b: Quat) -> f32 {
    2.0 * (a * b.conjugate()).xyz().length().min(1.0).asin()
}

fn about_y(degrees: f32) -> Quat {
    Quat::from_rotation_y(degrees.to_radians())
}

/// A clip holding the player at `x` metres along X, turned `degrees`
/// about Y.
fn hold(x: f32, degrees: f32) -> String {
    let q = about_y(degrees);
    format!(
        "(looping: true, length: 1.0, tracks: [
            (property: Translation3D, keys: [(time: 0.0, value: Vec3(({x}, 1.0, 0.0)))]),
            (property: Rotation3D, keys: [(time: 0.0, value: Quat(({}, {}, {}, {})))]),
        ])",
        q.x, q.y, q.z, q.w
    )
}

/// From no turn to `degrees` about Y over a second, and back by the second
/// after: an additive clip's change, from its first frame.
fn turn(degrees: f32) -> String {
    let (a, b) = (Quat::IDENTITY, about_y(degrees));
    format!(
        "(looping: true, length: 2.0, tracks: [
            (property: Rotation3D, keys: [
                (time: 0.0, value: Quat(({}, {}, {}, {}))),
                (time: 1.0, value: Quat(({}, {}, {}, {}))),
                (time: 2.0, value: Quat(({}, {}, {}, {}))),
            ]),
        ])",
        a.x, a.y, a.z, a.w, b.x, b.y, b.z, b.w, a.x, a.y, a.z, a.w
    )
}

fn store(files: &[(&str, String)]) -> AnimationClips {
    let root = voltra_testkit::scratch_root().keep();
    for (name, text) in files {
        std::fs::write(root.join(name), text).expect("writes");
    }
    AnimationClips::new(root)
}

fn player(world: &mut World, clip: &str) -> Entity {
    let root = world.spawn();
    world.insert(root, Transform3D::default());
    world.insert(root, AnimationPlayer::new(path(clip)));
    root
}

fn transform(world: &World, entity: Entity) -> Transform3D {
    *world.get::<Transform3D>(entity).expect("a transform")
}

#[test]
fn a_3d_rotation_turns_the_short_way_between_keys() {
    // 170° then -170°: 20° through 180°, not 340° back through 0°.
    let (a, b) = (about_y(170.0), about_y(-170.0));
    let clip = format!(
        "(tracks: [(property: Rotation3D, keys: [
            (time: 0.0, value: Quat(({}, {}, {}, {}))),
            (time: 1.0, value: Quat(({}, {}, {}, {}))),
        ])])",
        a.x, a.y, a.z, a.w, b.x, b.y, b.z, b.w
    );
    let mut clips = store(&[("spin.anim.ron", clip)]);
    let mut world = World::new();
    let root = player(&mut world, "spin.anim.ron");
    advance(&mut world, &mut clips, 0.5);
    let half = transform(&world, root).rotation;
    assert!(apart(half, about_y(180.0)) < 1e-4, "through 180°");
    advance(&mut world, &mut clips, 0.75);
    assert_eq!(
        transform(&world, root).rotation,
        b,
        "ends on its last key, bit for bit"
    );
}

#[test]
fn a_rotation_typed_near_unit_is_played_unit() {
    let clip = "(tracks: [(property: Rotation3D, keys: [
        (time: 0.0, value: Quat((0.0, 0.7, 0.0, 0.7))),
    ])])";
    let mut clips = store(&[("typed.anim.ron", clip.to_owned())]);
    let mut world = World::new();
    let root = player(&mut world, "typed.anim.ron");
    advance(&mut world, &mut clips, 0.1);
    let q = transform(&world, root).rotation;
    assert!((q.length() - 1.0).abs() < 1e-6, "{q}");
    assert!(apart(q, about_y(90.0)) < 1e-5);
}

#[test]
fn a_crossfade_slerps_a_3d_rotation_the_short_way_and_ends_on_the_new_clip() {
    let mut clips = store(&[
        ("a.anim.ron", hold(0.0, 170.0)),
        ("b.anim.ron", hold(4.0, -170.0)),
    ]);
    let mut world = World::new();
    let root = player(&mut world, "a.anim.ron");
    advance(&mut world, &mut clips, 0.1);
    assert_eq!(transform(&world, root).rotation, about_y(170.0));
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .cross_fade(path("b.anim.ron"), 1.0);

    advance(&mut world, &mut clips, 0.25);
    let quarter = transform(&world, root);
    assert!(
        (apart(quarter.rotation, about_y(170.0)).to_degrees() - 5.0).abs() < 1e-3,
        "a quarter of the 20° between"
    );
    advance(&mut world, &mut clips, 0.25);
    let half = transform(&world, root);
    assert!(apart(half.rotation, about_y(180.0)) < 1e-4, "through 180°");
    assert!((half.translation - Vec3::new(2.0, 1.0, 0.0)).length() < 1e-5);

    advance(&mut world, &mut clips, 0.6);
    let end = transform(&world, root);
    assert_eq!(end.rotation, about_y(-170.0), "the new clip's, bit for bit");
    assert_eq!(end.translation, Vec3::new(4.0, 1.0, 0.0));
}

#[test]
fn an_additive_layer_turns_a_3d_rotation_by_its_change_with_no_creep() {
    let mut clips = store(&[
        ("base.anim.ron", hold(0.0, 30.0)),
        ("turn.anim.ron", turn(40.0)),
    ]);
    let mut world = World::new();
    let root = player(&mut world, "base.anim.ron");
    let mut layer = PlayerLayer::new(LayerBlend::Additive);
    layer.player.play(path("turn.anim.ron"));
    layer.weight = 0.5;
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .layers_mut()
        .push(layer);

    // One second in, the change is 40°; by half, over the base's 30°.
    advance(&mut world, &mut clips, 1.0);
    let at = transform(&world, root).rotation;
    assert!(
        apart(at, about_y(50.0)) < 1e-4,
        "30° + 40° / 2: {}°",
        apart(at, Quat::IDENTITY).to_degrees()
    );

    // A layer holding over its base: the same pose frame after frame.
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .layers_mut()[0]
        .player
        .playing = false;
    for _ in 0..50 {
        advance(&mut world, &mut clips, 1.0 / 60.0);
    }
    let held = transform(&world, root).rotation;
    assert!(apart(held, about_y(50.0)) < 1e-4, "no creep over 50 frames");

    // Weight 0 is no layer: the base's rotation, to the bit.
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .layers_mut()[0]
        .weight = 0.0;
    advance(&mut world, &mut clips, 1.0 / 60.0);
    assert_eq!(transform(&world, root).rotation, about_y(30.0));
}
