//! Saving a machine's edited fades and trees without losing what loading
//! dropped.

use voltra_assets::AssetPath;

use super::StateMachines;
use crate::ron_store::RonAssetError;

impl StateMachines {
    /// Writes what the inspector edits of the loaded `path` — the
    /// transitions' durations, the states' blend trees and the layers'
    /// weights, masks and blends, of every layer — into its file, and
    /// nothing else of what is loaded.
    ///
    /// What is loaded is the machine [`checked`](super::StateMachine::checked):
    /// a transition naming a state that is not there, a second state of one
    /// name, a start that is not a state are gone from it. Saving that
    /// would delete them from the file, an author's typo turned into lost
    /// work by a click on a duration. So the file is read back as written,
    /// given the loaded durations
    /// ([`take_durations`](super::StateMachine::take_durations)), trees
    /// ([`take_trees`](super::StateMachine::take_trees)) and layer settings
    /// ([`take_layer_settings`](super::StateMachine::take_layer_settings))
    /// and written; what
    /// loading dropped stays in the file, to be fixed there. A file that no
    /// longer reads is not written over.
    pub fn save_edits(&mut self, path: &AssetPath) -> Result<(), RonAssetError> {
        let handle = self.load(path);
        let mut written = self.read_as_written(path)?;
        written.take_durations(self.get(handle));
        written.take_trees(self.get(handle));
        written.take_layer_settings(self.get(handle));
        self.write(path, &written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animator::StateMachine;

    #[test]
    fn a_save_keeps_what_loading_dropped_and_writes_the_edited_fade() {
        let root = voltra_testkit::scratch_root();
        let text = r#"(
            parameters: [(name: "speed", value: Float(0.0))],
            start: "walk",
            states: [
                (name: "idle", clip: Path("idle.anim.ron")),
                (name: "idle", clip: Path("other.anim.ron")),
                (name: "run", clip: Path("run.anim.ron")),
            ],
            transitions: [
                (to: "rnu", conditions: [Greater("speed", 0.1)]),
                (from: Some("idle"), to: "run", conditions: [Greater("speed", 0.1)], duration: 0.2),
                (to: "idle", conditions: [Greater("sped", 0.1)]),
                (from: Some("run"), to: "idle", conditions: [Less("speed", 0.1)]),
            ],
        )"#;
        std::fs::write(root.join("m.animator.ron"), text).expect("writes");
        let path = AssetPath::new("m.animator.ron").expect("valid");
        let mut machines = StateMachines::new(&root);
        let handle = machines.load(&path);
        assert_eq!(machines.get(handle).transitions.len(), 2, "two dropped");
        machines.get_mut(handle).transitions[0].duration = 0.5;
        machines.get_mut(handle).transitions[1].duration = 0.3;

        machines.save_edits(&path).expect("saves");
        let saved: StateMachine =
            ron::from_str(&std::fs::read_to_string(root.join("m.animator.ron")).expect("reads"))
                .expect("parses");
        assert_eq!(saved.start, "walk", "the bad start is the author's to fix");
        assert_eq!(saved.states.len(), 3);
        let fades: Vec<(&str, f32)> = saved
            .transitions
            .iter()
            .map(|t| (t.to.as_str(), t.duration))
            .collect();
        assert_eq!(
            fades,
            [("rnu", 0.0), ("run", 0.5), ("idle", 0.0), ("idle", 0.3)]
        );

        std::fs::write(root.join("m.animator.ron"), "(states: [").expect("writes");
        assert!(machines.save_edits(&path).is_err(), "not written over");
        assert_eq!(
            std::fs::read_to_string(root.join("m.animator.ron")).expect("reads"),
            "(states: ["
        );
    }

    #[test]
    fn a_save_writes_the_edited_tree_and_keeps_what_loading_dropped() {
        use crate::animator::BlendTree;

        let root = voltra_testkit::scratch_root();
        let text = r#"(
            parameters: [(name: "speed", value: Float(0.0))],
            states: [
                (name: "move", tree: Blend1D(parameter: "speed", children: [
                    (threshold: 0.0, clip: Path("idle.anim.ron")),
                    (threshold: 2.0, clip: Path("run.anim.ron")),
                ])),
                (name: "move", clip: Path("other.anim.ron")),
            ],
            transitions: [(to: "gone", conditions: [])],
        )"#;
        std::fs::write(root.join("m.animator.ron"), text).expect("writes");
        let path = AssetPath::new("m.animator.ron").expect("valid");
        let mut machines = StateMachines::new(&root);
        let handle = machines.load(&path);
        let machine = machines.get_mut(handle);
        assert_eq!((machine.states.len(), machine.transitions.len()), (1, 0));
        if let Some(BlendTree::Blend1D { children, .. }) = &mut machine.states[0].tree {
            children[1].threshold = 3.5;
        }

        machines.save_edits(&path).expect("saves");
        let saved: StateMachine =
            ron::from_str(&std::fs::read_to_string(root.join("m.animator.ron")).expect("reads"))
                .expect("parses");
        assert_eq!(saved.states.len(), 2, "the second `move` is the author's");
        assert_eq!(saved.transitions.len(), 1, "so is the bad transition");
        let Some(BlendTree::Blend1D { children, .. }) = &saved.states[0].tree else {
            panic!("a tree");
        };
        assert_eq!(children[1].threshold, 3.5);
        assert_eq!(saved.states[1].tree, None);
    }
}
