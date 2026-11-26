//! What a `.animator.ron` file holds: parameters, states and the
//! transitions between them.

use serde::{Deserialize, Serialize};
use voltra_assets::AssetPath;

/// A state machine: the states an [`Animator`](super::Animator) moves
/// between, each playing a clip, and the conditions on its parameters that
/// move it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StateMachine {
    /// The values a game sets, with the ones they start at.
    pub parameters: Vec<Parameter>,
    /// The state it starts in, by name; empty is the first.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub start: String,
    /// The states the machine can be in.
    pub states: Vec<State>,
    /// Tried in order; the first whose conditions hold is taken.
    pub transitions: Vec<Transition>,
}

/// A named value a game sets and the transitions read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    /// The name transitions and the game refer to it by.
    pub name: String,
    /// Its kind, and the value it starts at.
    pub value: ParamValue,
}

/// A parameter's value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ParamValue {
    /// A number, tested with greater-than and less-than.
    Float(f32),
    /// A flag, tested for true or false.
    Bool(bool),
    /// Set by a game, and spent by the transition it lets through — a jump
    /// pressed, a hit taken. Unity's trigger.
    Trigger(bool),
}

impl ParamValue {
    /// Whether two values are of one kind.
    pub fn same_kind(self, other: Self) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }
}

/// One state: a clip, played at a speed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// The name transitions refer to it by.
    pub name: String,
    /// The `.anim.ron` it plays — looping or not as the clip says.
    pub clip: AssetPath,
    /// A multiple of the clip's speed.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub speed: f32,
}

fn one() -> f32 {
    1.0
}

fn is_one(value: &f32) -> bool {
    *value == 1.0
}

/// A move from one state — or from any — to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    /// The state it leaves, by name; `None` leaves any state but `to`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// The state it enters, by name.
    pub to: String,
    /// All must hold. None at all, with no `exit_time`, moves at once.
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// How much of the state's clip, as a fraction of its length, must have
    /// played first. Below one, the point in each loop: `0.5` is half way
    /// through any loop. One or more counts loops on a clip that loops —
    /// `2.5` waits for two loops and a half — and is the end of one that
    /// does not. A clip of no length has nothing to wait for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_time: Option<f32>,
    /// Seconds to crossfade into `to`'s clip, the one leaving still playing
    /// meanwhile; `0`, the default, cuts. Per transition, with no machine-
    /// wide default: one place to look.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub duration: f32,
}

fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

/// A test of one parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// A bool that is true, or a trigger that is set.
    If(String),
    /// A bool that is false, or a trigger that is not set.
    IfNot(String),
    /// A float above the value.
    Greater(String, f32),
    /// A float below the value.
    Less(String, f32),
}

impl Condition {
    /// The parameter it reads.
    pub fn parameter(&self) -> &str {
        match self {
            Self::If(name) | Self::IfNot(name) | Self::Greater(name, _) | Self::Less(name, _) => {
                name
            }
        }
    }
}

impl StateMachine {
    /// The state named `name`.
    pub fn state(&self, name: &str) -> Option<&State> {
        self.states.iter().find(|s| s.name == name)
    }

    /// The state it starts in: `start`, or the first.
    pub fn start_state(&self) -> Option<&State> {
        self.state(&self.start).or(self.states.first())
    }

    /// The parameter named `name`'s starting value.
    pub fn parameter(&self, name: &str) -> Option<ParamValue> {
        self.parameters
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.value)
    }

    /// The machine as an animator can trust it: transitions naming a state
    /// that is not there, or testing a parameter that is not — or one of
    /// the wrong kind — dropped, with a warning naming `source`; two states
    /// of one name keep the first; a fade's duration that is not a time of
    /// zero or more is a cut.
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
