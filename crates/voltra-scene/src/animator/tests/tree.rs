//! States that play blend trees: weights by parameters, synced clips,
//! crossfades with clip states, reloads.

use super::*;
use crate::keyframes::SyncedClips;

/// A clip holding the hero at `(x, y)`.
fn at(x: f32, y: f32) -> String {
    format!("(looping: true, length: 1.0, tracks: [(property: Translation, keys: [(time: 0.0, value: Vec2(({x}, {y})))])])")
}

/// A looping clip `length` long, sweeping x from 0 to `to`.
fn sweep(length: f32, to: f32) -> String {
    format!("(looping: true, length: {length}, tracks: [(property: Translation, keys: [(time: 0.0, value: Vec2((0.0, 0.0))), (time: {length}, value: Vec2(({to}, 0.0)))])])")
}

fn rig(machine: &str, clips: &[(&str, String)]) -> Rig {
    let clips: Vec<(&str, &str)> = clips.iter().map(|(n, t)| (*n, t.as_str())).collect();
    Rig::with_clips(machine, &clips)
}

impl Rig {
    fn at(&self) -> [f32; 2] {
        let t = self.world.get::<Transform>(self.hero).expect("a transform");
        [t.translation.x, t.translation.y]
    }

    fn synced(&self) -> Option<SyncedClips> {
        self.world
            .get::<AnimationPlayer>(self.hero)
            .and_then(AnimationPlayer::synced)
            .cloned()
    }
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4
}

const LINE: &str = r#"(
    parameters: [(name: "speed", value: Float(0.0))],
    states: [
        (name: "move", tree: Blend1D(parameter: "speed", children: [
            (threshold: 0.0, clip: Path("idle.anim.ron")),
            (threshold: 2.0, clip: Path("walk.anim.ron")),
            (threshold: 4.0, clip: Path("run.anim.ron")),
        ])),
    ],
)"#;

fn line() -> Rig {
    rig(
        LINE,
        &[
            ("idle", at(0.0, 0.0)),
            ("walk", at(4.0, 0.0)),
            ("run", at(8.0, 0.0)),
        ],
    )
}

#[test]
fn a_1d_tree_mixes_its_clips_by_its_parameter_every_frame() {
    let mut rig = line();
    for (speed, x) in [
        (0.0, 0.0),
        (2.0, 4.0),
        (1.0, 2.0),
        (3.0, 6.0),
        (9.0, 8.0),
        (-1.0, 0.0),
    ] {
        rig.animator().set_float("speed", speed);
        rig.frame(0.1);
        assert!(near(rig.at(), [x, 0.0]), "speed {speed}: {:?}", rig.at());
    }
    assert_eq!(rig.state().as_deref(), Some("move"), "no transition");
    assert_eq!(rig.clip(), None, "a blend, not a clip");
}

#[test]
fn a_2d_tree_mixes_the_triangle_around_the_input() {
    let machine = r#"(
        parameters: [(name: "x", value: Float(0.0)), (name: "y", value: Float(0.0))],
        states: [
            (name: "walk", tree: Blend2D(x: "x", y: "y", children: [
                (position: (0.0, 0.0), clip: Path("idle.anim.ron")),
                (position: (1.0, 0.0), clip: Path("right.anim.ron")),
                (position: (0.0, 1.0), clip: Path("up.anim.ron")),
                (position: (-1.0, 0.0), clip: Path("left.anim.ron")),
                (position: (0.0, -1.0), clip: Path("down.anim.ron")),
            ])),
        ],
    )"#;
    let mut rig = rig(
        machine,
        &[
            ("idle", at(0.0, 0.0)),
            ("right", at(4.0, 0.0)),
            ("up", at(0.0, 4.0)),
            ("left", at(-4.0, 0.0)),
            ("down", at(0.0, -4.0)),
        ],
    );
    for (input, expected) in [
        ([1.0, 0.0], [4.0, 0.0]),   // at a child
        ([0.5, 0.5], [2.0, 2.0]),   // on an edge
        ([0.25, 0.25], [1.0, 1.0]), // in a triangle
        ([-0.2, -0.6], [-0.8, -2.4]),
        ([2.0, 2.0], [2.0, 2.0]), // outside: the nearest edge
    ] {
        rig.animator().set_float("x", input[0]);
        rig.animator().set_float("y", input[1]);
        rig.frame(0.1);
        assert!(near(rig.at(), expected), "{input:?}: {:?}", rig.at());
    }
}

#[test]
fn a_tree_s_clips_keep_one_phase_and_its_exit_time_counts_its_loops() {
    let machine = r#"(
        parameters: [(name: "speed", value: Float(1.0))],
        states: [
            (name: "move", tree: Blend1D(parameter: "speed", children: [
                (threshold: 0.0, clip: Path("walk.anim.ron")),
                (threshold: 2.0, clip: Path("run.anim.ron")),
            ])),
            (name: "rest", clip: Path("idle.anim.ron")),
        ],
        transitions: [(from: Some("move"), to: "rest", exit_time: Some(2.0))],
    )"#;
    let mut rig = rig(
        machine,
        &[
            ("walk", sweep(1.0, 4.0)),
            ("run", sweep(0.5, 8.0)),
            ("idle", at(9.0, 0.0)),
        ],
    );
    rig.frame(0.0);
    // Half walk, half run: three quarters of a second a loop. Half way
    // through, the walk is at 2 and the run at 4.
    rig.frame(0.375);
    assert!(near(rig.at(), [3.0, 0.0]), "{:?}", rig.at());
    let set = rig.synced().expect("a blend");
    assert!((set.phase() - 0.5).abs() < 1e-5);
    // Two loops are a second and a half.
    rig.frame(1.0);
    assert_eq!(rig.state().as_deref(), Some("move"));
    rig.frame(0.2);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("rest"), "two loops played");
}

#[test]
fn an_empty_tree_warns_and_plays_nothing() {
    let machine = r#"(
        parameters: [(name: "speed", value: Float(0.0))],
        states: [(name: "move", tree: Blend1D(parameter: "speed", children: []))],
    )"#;
    let checked: StateMachine = ron::from_str(machine).expect("reads");
    let problems = checked.motion_problems(&checked.states[0]);
    assert!(
        problems.iter().any(|p| p.contains("no clips")),
        "{problems:?}"
    );
    let mut rig = rig(machine, &[]);
    rig.world
        .get_mut::<Transform>(rig.hero)
        .expect("t")
        .translation
        .x = 5.0;
    rig.frame(0.1);
    rig.frame(0.1);
    assert_eq!(rig.state().as_deref(), Some("move"));
    let player = rig.world.get::<AnimationPlayer>(rig.hero).expect("added");
    assert!(player.clip.is_none() && player.synced().is_none());
    assert_eq!(rig.at(), [5.0, 0.0], "nothing written");
}

#[test]
fn a_tree_crossfades_into_a_clip_and_a_clip_into_a_tree() {
    let machine = r#"(
        parameters: [(name: "speed", value: Float(2.0)), (name: "air", value: Bool(false))],
        states: [
            (name: "move", tree: Blend1D(parameter: "speed", children: [
                (threshold: 0.0, clip: Path("idle.anim.ron")),
                (threshold: 2.0, clip: Path("walk.anim.ron")),
            ])),
            (name: "jump", clip: Path("jump.anim.ron")),
        ],
        transitions: [
            (from: Some("move"), to: "jump", conditions: [If("air")], duration: 1.0),
            (from: Some("jump"), to: "move", conditions: [IfNot("air")], duration: 1.0),
        ],
    )"#;
    let mut rig = rig(
        machine,
        &[
            ("idle", at(0.0, 0.0)),
            ("walk", at(4.0, 0.0)),
            ("jump", at(8.0, 0.0)),
        ],
    );
    rig.frame(0.1);
    assert!(near(rig.at(), [4.0, 0.0]), "{:?}", rig.at());
    rig.animator().set_bool("air", true);
    rig.frame(0.5);
    assert_eq!(rig.state().as_deref(), Some("jump"));
    assert!(
        near(rig.at(), [6.0, 0.0]),
        "the tree fading out: {:?}",
        rig.at()
    );
    rig.frame(0.5);
    assert!(near(rig.at(), [8.0, 0.0]));

    rig.animator().set_bool("air", false);
    rig.animator().set_float("speed", 0.0);
    rig.frame(0.5);
    assert_eq!(rig.state().as_deref(), Some("move"));
    assert!(
        near(rig.at(), [4.0, 0.0]),
        "into the tree at idle: {:?}",
        rig.at()
    );
    rig.frame(0.5);
    assert!(near(rig.at(), [0.0, 0.0]), "{:?}", rig.at());
}

#[test]
fn a_reloaded_tree_takes_its_new_thresholds_and_clips_at_once() {
    let mut rig = line();
    rig.animator().set_float("speed", 1.0);
    rig.frame(0.25);
    assert!(near(rig.at(), [2.0, 0.0]));
    let phase = rig.synced().expect("a blend").phase();
    let root = rig.machines.root().to_path_buf();
    let file = root.join("hero.animator.ron");
    std::fs::write(&file, LINE.replace("threshold: 2.0", "threshold: 1.0")).expect("writes");
    assert!(rig.machines.reload(&path("hero.animator.ron")));
    rig.frame(0.0);
    assert!(near(rig.at(), [4.0, 0.0]), "walk now at 1: {:?}", rig.at());
    assert_eq!(rig.synced().expect("a blend").phase(), phase, "in step");

    let fewer = LINE.replace("(threshold: 2.0, clip: Path(\"walk.anim.ron\")),", "");
    std::fs::write(&file, fewer).expect("writes");
    assert!(rig.machines.reload(&path("hero.animator.ron")));
    rig.frame(0.0);
    assert_eq!(rig.synced().expect("a blend").clips().len(), 2);
    assert!(
        near(rig.at(), [2.0, 0.0]),
        "a quarter of the way to run: {:?}",
        rig.at()
    );
}

#[test]
fn a_tree_reading_a_parameter_that_is_not_a_float_reads_zero() {
    let machine = LINE
        .replace("parameter: \"speed\"", "parameter: \"grounded\"")
        .replace(
            "(name: \"speed\", value: Float(0.0))",
            "(name: \"grounded\", value: Bool(true))",
        );
    let checked: StateMachine = ron::from_str(&machine).expect("reads");
    let problems = checked.motion_problems(&checked.states[0]);
    assert!(
        problems.iter().any(|p| p.contains("not a float")),
        "{problems:?}"
    );
    let mut rig = rig(
        &machine,
        &[
            ("idle", at(1.0, 0.0)),
            ("walk", at(4.0, 0.0)),
            ("run", at(8.0, 0.0)),
        ],
    );
    rig.frame(0.1);
    assert!(near(rig.at(), [1.0, 0.0]), "at zero: idle");
}

#[test]
fn a_clip_state_is_written_as_it_always_was() {
    let machine: StateMachine = ron::from_str(MACHINE).expect("an older file reads");
    let text = ron::to_string(&machine.states[0]).expect("writes");
    assert!(text.contains("clip:Path(\"idle.anim.ron\")"), "{text}");
    assert!(!text.contains("Some") && !text.contains("tree"), "{text}");
    let line: StateMachine = ron::from_str(LINE).expect("reads");
    let text = ron::to_string(&line).expect("writes");
    assert!(
        text.contains("tree:Blend1D(") && !text.contains("clip:None"),
        "{text}"
    );
    let back: StateMachine = ron::from_str(&text).expect("reads back");
    assert_eq!(back, line);
}
