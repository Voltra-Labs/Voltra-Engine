//! Loading a machine leniently: what cannot run is dropped and warned
//! about, and what an editor changed is put back into the file as written.

use super::machine::{Condition, ParamValue, State, StateMachine};
use super::tree::BlendTree;

impl StateMachine {
    /// The machine as an animator can trust it: transitions naming a state
    /// that is not there, or testing a parameter that is not — or one of
    /// the wrong kind — dropped, with a warning naming `source`; two states
    /// of one name keep the first; a fade's duration that is not a time of
    /// zero or more is a cut. A state's motion is checked but kept as
    /// written — a tree reading a parameter that is not a float reads
    /// zero, a child at a place that is not a finite number weighs
    /// nothing, an empty tree or a state with neither clip nor tree plays
    /// nothing — each with a warning; so the file an editor saves back has
    /// it all.
    ///
    /// Dropped rather than refusing the file, as a clip's bad keys are: an
    /// author mid-edit should still see the rest run.
    pub fn checked(mut self, source: &str) -> Self {
        let mut seen = std::collections::HashSet::new();
        self.states.retain(|state| {
            let fresh = seen.insert(state.name.clone());
            if !fresh {
                log::warn!(
                    "{source}: a second state named {:?}; the first is kept",
                    state.name
                );
            }
            fresh
        });
        for state in &self.states {
            for problem in self.motion_problems(state) {
                log::warn!("{source}: the state {:?} {problem}", state.name);
            }
        }
        if !self.start.is_empty() && self.state(&self.start).is_none() {
            log::warn!(
                "{source}: the start state {:?} is not there; starting in the first",
                self.start
            );
            self.start.clear();
        }
        for t in &mut self.transitions {
            if !(t.duration.is_finite() && t.duration >= 0.0) {
                log::warn!(
                    "{source}: a transition to {:?} lasts {}s, which is not a time; it cuts",
                    t.to,
                    t.duration
                );
                t.duration = 0.0;
            }
        }
        let machine = self.clone();
        self.transitions.retain(|t| {
            let known = |name: &str| machine.state(name).is_some();
            if !known(&t.to) || t.from.as_deref().is_some_and(|from| !known(from)) {
                log::warn!(
                    "{source}: a transition to {:?} names a state that is not there; dropped",
                    t.to
                );
                return false;
            }
            for condition in &t.conditions {
                if let Some(problem) = machine.condition_problem(condition) {
                    log::warn!("{source}: a transition to {:?} {problem}; dropped", t.to);
                    return false;
                }
            }
            true
        });
        self
    }

    /// Gives each transition the `duration` of its counterpart in `loaded`
    /// — this machine as [`checked`](Self::checked) kept it, then edited.
    ///
    /// The check keeps transitions in order, so each of `loaded`'s is the
    /// next of this machine's with the same `from`, `to`, conditions and
    /// exit time; one the check dropped keeps its own duration.
    pub fn take_durations(&mut self, loaded: &StateMachine) {
        let mut kept = loaded.transitions.iter().peekable();
        for transition in &mut self.transitions {
            let Some(edited) = kept.peek() else {
                return;
            };
            let same = edited.from == transition.from
                && edited.to == transition.to
                && edited.conditions == transition.conditions
                && edited.exit_time == transition.exit_time;
            if same {
                transition.duration = edited.duration;
                kept.next();
            }
        }
    }

    /// Gives each state the blend tree of its counterpart in `loaded` —
    /// this machine as [`checked`](Self::checked) kept it, then edited:
    /// the first state of each name, as the check keeps. A state the
    /// check dropped keeps its own.
    pub fn take_trees(&mut self, loaded: &StateMachine) {
        let mut seen = std::collections::HashSet::new();
        for state in &mut self.states {
            if !seen.insert(state.name.clone()) {
                continue;
            }
            if let Some(edited) = loaded.state(&state.name) {
                state.tree.clone_from(&edited.tree);
            }
        }
    }

    /// What is wrong with what `state` plays, if anything.
    pub(super) fn motion_problems(&self, state: &State) -> Vec<String> {
        let mut problems = Vec::new();
        let Some(tree) = &state.tree else {
            if state.clip.is_none() {
                problems.push("has neither a clip nor a tree; it plays nothing".to_owned());
            }
            return problems;
        };
        if state.clip.is_some() {
            problems.push("has both a clip and a tree; it plays the tree".to_owned());
        }
        if tree.is_empty() {
            problems.push("has a blend tree of no clips; it plays nothing".to_owned());
        }
        for name in tree.parameters() {
            match self.parameter(name) {
                Some(ParamValue::Float(_)) => {}
                Some(_) => problems.push(format!(
                    "blends by {name:?}, which is not a float; it reads 0"
                )),
                None => problems.push(format!(
                    "blends by {name:?}, which is not a parameter; it reads 0"
                )),
            }
        }
        let unplaced = match tree {
            BlendTree::Blend1D { children, .. } => {
                children.iter().filter(|c| !c.threshold.is_finite()).count()
            }
            BlendTree::Blend2D { children, .. } => children
                .iter()
                .filter(|c| !c.position.iter().all(|v| v.is_finite()))
                .count(),
        };
        if unplaced > 0 {
            problems.push(format!(
                "has {unplaced} blend tree clip(s) at a place that is not a finite number; they weigh nothing"
            ));
        }
        problems
    }

    fn condition_problem(&self, condition: &Condition) -> Option<String> {
        let name = condition.parameter();
        let Some(value) = self.parameter(name) else {
            return Some(format!("tests {name:?}, which is not a parameter"));
        };
        let fits = match condition {
            Condition::If(_) | Condition::IfNot(_) => !matches!(value, ParamValue::Float(_)),
            Condition::Greater(..) | Condition::Less(..) => matches!(value, ParamValue::Float(_)),
        };
        (!fits).then(|| format!("tests {name:?} as the wrong kind"))
    }
}
