//! Layers over a player's motion: masked overrides, additive changes, and
//! weights that are continuous down to zero and leave nothing behind.

use voltra_ecs::World;
use voltra_render::glam::Vec2;

use super::*;
use crate::Transform;

/// The motion: the player at x 1, its arm turned 0.5, its leg 0.25. The
/// hand is not animated.
const BASE: &str = r#"(looping: true, length: 1.0, tracks: [
    (property: Translation, keys: [(time: 0.0, value: Vec2((1.0, 0.0)))]),
    (target: "arm", property: Rotation, keys: [(time: 0.0, value: Float(0.5))]),
    (target: "leg", property: Rotation, keys: [(time: 0.0, value: Float(0.25))]),
])"#;

/// A wave: the arm at 2, its hand at 1, the leg at 3 — and the root moved,
/// which a mask to the arm keeps out.
const WAVE: &str = r#"(looping: true, length: 1.0, tracks: [
    (property: Translation, keys: [(time: 0.0, value: Vec2((9.0, 9.0)))]),
    (target: "arm", property: Rotation, keys: [(time: 0.0, value: Float(2.0))]),
    (target: "arm/hand", property: Rotation, keys: [(time: 0.0, value: Float(1.0))]),
    (target: "leg", property: Rotation, keys: [(time: 0.0, value: Float(3.0))]),
])"#;

/// A breath: from its first frame, the scale grows by 0.2 and the root
/// rises by 1 over a second, then back.
const BREATH: &str = r#"(looping: true, length: 2.0, tracks: [
    (property: Scale, keys: [(time: 0.0, value: Vec2((1.0, 1.0))), (time: 1.0, value: Vec2((1.2, 1.2))), (time: 2.0, value: Vec2((1.0, 1.0)))]),
    (property: Translation, keys: [(time: 0.0, value: Vec2((5.0, 5.0))), (time: 1.0, value: Vec2((5.0, 6.0))), (time: 2.0, value: Vec2((5.0, 5.0)))]),
])"#;

struct Body {
    world: World,
    clips: AnimationClips,
    root: Entity,
    arm: Entity,
    hand: Entity,
    leg: Entity,
}

fn named(world: &mut World, name: &str, parent: Entity) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, Transform::default().with_rotation(-1.0));
    set_parent(world, entity, parent).expect("a plain reparent");
    entity
}

fn body() -> Body {
    let root_dir = voltra_testkit::scratch_root();
    for (name, text) in [("base", BASE), ("wave", WAVE), ("breath", BREATH)] {
        std::fs::write(root_dir.join(format!("{name}.anim.ron")), text).expect("writes");
    }
    let mut world = World::new();
    let root = world.spawn();
    world.insert(root, SceneId::new());
    world.insert(root, Transform::default());
    world.insert(root, AnimationPlayer::new(path("base.anim.ron")));
    let arm = named(&mut world, "arm", root);
    let hand = named(&mut world, "hand", arm);
    let leg = named(&mut world, "leg", root);
    resolve_parents(&mut world);
    Body {
        world,
        clips: AnimationClips::new(root_dir),
        root,
        arm,
        hand,
        leg,
    }
}

impl Body {
    fn layer(&mut self, clip: &str, blend: LayerBlend, mask: &[&str], weight: f32) {
        let mut layer = PlayerLayer::new(blend);
        layer.player.play(path(clip));
        layer.mask = mask.iter().map(|m| (*m).to_owned()).collect();
        layer.weight = weight;
        self.player().layers_mut().push(layer);
    }

    fn player(&mut self) -> &mut AnimationPlayer {
        self.world
            .get_mut::<AnimationPlayer>(self.root)
            .expect("a player")
    }

    fn weight(&mut self, weight: f32) {
        self.player().layers_mut()[0].weight = weight;
    }

    fn frame(&mut self, delta: f32) {
        advance(&mut self.world, &mut self.clips, delta);
    }

    fn transform(&self, entity: Entity) -> Transform {
        *self.world.get::<Transform>(entity).expect("a transform")
    }

    fn turn(&self, entity: Entity) -> f32 {
        self.transform(entity).rotation
    }
}

#[test]
fn an_override_layer_masked_to_an_arm_moves_the_arm_and_its_hand_only() {
    let mut body = body();
    body.layer("wave.anim.ron", LayerBlend::Override, &["arm"], 1.0);
    body.frame(0.1);
    assert_eq!(body.turn(body.arm), 2.0, "the layer's");
    assert_eq!(body.turn(body.hand), 1.0, "under the arm: the layer's");
    assert_eq!(body.turn(body.leg), 0.25, "masked out: the motion's");
    assert_eq!(body.transform(body.root).translation, Vec2::new(1.0, 0.0));

    body.weight(0.5);
    body.frame(0.1);
    assert!(
        (body.turn(body.arm) - 1.25).abs() < 1e-6,
        "{}",
        body.turn(body.arm)
    );
    assert!(
        (body.turn(body.hand) - 0.0).abs() < 1e-6,
        "half way from what it was before the layers (-1) to 1: {}",
        body.turn(body.hand)
    );
}

#[test]
fn an_additive_layer_adds_its_change_from_its_first_frame() {
    let mut body = body();
    body.layer("breath.anim.ron", LayerBlend::Additive, &[], 0.5);
    body.frame(1.0);
    let t = body.transform(body.root);
    // The motion holds x at 1; the breath rose 1 from its start, by half.
    assert!(
        (t.translation - Vec2::new(1.0, 0.5)).length() < 1e-5,
        "{}",
        t.translation
    );
    // Nothing under it animates the scale: the authored 1, plus 0.2 by half.
    assert!((t.scale - Vec2::splat(1.1)).length() < 1e-5, "{}", t.scale);
    for _ in 0..50 {
        body.frame(0.0);
    }
    let again = body.transform(body.root).scale;
    assert!(
        (again - Vec2::splat(1.1)).length() < 1e-5,
        "no creep: {again}"
    );
}

#[test]
fn a_layer_at_weight_zero_writes_nothing() {
    let mut plain = body();
    plain.frame(0.3);
    let mut layered = body();
    layered.layer("wave.anim.ron", LayerBlend::Override, &[], 0.0);
    layered.frame(0.3);
    for (a, b) in [
        (plain.root, layered.root),
        (plain.arm, layered.arm),
        (plain.hand, layered.hand),
        (plain.leg, layered.leg),
    ] {
        assert_eq!(plain.transform(a), layered.transform(b));
    }
}

#[test]
fn a_property_only_a_layer_animates_is_continuous_near_zero_and_handed_back() {
    let mut body = body();
    body.layer("wave.anim.ron", LayerBlend::Override, &["arm"], 0.01);
    for _ in 0..200 {
        body.frame(1.0 / 60.0);
    }
    // From -1 (as authored) towards 1 by a hundredth — not creeping on.
    assert!(
        (body.turn(body.hand) - (-0.98)).abs() < 1e-5,
        "{}",
        body.turn(body.hand)
    );
    body.weight(0.5);
    body.frame(0.0);
    assert!((body.turn(body.hand) - 0.0).abs() < 1e-5);
    body.weight(1.0);
    body.frame(0.0);
    assert_eq!(body.turn(body.hand), 1.0);
    body.weight(0.0);
    body.frame(0.0);
    assert_eq!(body.turn(body.hand), -1.0, "handed back as it was");
    assert_eq!(body.turn(body.arm), 0.5, "the motion's again");
}

#[test]
fn something_else_writing_a_covered_property_is_what_the_layer_goes_over() {
    let mut body = body();
    body.layer("wave.anim.ron", LayerBlend::Override, &["arm"], 0.5);
    body.frame(0.0);
    assert!((body.turn(body.hand) - 0.0).abs() < 1e-6);
    // A game aims the hand.
    if let Some(t) = body.world.get_mut::<Transform>(body.hand) {
        t.rotation = 3.0;
    }
    body.frame(0.0);
    assert!(
        (body.turn(body.hand) - 2.0).abs() < 1e-6,
        "{}",
        body.turn(body.hand)
    );
    body.player().layers_mut().clear();
    body.frame(0.0);
    assert_eq!(
        body.turn(body.hand),
        3.0,
        "a layer taken out hands back the aim"
    );
    assert!(!body.player().is_layered(), "and is done");
}

#[test]
fn a_layer_played_once_holds_its_last_frame() {
    let once = r#"(length: 1.0, tracks: [(target: "leg", property: Rotation, keys: [(time: 0.0, value: Float(0.0)), (time: 1.0, value: Float(1.0))])])"#;
    let mut body = body();
    std::fs::write(body.clips.root().join("once.anim.ron"), once).expect("writes");
    body.layer("once.anim.ron", LayerBlend::Additive, &[], 1.0);
    body.frame(2.0);
    assert!(!body.player().layers()[0].player.playing, "ended");
    body.frame(0.5);
    assert!(
        (body.turn(body.leg) - 1.25).abs() < 1e-6,
        "held: {}",
        body.turn(body.leg)
    );
}

#[test]
fn an_additive_fade_into_a_clip_without_a_property_mixes_towards_no_change() {
    let mut body = body();
    body.layer("breath.anim.ron", LayerBlend::Additive, &[], 1.0);
    body.frame(1.0);
    // The wave does not animate the scale: fading into it, the breath's
    // change fades out rather than holding whole.
    body.player().layers_mut()[0]
        .player
        .cross_fade(path("wave.anim.ron"), 1.0);
    body.frame(0.5);
    let scale = body.transform(body.root).scale;
    // Half way: the breath's change at 1.5s (0.1) mixed half with none.
    assert!((scale.x - 1.05).abs() < 1e-5, "{scale}");
}
