//! Loading a machine leniently: what cannot run is dropped and warned
//! about, and what an editor changed is put back into the file as written.

use std::collections::HashSet;

use super::machine::{Condition, ParamValue, State, StateMachine, Transition};
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
    /// it all. Every layer is checked the same way, each against its own
    /// states; a layer's weight outside `0..=1` is warned of and clamped
    /// when it plays, kept as written.
    ///
    /// Dropped rather than refusing the file, as a clip's bad keys are: an
    /// author mid-edit should still see the rest run.
    pub fn checked(mut self, source: &str) -> Self {
        let mut start = std::mem::take(&mut self.start);
        let mut states = std::mem::take(&mut self.states);
        let mut transitions = std::mem::take(&mut self.transitions);
        let mut layers = std::mem::take(&mut self.layers);
        self.check_graph(source, None, &mut start, &mut states, &mut transitions);
        let mut names = HashSet::new();
        for layer in &mut layers {
            let label = format!("the layer {:?}", layer.name);
            if !names.insert(layer.name.clone()) {
                log::warn!("{source}: a second layer named {:?}; a weight set by its name reaches the first", layer.name);
            }
            if !(0.0..=1.0).contains(&layer.weight) {
                log::warn!(
                    "{source}: {label} weighs {}, which is not in 0..=1; it plays at {}",
                    layer.weight,
                    if layer.weight.is_nan() {
                        0.0
                    } else {
                        layer.weight.clamp(0.0, 1.0)
                    }
                );
            }
            self.check_graph(
                source,
                Some(&label),
                &mut layer.start,
                &mut layer.states,
                &mut layer.transitions,
            );
        }
        self.start = start;
        self.states = states;
        self.transitions = transitions;
        self.layers = layers;
        self
    }

    /// [`checked`](Self::checked) for one layer's states and transitions —
    /// the top level's, `layer` `None`, or a named layer's — against this
    /// machine's parameters.
    fn check_graph(
        &self,
        source: &str,
        layer: Option<&str>,
        start: &mut String,
        states: &mut Vec<State>,
        transitions: &mut Vec<Transition>,
    ) {
        let place = |what: String| match layer {
            Some(layer) => format!("{source}: {layer}: {what}"),
            None => format!("{source}: {what}"),
        };
        let mut seen = HashSet::new();
        states.retain(|state| {
            let fresh = seen.insert(state.name.clone());
            if !fresh {
                log::warn!(
                    "{}",
                    place(format!(
                        "a second state named {:?}; the first is kept",
                        state.name
                    ))
                );
            }
            fresh
        });
        for state in states.iter() {
            for problem in self.problems_of(state, layer.is_some()) {
                log::warn!("{}", place(format!("the state {:?} {problem}", state.name)));
            }
        }
        if !start.is_empty() && !states.iter().any(|s| s.name == *start) {
            log::warn!(
                "{}",
                place(format!(
                    "the start state {start:?} is not there; starting in the first"
                ))
            );
            start.clear();
        }
        for t in transitions.iter_mut() {
            if !(t.duration.is_finite() && t.duration >= 0.0) {
                log::warn!(
                    "{}",
                    place(format!(
                        "a transition to {:?} lasts {}s, which is not a time; it cuts",
                        t.to, t.duration
                    ))
                );
                t.duration = 0.0;
            }
        }
        transitions.retain(|t| {
            let known = |name: &str| states.iter().any(|s| s.name == name);
            if !known(&t.to) || t.from.as_deref().is_some_and(|from| !known(from)) {
                log::warn!(
                    "{}",
                    place(format!(
                        "a transition to {:?} names a state that is not there; dropped",
                        t.to
                    ))
                );
                return false;
            }
            for condition in &t.conditions {
                if let Some(problem) = self.condition_problem(condition) {
                    log::warn!(
                        "{}",
                        place(format!("a transition to {:?} {problem}; dropped", t.to))
                    );
                    return false;
                }
            }
            true
        });
    }

    /// Gives each transition, of every layer, the `duration` of its
    /// counterpart in `loaded` — this machine as [`checked`](Self::checked)
    /// kept it, then edited.
    ///
    /// The check keeps transitions in order, so each of `loaded`'s is the
    /// next of this machine's with the same `from`, `to`, conditions and
    /// exit time; one the check dropped keeps its own duration. The check
    /// keeps every layer, so layers are matched by place.
    pub fn take_durations(&mut self, loaded: &StateMachine) {
        take_durations(&mut self.transitions, &loaded.transitions);
        for (layer, edited) in self.layers.iter_mut().zip(&loaded.layers) {
            take_durations(&mut layer.transitions, &edited.transitions);
        }
    }

    /// Gives each state, of every layer, the blend tree of its counterpart
    /// in `loaded` — this machine as [`checked`](Self::checked) kept it,
    /// then edited: the first state of each name, as the check keeps. A
    /// state the check dropped keeps its own.
    pub fn take_trees(&mut self, loaded: &StateMachine) {
        take_trees(&mut self.states, &loaded.states);
        for (layer, edited) in self.layers.iter_mut().zip(&loaded.layers) {
            take_trees(&mut layer.states, &edited.states);
        }
    }

    /// Gives each layer the weight, mask and blend of its counterpart in
    /// `loaded`, matched by place: the check keeps every layer.
    pub fn take_layer_settings(&mut self, loaded: &StateMachine) {
        for (layer, edited) in self.layers.iter_mut().zip(&loaded.layers) {
            layer.weight = edited.weight;
            layer.mask.clone_from(&edited.mask);
            layer.blend = edited.blend;
        }
    }

    /// What is wrong with what `state` of layer 0 plays, if anything.
    #[cfg(test)]
    pub(super) fn motion_problems(&self, state: &State) -> Vec<String> {
        self.problems_of(state, false)
    }

    /// What is wrong with what `state` plays, if anything. In a layer above
    /// the first, a state that plays nothing is the way to show nothing,
    /// and is not a problem.
    fn problems_of(&self, state: &State, in_layer: bool) -> Vec<String> {
        let mut problems = Vec::new();
        let Some(tree) = &state.tree else {
            if state.clip.is_none() && !in_layer {
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

/// [`StateMachine::take_durations`] for one layer's transitions.
fn take_durations(transitions: &mut [Transition], loaded: &[Transition]) {
    let mut kept = loaded.iter().peekable();
    for transition in transitions {
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

/// [`StateMachine::take_trees`] for one layer's states.
fn take_trees(states: &mut [State], loaded: &[State]) {
    let mut seen = HashSet::new();
    for state in states {
        if !seen.insert(state.name.clone()) {
            continue;
        }
        if let Some(edited) = loaded.iter().find(|s| s.name == state.name) {
            state.tree.clone_from(&edited.tree);
        }
    }
}
