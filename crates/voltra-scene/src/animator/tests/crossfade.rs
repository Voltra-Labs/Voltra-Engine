//! Transitions that crossfade, and exit times that count loops.

use super::*;

/// Idle and run, each fading into the other over a second.
const FADING: &str = r#"(
    parameters: [(name: "speed", value: Float(0.0))],
    start: "idle",
    states: [
        (name: "idle", clip: Path("idle.anim.ron")),
        (name: "run", clip: Path("run.anim.ron")),
    ],
    transitions: [
        (from: Some("idle"), to: "run", conditions: [Greater("speed", 0.1)], duration: 1.0),
        (from: Some("run"), to: "idle", conditions: [Less("speed", 0.1)], duration: 1.0),
    ],
)"#;

const AT_0: &str = "(looping: true, length: 1.0, tracks: [(property: Translation, keys: [(time: 0.0, value: Vec2((0.0, 0.0)))])])";
const AT_4: &str = "(looping: true, length: 1.0, tracks: [(property: Translation, keys: [(time: 0.0, value: Vec2((4.0, 0.0)))])])";

fn holds(machine: &str) -> Rig {
    Rig::with_clips(machine, &[("idle", AT_0), ("run", AT_4)])
}

impl Rig {
    fn x(&self) -> f32 {
        self.world
            .get::<Transform2D>(self.hero)
            .expect("a transform")
            .translation
            .x
    }
}

#[test]
fn a_transition_with_a_duration_crossfades_its_clips() {
    let mut rig = holds(FADING);
    rig.frame(0.0);
    rig.animator().set_float("speed", 3.0);
    rig.frame(0.5);
    assert_eq!(
        rig.state().as_deref(),
        Some("run"),
        "the state moves at once"
    );
    assert!((rig.x() - 2.0).abs() < 1e-5, "half way: {}", rig.x());
    rig.frame(0.5);
    assert_eq!(rig.x(), 4.0);
}

#[test]
fn a_transition_fired_mid_fade_fades_from_the_mix() {
    let mut rig = holds(FADING);
    rig.frame(0.0);
    rig.animator().set_float("speed", 3.0);
    rig.frame(0.5);
    rig.animator().set_float("speed", 0.0);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("idle"));
    assert!((rig.x() - 2.0).abs() < 1e-5, "no jump: {}", rig.x());
    rig.frame(0.5);
    assert!((rig.x() - 1.0).abs() < 1e-5, "{}", rig.x());
    rig.frame(0.5);
    assert_eq!(rig.x(), 0.0);
}

#[test]
fn a_duration_of_zero_cuts_as_a_transition_always_did() {
    let cutting = FADING.replace("duration: 1.0", "duration: 0.0");
    let mut rig = holds(&cutting);
    rig.frame(0.0);
    rig.animator().set_float("speed", 3.0);
    rig.frame(0.1);
    assert_eq!(rig.x(), 4.0);
    let player = rig.world.get::<AnimationPlayer>(rig.hero).expect("p");
    assert_eq!(player.fade_weight(), None);

    let unsaid: StateMachine =
        ron::from_str(&FADING.replace(", duration: 1.0", "")).expect("reads");
    assert!(unsaid.transitions.iter().all(|t| t.duration == 0.0));
    let text = ron::to_string(&unsaid).expect("writes");
    assert!(!text.contains("duration"), "a cut is not written: {text}");
}

#[test]
fn a_duration_that_is_not_a_time_cuts() {
    let machine: StateMachine =
        ron::from_str(&FADING.replace("duration: 1.0", "duration: -2.0")).expect("reads");
    let machine = machine.checked("test");
    assert_eq!(machine.transitions.len(), 2, "kept, as cuts");
    assert!(machine.transitions.iter().all(|t| t.duration == 0.0));
}

#[test]
fn a_machine_reloaded_mid_fade_keeps_the_state_and_the_fade() {
    let mut rig = holds(FADING);
    rig.frame(0.0);
    rig.animator().set_float("speed", 3.0);
    rig.frame(0.25);
    let root = rig.machines.root().to_path_buf();
    std::fs::write(
        root.join("hero.animator.ron"),
        FADING.replace("duration: 1.0", "duration: 4.0"),
    )
    .expect("writes");
    assert!(rig.machines.reload(&path("hero.animator.ron")));
    rig.frame(0.25);
    assert_eq!(rig.state().as_deref(), Some("run"));
    assert!(
        (rig.x() - 2.0).abs() < 1e-5,
        "the fade begun runs on: {}",
        rig.x()
    );
}

#[test]
fn an_entity_despawned_mid_fade_is_simply_gone() {
    let mut rig = holds(FADING);
    rig.frame(0.0);
    rig.animator().set_float("speed", 3.0);
    rig.frame(0.25);
    assert!(rig.world.despawn(rig.hero));
    rig.frame(0.25);
    assert_eq!(rig.world.query::<AnimationPlayer>().count(), 0);
}

#[test]
fn an_exit_time_above_one_counts_loops() {
    let looping = r#"(
        states: [
            (name: "idle", clip: Path("idle.anim.ron")),
            (name: "run", clip: Path("run.anim.ron")),
        ],
        transitions: [
            (from: Some("idle"), to: "run", exit_time: Some(2.5)),
            (from: Some("run"), to: "idle", exit_time: Some(1.0)),
        ],
    )"#;
    let mut rig = holds(looping);
    rig.frame(1.0);
    rig.frame(1.0);
    rig.frame(0.4);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("idle"), "two loops and 0.4");
    rig.frame(0.2);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("run"), "two and a half");

    // 1.0 on a clip that loops is one whole loop — not never.
    rig.frame(0.9);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("run"));
    rig.frame(0.2);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("idle"));
}

#[test]
fn an_exit_time_above_one_on_a_clip_that_plays_once_is_its_end() {
    let once = r#"(
        start: "land",
        states: [
            (name: "land", clip: Path("land.anim.ron")),
            (name: "idle", clip: Path("idle.anim.ron")),
            (name: "blink", clip: Path("blink.anim.ron")),
        ],
        transitions: [
            (from: Some("land"), to: "idle", exit_time: Some(2.0)),
            (from: Some("idle"), to: "blink", exit_time: Some(1.0)),
            (from: Some("blink"), to: "land", exit_time: Some(3.0)),
        ],
    )"#;
    // `land` plays once, half a second; `blink` is one key, of no length.
    let blink =
        "(looping: true, tracks: [(property: Rotation, keys: [(time: 0.0, value: Float(1.0))])])";
    let mut rig = Rig::with_clips(once, &[("land", ONCE), ("idle", LOOP), ("blink", blink)]);
    rig.frame(0.3);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("land"), "not yet at its end");
    rig.frame(0.3);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("idle"), "its end, not never");

    rig.frame(1.1);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("blink"));
    rig.frame(0.0);
    assert_eq!(
        rig.state().as_deref(),
        Some("land"),
        "a clip of no length has nothing to wait for"
    );
}
