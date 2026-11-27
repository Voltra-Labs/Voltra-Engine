//! Layers: machines of their own over the machine's states, masked, weighed
//! and blended — and files of one layer reading and writing as they did.

use super::*;
use crate::hierarchy::{resolve_parents, set_parent};
use crate::keyframes::LayerBlend;
use crate::{Name, SceneId};
use voltra_render::glam::Vec2;

/// Layer 0 holds the hero still; over it, `upper` waves the arm on a
/// trigger and goes back to resting once the wave has played, and `breath`
/// adds a rise to the hero at half weight.
const LAYERED: &str = r#"(
    parameters: [(name: "wave", value: Trigger(false))],
    states: [(name: "stand", clip: Path("stand.anim.ron"))],
    transitions: [],
    layers: [
        (
            name: "upper",
            mask: ["arm"],
            start: "rest",
            states: [
                (name: "rest"),
                (name: "wave", clip: Path("wave.anim.ron")),
            ],
            transitions: [
                (from: Some("rest"), to: "wave", conditions: [If("wave")]),
                (from: Some("wave"), to: "rest", exit_time: Some(1.0)),
            ],
        ),
        (
            name: "breath",
            weight: 0.5,
            blend: Additive,
            states: [(name: "in", clip: Path("breath.anim.ron"))],
        ),
    ],
)"#;

/// The hero at x 1, its arm at 0.5.
const STAND: &str = r#"(looping: true, length: 1.0, tracks: [
    (property: Translation, keys: [(time: 0.0, value: Vec2((1.0, 0.0)))]),
    (target: "arm", property: Rotation, keys: [(time: 0.0, value: Float(0.5))]),
])"#;

/// Played once: the arm to 2 — and the hero moved, which the mask keeps out.
const WAVE: &str = r#"(length: 0.5, tracks: [
    (property: Translation, keys: [(time: 0.0, value: Vec2((7.0, 7.0)))]),
    (target: "arm", property: Rotation, keys: [(time: 0.0, value: Float(2.0))]),
])"#;

/// From its first frame, the hero rises 2 over a second.
const BREATH: &str = r#"(looping: true, length: 2.0, tracks: [
    (property: Translation, keys: [(time: 0.0, value: Vec2((0.0, 0.0))), (time: 1.0, value: Vec2((0.0, 2.0))), (time: 2.0, value: Vec2((0.0, 0.0)))]),
])"#;

fn layered() -> (Rig, Entity) {
    let mut rig = Rig::with_clips(
        LAYERED,
        &[("stand", STAND), ("wave", WAVE), ("breath", BREATH)],
    );
    rig.world.insert(rig.hero, SceneId::new());
    let arm = rig.world.spawn();
    rig.world.insert(arm, SceneId::new());
    rig.world.insert(arm, Name::new("arm"));
    rig.world.insert(arm, Transform2D::default());
    set_parent(&mut rig.world, arm, rig.hero).expect("a plain reparent");
    resolve_parents(&mut rig.world);
    (rig, arm)
}

impl Rig {
    fn layer_state(&self, layer: &str) -> Option<String> {
        let animator = self.world.get::<Animator>(self.hero).expect("an animator");
        animator.layer_state(layer).map(str::to_owned)
    }

    fn transform(&self, entity: Entity) -> Transform2D {
        *self.world.get::<Transform2D>(entity).expect("a transform")
    }
}

#[test]
fn each_layer_runs_its_own_states_over_the_machine_s() {
    let (mut rig, arm) = layered();
    rig.frame(0.5);
    assert_eq!(rig.state().as_deref(), Some("stand"));
    assert_eq!(rig.layer_state("upper").as_deref(), Some("rest"));
    assert_eq!(rig.layer_state("breath").as_deref(), Some("in"));
    let player = rig.world.get::<AnimationPlayer>(rig.hero).expect("p");
    assert_eq!(player.layers().len(), 2);
    assert_eq!(player.layers()[1].blend, LayerBlend::Additive);
    assert_eq!(player.layers()[0].mask, ["arm"]);
    // Resting plays nothing: the arm is layer 0's; the breath rose 1 of 2,
    // at half weight.
    assert_eq!(rig.transform(arm).rotation, 0.5);
    let hero = rig.transform(rig.hero).translation;
    assert!((hero - Vec2::new(1.0, 0.5)).length() < 1e-5, "{hero}");

    rig.animator().trigger("wave");
    rig.frame(0.1);
    assert_eq!(rig.layer_state("upper").as_deref(), Some("wave"));
    assert_eq!(
        rig.animator().parameter("wave"),
        Some(ParamValue::Trigger(false)),
        "spent"
    );
    assert_eq!(
        rig.transform(arm).rotation,
        2.0,
        "the layer's, through its mask"
    );
    assert!(
        (rig.transform(rig.hero).translation.x - 1.0).abs() < 1e-6,
        "masked out"
    );

    rig.frame(0.5);
    rig.frame(0.0);
    assert_eq!(
        rig.layer_state("upper").as_deref(),
        Some("rest"),
        "the wave ended"
    );
    assert_eq!(rig.transform(arm).rotation, 0.5);
}

#[test]
fn a_game_sets_a_layer_s_weight_over_the_file_s() {
    let (mut rig, _) = layered();
    rig.frame(1.0);
    assert!((rig.transform(rig.hero).translation.y - 1.0).abs() < 1e-5);
    rig.animator().set_layer_weight("breath", 0.0);
    rig.frame(0.0);
    assert_eq!(
        rig.transform(rig.hero).translation,
        Vec2::new(1.0, 0.0),
        "no rise at all"
    );
    rig.animator().set_layer_weight("breath", f32::NAN);
    assert_eq!(rig.animator().layer_weight("breath"), Some(0.0), "ignored");
    rig.animator().restart();
    rig.frame(1.0);
    assert!(
        (rig.transform(rig.hero).translation.y - 1.0).abs() < 1e-5,
        "restart forgets it"
    );
}

#[test]
fn a_reload_that_moves_a_layer_restarts_it_and_one_that_drops_it_takes_it_out() {
    let (mut rig, arm) = layered();
    rig.frame(0.0);
    rig.animator().trigger("wave");
    rig.frame(0.1);
    assert_eq!(rig.transform(arm).rotation, 2.0);
    let root = rig.machines.root().to_path_buf();
    let swapped = LAYERED.replace("name: \"upper\"", "name: \"top\"");
    std::fs::write(root.join("hero.animator.ron"), swapped).expect("writes");
    assert!(rig.machines.reload(&path("hero.animator.ron")));
    rig.frame(0.0);
    assert_eq!(
        rig.layer_state("top").as_deref(),
        Some("rest"),
        "started over"
    );
    let one = LAYERED.split("layers:").next().expect("a head").to_owned() + ")";
    std::fs::write(root.join("hero.animator.ron"), one).expect("writes");
    assert!(rig.machines.reload(&path("hero.animator.ron")));
    rig.frame(0.0);
    let player = rig.world.get::<AnimationPlayer>(rig.hero).expect("p");
    assert!(player.layers().is_empty());
    assert_eq!(rig.transform(arm).rotation, 0.5);
}

#[test]
fn a_machine_of_one_layer_reads_and_writes_as_it_always_did() {
    let machine: StateMachine = ron::from_str(MACHINE).expect("reads");
    assert!(machine.layers.is_empty());
    let text =
        ron::ser::to_string_pretty(&machine, ron::ser::PrettyConfig::default()).expect("writes");
    assert!(!text.contains("layers"), "{text}");
    let back: StateMachine = ron::from_str(&text).expect("reads back");
    assert_eq!(back, machine);

    let layered: StateMachine = ron::from_str(LAYERED).expect("reads");
    let text =
        ron::ser::to_string_pretty(&layered, ron::ser::PrettyConfig::default()).expect("writes");
    assert!(
        !text.contains("weight: 1.0") && text.contains("weight: 0.5"),
        "{text}"
    );
    assert!(!text.contains("Override") && text.contains("Additive"));
    let back: StateMachine = ron::from_str(&text).expect("reads back");
    assert_eq!(back, layered);
}

#[test]
fn a_layer_is_checked_against_its_own_states_and_keeps_its_weight_as_written() {
    let text = r#"(
        parameters: [(name: "go", value: Bool(false))],
        states: [(name: "a", clip: Path("a.anim.ron"))],
        layers: [(
            name: "over",
            weight: 4.0,
            start: "nowhere",
            states: [(name: "empty"), (name: "empty"), (name: "b", clip: Path("b.anim.ron"))],
            transitions: [
                (to: "a", conditions: []),
                (to: "b", conditions: [If("go")], duration: -1.0),
                (to: "b", conditions: [Greater("go", 1.0)]),
            ],
        )],
    )"#;
    let machine = ron::from_str::<StateMachine>(text)
        .expect("reads")
        .checked("test");
    let layer = &machine.layers[0];
    assert_eq!(layer.weight, 4.0, "clamped when played, not loaded");
    assert_eq!(layer.states.len(), 2, "the second `empty` dropped");
    assert!(layer.start.is_empty());
    assert_eq!(
        layer.transitions.len(),
        1,
        "`a` is layer 0's; `go` is no float"
    );
    assert_eq!(layer.transitions[0].duration, 0.0, "a cut");
}

#[test]
fn a_save_writes_the_edited_layer_settings_and_keeps_what_loading_dropped() {
    let (mut rig, _) = layered();
    let file = path("hero.animator.ron");
    let root = rig.machines.root().to_path_buf();
    let with_typo = LAYERED.replace(
        "(from: Some(\"wave\"), to: \"rest\", exit_time: Some(1.0)),",
        "(from: Some(\"wave\"), to: \"rest\", exit_time: Some(1.0)),\n(to: \"gone\"),",
    );
    std::fs::write(root.join("hero.animator.ron"), with_typo).expect("writes");
    let handle = rig.machines.load(&file);
    let machine = rig.machines.get_mut(handle);
    assert_eq!(machine.layers[0].transitions.len(), 2, "the typo dropped");
    machine.layers[0].weight = 0.25;
    machine.layers[0].mask = vec!["arm/hand".to_owned()];
    machine.layers[1].blend = LayerBlend::Override;
    machine.layers[0].transitions[0].duration = 0.2;
    rig.machines.save_edits(&file).expect("saves");
    let saved: StateMachine =
        ron::from_str(&std::fs::read_to_string(root.join("hero.animator.ron")).expect("reads"))
            .expect("parses");
    let upper = &saved.layers[0];
    assert_eq!(
        (upper.weight, upper.mask.as_slice()),
        (0.25, &["arm/hand".to_owned()][..])
    );
    assert_eq!(saved.layers[1].blend, LayerBlend::Override);
    assert_eq!(
        upper.transitions.len(),
        3,
        "the typo is the author's to fix"
    );
    assert_eq!(upper.transitions[0].duration, 0.2);
}

#[test]
fn an_animator_given_a_machine_of_one_layer_takes_its_layers_out() {
    let (mut rig, arm) = layered();
    rig.frame(0.0);
    rig.animator().trigger("wave");
    rig.frame(0.1);
    assert_eq!(rig.transform(arm).rotation, 2.0);
    let root = rig.machines.root().to_path_buf();
    std::fs::write(root.join("one.animator.ron"), MACHINE).expect("writes");
    std::fs::write(root.join("idle.anim.ron"), STAND).expect("writes");
    // What the inspector does when the path changes.
    let animator = rig.animator();
    animator.machine = Some(path("one.animator.ron"));
    animator.restart();
    rig.frame(0.1);
    let player = rig.world.get::<AnimationPlayer>(rig.hero).expect("p");
    assert!(
        player.layers().is_empty(),
        "the old machine's layers are gone"
    );
    assert_eq!(
        rig.transform(arm).rotation,
        0.5,
        "the idle's arm, no wave over it"
    );
}

#[test]
fn an_animator_removed_takes_its_layers_out_and_a_game_s_own_stay() {
    let (mut rig, arm) = layered();
    rig.frame(0.0);
    rig.animator().trigger("wave");
    rig.frame(0.1);
    rig.world.remove::<Animator>(rig.hero);
    rig.frame(0.1);
    let player = rig.world.get::<AnimationPlayer>(rig.hero).expect("p");
    assert!(player.layers().is_empty());
    assert_eq!(rig.transform(arm).rotation, 0.5, "handed back");

    // A game's own layer on a player with no animator is its own.
    let player = rig.world.get_mut::<AnimationPlayer>(rig.hero).expect("p");
    let mut own = crate::keyframes::PlayerLayer::default();
    own.player.play(path("wave.anim.ron"));
    player.layers_mut().push(own);
    rig.frame(0.1);
    let player = rig.world.get::<AnimationPlayer>(rig.hero).expect("p");
    assert_eq!(player.layers().len(), 1);
}

#[test]
fn a_weight_set_by_a_name_two_layers_share_reaches_the_first() {
    let twice = LAYERED.replace("name: \"breath\"", "name: \"upper\"");
    let mut rig = Rig::with_clips(
        &twice,
        &[("stand", STAND), ("wave", WAVE), ("breath", BREATH)],
    );
    rig.frame(0.0);
    rig.animator().set_layer_weight("upper", 0.25);
    rig.frame(0.0);
    let player = rig.world.get::<AnimationPlayer>(rig.hero).expect("p");
    let weights: Vec<f32> = player.layers().iter().map(|l| l.weight).collect();
    assert_eq!(weights, [0.25, 0.5], "the second keeps the file's");
    assert_eq!(
        rig.layer_state("upper").as_deref(),
        Some("rest"),
        "the first's state"
    );
}
