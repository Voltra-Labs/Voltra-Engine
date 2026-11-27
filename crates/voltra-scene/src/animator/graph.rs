//! A machine's states and transitions, as one layer — the top level, or one
//! of its `layers` — holds them.

use super::machine::{State, Transition};

/// The states one layer of a machine moves between and the transitions that
/// move it: the machine's own (layer 0), or a [`Layer`](super::Layer)'s.
/// Borrowed, so the two are walked by the same code.
#[derive(Debug, Clone, Copy)]
pub struct Graph<'a> {
    /// The state it starts in, by name; empty is the first.
    pub start: &'a str,
    /// Its states.
    pub states: &'a [State],
    /// Its transitions, tried in order.
    pub transitions: &'a [Transition],
}

impl<'a> Graph<'a> {
    /// The state named `name`.
    pub fn state(&self, name: &str) -> Option<&'a State> {
        self.states.iter().find(|s| s.name == name)
    }

    /// The state it starts in: `start`, or the first.
    pub fn start_state(&self) -> Option<&'a State> {
        self.state(self.start).or(self.states.first())
    }
}
