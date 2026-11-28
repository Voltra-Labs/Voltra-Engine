//! A clip keying a `Transform3D`: a rotation turning the short way between
//! keys, through a crossfade, and added by an additive layer.

use voltra_ecs::World;
use voltra_render::glam::{Quat, Vec3};

use super::*;
use crate::Transform3D;
use voltra_testkit::rotation_apart;

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
    assert!(rotation_apart(half, about_y(180.0)) < 1e-4, "through 180°");
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
    assert!(rotation_apart(q, about_y(90.0)) < 1e-5);
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
        (rotation_apart(quarter.rotation, about_y(170.0)).to_degrees() - 5.0).abs() < 1e-3,
        "a quarter of the 20° between"
    );
    advance(&mut world, &mut clips, 0.25);
    let half = transform(&world, root);
    assert!(
        rotation_apart(half.rotation, about_y(180.0)) < 1e-4,
        "through 180°"
    );
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
        rotation_apart(at, about_y(50.0)) < 1e-4,
        "30° + 40° / 2: {}°",
        rotation_apart(at, Quat::IDENTITY).to_degrees()
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
    assert!(
        rotation_apart(held, about_y(50.0)) < 1e-4,
        "no creep over 50 frames"
    );

    // Weight 0 is no layer: the base's rotation, to the bit.
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .layers_mut()[0]
        .weight = 0.0;
    advance(&mut world, &mut clips, 1.0 / 60.0);
    assert_eq!(transform(&world, root).rotation, about_y(30.0));
}

/// A clip that moves the player and leaves its rotation alone, so what is
/// under a layer's rotation is the world's.
fn still() -> String {
    "(looping: true, length: 1.0, tracks: [
        (property: Translation3D, keys: [(time: 0.0, value: Vec3((0.0, 1.0, 0.0)))]),
    ])"
    .to_owned()
}

/// From no turn to `degrees` about X over a second: a tilt, which does not
/// commute with a yaw.
fn tilt(degrees: f32) -> String {
    let q = Quat::from_rotation_x(degrees.to_radians());
    format!(
        "(looping: false, length: 1.0, tracks: [
            (property: Rotation3D, keys: [
                (time: 0.0, value: Quat((0.0, 0.0, 0.0, 1.0))),
                (time: 1.0, value: Quat(({}, {}, {}, {}))),
            ]),
        ])",
        q.x, q.y, q.z, q.w
    )
}

/// A player under a layer of `blend` at `weight` playing `clip`, played
/// a second and held there.
fn layered(clip: String, blend: LayerBlend, weight: f32) -> (World, AnimationClips, Entity) {
    let mut clips = store(&[("still.anim.ron", still()), ("layer.anim.ron", clip)]);
    let mut world = World::new();
    let root = player(&mut world, "still.anim.ron");
    let mut layer = PlayerLayer::new(blend);
    layer.player.play(path("layer.anim.ron"));
    layer.weight = weight;
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .layers_mut()
        .push(layer);
    advance(&mut world, &mut clips, 1.0);
    world
        .get_mut::<AnimationPlayer>(root)
        .expect("p")
        .layers_mut()[0]
        .player
        .playing = false;
    (world, clips, root)
}

fn turn_to(world: &mut World, root: Entity, rotation: impl Fn(Quat) -> Quat) {
    let t = world.get_mut::<Transform3D>(root).expect("a transform");
    t.rotation = rotation(t.rotation);
}

#[test]
fn a_game_yawing_under_a_tilting_layer_yaws_what_is_under_on_its_own_axis() {
    let tilted = Quat::from_rotation_x(40f32.to_radians());
    let (mut world, mut clips, root) = layered(tilt(40.0), LayerBlend::Additive, 1.0);
    assert!(rotation_apart(transform(&world, root).rotation, tilted) < 1e-5);
    // The game yaws the entity on its own axes, 1° a frame for 90 frames:
    // what is under the layer yaws to 90°, and the layer's tilt stays on
    // top of it.
    for _ in 0..90 {
        turn_to(&mut world, root, |q| q * about_y(1.0));
        advance(&mut world, &mut clips, 1.0 / 60.0);
    }
    let shown = transform(&world, root).rotation;
    let off = rotation_apart(shown, tilted * about_y(90.0));
    assert!(off < 1e-4, "{off} rad from 40° of tilt over 90° of yaw");
}

#[test]
fn a_rotation_set_outright_under_an_additive_layer_is_shown_the_next_frame() {
    let (mut world, mut clips, root) = layered(tilt(40.0), LayerBlend::Additive, 1.0);
    let set = Quat::from_euler(voltra_render::glam::EulerRot::YXZ, 1.1, -0.4, 0.3);
    turn_to(&mut world, root, |_| set);
    advance(&mut world, &mut clips, 1.0 / 60.0);
    let off = rotation_apart(transform(&world, root).rotation, set);
    assert!(off < 1e-5, "one frame: {off} rad from what was set");
}

#[test]
fn a_rotation_set_outright_each_frame_under_an_override_layer_settles_on_it() {
    // As in 2D: an override below one shows the game's value once it has
    // carried the difference under, halving the error a frame at 0.5.
    let (mut world, mut clips, root) = layered(tilt(40.0), LayerBlend::Override, 0.5);
    let set = Quat::from_euler(voltra_render::glam::EulerRot::YXZ, 1.1, -0.4, 0.3);
    let mut first = None;
    for _ in 0..40 {
        turn_to(&mut world, root, |_| set);
        advance(&mut world, &mut clips, 1.0 / 60.0);
        first.get_or_insert(rotation_apart(transform(&world, root).rotation, set));
    }
    let off = rotation_apart(transform(&world, root).rotation, set);
    assert!(off < 1e-4, "settled, no drift: {off} rad");
    assert!(first.is_some_and(|f| f > off), "converging");
}
