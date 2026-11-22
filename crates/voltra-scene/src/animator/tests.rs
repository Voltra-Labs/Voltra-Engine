//! An animator run against a small machine: idle, run, jump and land.

use voltra_assets::AssetPath;
use voltra_ecs::{Entity, World};

use super::*;
use crate::keyframes::{self, AnimationClips};
use crate::Transform;

const MACHINE: &str = r#"(
    parameters: [
        (name: "speed", value: Float(0.0)),
        (name: "grounded", value: Bool(true)),
        (name: "jump", value: Trigger(false)),
    ],
    start: "idle",
    states: [
        (name: "idle", clip: Path("idle.anim.ron")),
        (name: "run", clip: Path("run.anim.ron"), speed: 2.0),
        (name: "jump", clip: Path("jump.anim.ron")),
        (name: "land", clip: Path("land.anim.ron")),
    ],
    transitions: [
        (from: Some("idle"), to: "run", conditions: [Greater("speed", 0.1)]),
        (from: Some("run"), to: "idle", conditions: [Less("speed", 0.1)]),
        (to: "jump", conditions: [If("jump"), If("grounded")]),
        (from: Some("jump"), to: "land", conditions: [If("grounded")], exit_time: Some(0.5)),
        (from: Some("land"), to: "idle", exit_time: Some(1.0)),
    ],
)"#;

const LOOP: &str = "(looping: true, tracks: [(property: Rotation, keys: [(time: 0.0, value: Float(0.0)), (time: 1.0, value: Float(1.0))])])";
const ONCE: &str = "(tracks: [(property: Rotation, keys: [(time: 0.0, value: Float(0.0)), (time: 0.5, value: Float(1.0))])])";

fn path(name: &str) -> AssetPath {
    AssetPath::new(name).expect("valid")
}

struct Rig {
    world: World,
    machines: StateMachines,
    clips: AnimationClips,
    hero: Entity,
}

impl Rig {
    fn new(machine: &str) -> Self {
        let root = voltra_testkit::scratch_root();
        std::fs::write(root.join("hero.animator.ron"), machine).expect("writes");
        for (name, text) in [
            ("idle", LOOP),
            ("run", LOOP),
            ("jump", LOOP),
            ("land", ONCE),
        ] {
            std::fs::write(root.join(format!("{name}.anim.ron")), text).expect("writes");
        }
        let mut world = World::new();
        let hero = world.spawn();
        world.insert(hero, Transform::default());
        world.insert(hero, Animator::new(path("hero.animator.ron")));
        Self {
            world,
            machines: StateMachines::new(&root),
            clips: AnimationClips::new(&root),
            hero,
        }
    }

    fn frame(&mut self, delta: f32) {
        evaluate(&mut self.world, &mut self.machines, &mut self.clips);
        keyframes::advance(&mut self.world, &mut self.clips, delta);
    }

    fn animator(&mut self) -> &mut Animator {
        self.world
            .get_mut::<Animator>(self.hero)
            .expect("an animator")
    }

    fn state(&self) -> Option<String> {
        let animator = self.world.get::<Animator>(self.hero).expect("an animator");
        animator.current_state().map(str::to_owned)
    }

    fn clip(&self) -> Option<String> {
        let player = self.world.get::<AnimationPlayer>(self.hero)?;
        player.clip.as_ref().map(|p| p.as_str().to_owned())
    }
}

#[test]
fn it_starts_in_the_start_state_and_gives_its_entity_a_player() {
    let mut rig = Rig::new(MACHINE);
    rig.frame(0.1);
    assert_eq!(rig.state().as_deref(), Some("idle"));
    assert_eq!(rig.clip().as_deref(), Some("idle.anim.ron"));
}

#[test]
fn a_float_moves_it_between_states_at_the_state_s_speed() {
    let mut rig = Rig::new(MACHINE);
    rig.frame(0.1);
    rig.animator().set_float("speed", 3.0);
    rig.frame(0.1);
    assert_eq!(rig.state().as_deref(), Some("run"));
    assert_eq!(
        rig.world.get::<AnimationPlayer>(rig.hero).expect("p").speed,
        2.0
    );
    rig.animator().set_float("speed", 0.0);
    rig.frame(0.1);
    assert_eq!(rig.state().as_deref(), Some("idle"));
}

#[test]
fn a_trigger_is_spent_by_the_transition_it_lets_through() {
    let mut rig = Rig::new(MACHINE);
    rig.frame(0.1);
    rig.animator().set_bool("grounded", true);
    rig.animator().trigger("jump");
    rig.frame(0.1);
    assert_eq!(rig.state().as_deref(), Some("jump"));
    assert_eq!(
        rig.animator().parameter("jump"),
        Some(ParamValue::Trigger(false))
    );
    // From any state, but never back into itself.
    rig.animator().trigger("jump");
    rig.animator().set_bool("grounded", false);
    rig.frame(0.1);
    assert_eq!(rig.state().as_deref(), Some("jump"));
}

#[test]
fn an_exit_time_waits_for_the_clip() {
    let mut rig = Rig::new(MACHINE);
    rig.frame(0.0);
    rig.animator().trigger("jump");
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("jump"));
    // Grounded at once, but jump's clip must be half played first.
    rig.frame(0.3);
    assert_eq!(rig.state().as_deref(), Some("jump"));
    rig.frame(0.3);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("land"));
    // Land does not loop; idle waits for it to end.
    rig.frame(0.3);
    assert_eq!(rig.state().as_deref(), Some("land"));
    rig.frame(0.3);
    rig.frame(0.0);
    assert_eq!(rig.state().as_deref(), Some("idle"));
}

#[test]
fn a_reloaded_machine_keeps_a_state_it_still_has_and_restarts_without_one() {
    let mut rig = Rig::new(MACHINE);
    rig.frame(0.1);
    rig.animator().set_float("speed", 3.0);
    rig.frame(0.1);
    let root = rig.machines.root().to_path_buf();
    std::fs::write(
        root.join("hero.animator.ron"),
        MACHINE.replace("speed: 2.0", "speed: 3.0"),
    )
    .expect("writes");
    assert!(rig.machines.reload(&path("hero.animator.ron")));
    rig.frame(0.1);
    assert_eq!(rig.state().as_deref(), Some("run"), "kept");

    let renamed = MACHINE.replace("\"run\"", "\"sprint\"");
    std::fs::write(root.join("hero.animator.ron"), renamed).expect("writes");
    assert!(rig.machines.reload(&path("hero.animator.ron")));
    rig.frame(0.1);
    assert_eq!(
        rig.state().as_deref(),
        Some("idle"),
        "its state is gone: the start"
    );
}

#[test]
fn transitions_that_name_what_is_not_there_are_dropped() {
    let text = r#"(
        parameters: [(name: "speed", value: Float(0.0))],
        states: [(name: "a", clip: Path("idle.anim.ron")), (name: "a", clip: Path("run.anim.ron"))],
        transitions: [
            (to: "b"),
            (to: "a", conditions: [If("speed")]),
            (to: "a", conditions: [Greater("height", 1.0)]),
            (from: Some("a"), to: "a", conditions: [Greater("speed", 1.0)]),
        ],
    )"#;
    let machine: StateMachine = ron::from_str(text).expect("reads");
    let machine = machine.checked("test");
    assert_eq!(machine.states.len(), 1, "the second `a` goes");
    assert_eq!(machine.transitions.len(), 1, "only the last survives");
}

#[test]
fn a_machine_that_will_not_read_runs_nothing() {
    let mut rig = Rig::new("(states: [");
    rig.frame(0.1);
    assert_eq!(rig.state(), None);
    assert!(rig.world.get::<AnimationPlayer>(rig.hero).is_none());
}

#[test]
fn the_shipped_machine_reads_whole_and_names_clips_that_are_there() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut machines = StateMachines::new(&root);
    let handle = machines.load(&path("animations/hero.animator.ron"));
    let machine = machines.get(handle);
    assert_eq!(machine.states.len(), 4);
    assert_eq!(machine.transitions.len(), 4, "none dropped");
    let mut clips = AnimationClips::new(&root);
    for state in &machine.states {
        assert!(
            root.join(state.clip.as_str()).is_file(),
            "{}",
            state.clip.as_str()
        );
        let clip = clips.load(&state.clip);
        assert!(!clips.get(clip).is_empty(), "{} reads", state.clip.as_str());
    }
}
