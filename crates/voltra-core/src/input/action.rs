//! What each action is doing this frame, derived from its bindings.

use std::collections::HashMap;

use super::binding::Binding;
use super::map::ActionMap;

/// One action's state for the frame: held, and whether it went down or up.
///
/// Edges are the *action's*, not a binding's. Two keys bound to one action,
/// pressed one after the other, are one press: the action was already held
/// when the second went down. And a tap shorter than a frame — down and up
/// between two frames — still reports a press and a release, never nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ActionState {
    held: bool,
    pressed: bool,
    released: bool,
    strength: f32,
}

impl ActionState {
    /// Whether any binding is past the action's dead zone.
    pub fn is_held(&self) -> bool {
        self.held
    }

    /// Whether the action went down this frame.
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    /// Whether the action went up this frame.
    pub fn released(&self) -> bool {
        self.released
    }

    /// How far the action is in, `0` to `1`: `1` for a key, a stick's travel
    /// past the dead zone rescaled to start at `0` — so a walk speed scaled
    /// by it does not jump from nothing to a quarter.
    pub fn strength(&self) -> f32 {
        self.strength
    }
}

/// What the frame's raw input says about one binding.
pub(crate) trait BindingSource {
    /// How far `binding` is in, `0` to `1`: `0` or `1` for a key.
    fn value(&self, binding: Binding) -> f32;
    /// Whether `binding` was pressed this frame — the sub-frame tap a held
    /// test alone would miss. Always `false` for a stick.
    fn went_down(&self, binding: Binding) -> bool;
}

/// Every action's state, re-derived once per frame.
#[derive(Debug, Clone, Default)]
pub(crate) struct ActionStates {
    states: HashMap<String, ActionState>,
}

impl ActionStates {
    /// The state of `action`, or `None` for one the map does not have.
    pub(crate) fn get(&self, action: &str) -> Option<ActionState> {
        self.states.get(action).copied()
    }

    /// Re-derives every action in `map` from `source`, against last frame's
    /// held state. Actions no longer in the map are dropped.
    pub(crate) fn update(&mut self, map: &ActionMap, source: &impl BindingSource) {
        let previous = std::mem::take(&mut self.states);
        for name in map.names() {
            let was_held = previous.get(name).is_some_and(ActionState::is_held);
            let bindings = map.bindings(name);
            let raw = bindings
                .iter()
                .map(|b| source.value(*b))
                .fold(0.0, f32::max);
            let deadzone = map.deadzone(name);
            let held = raw > deadzone;
            let strength = if held {
                ((raw - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let tapped = bindings.iter().any(|b| source.went_down(*b));
            let state = ActionState {
                held,
                pressed: !was_held && (held || tapped),
                released: (was_held && !held) || (!was_held && !held && tapped),
                strength,
            };
            self.states.insert(name.to_owned(), state);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use winit::keyboard::KeyCode;

    use super::*;

    const SPACE: Binding = Binding::Key(KeyCode::Space);
    const UP: Binding = Binding::Key(KeyCode::ArrowUp);

    /// A frame of raw input: what is down, and what went down.
    #[derive(Default)]
    struct Frame {
        down: HashSet<Binding>,
        went_down: HashSet<Binding>,
    }

    impl BindingSource for Frame {
        fn value(&self, binding: Binding) -> f32 {
            f32::from(u8::from(self.down.contains(&binding)))
        }
        fn went_down(&self, binding: Binding) -> bool {
            self.went_down.contains(&binding)
        }
    }

    fn map() -> ActionMap {
        let mut map = ActionMap::new();
        map.bind("jump", SPACE);
        map.bind("jump", UP);
        map
    }

    fn step(states: &mut ActionStates, down: &[Binding], went_down: &[Binding]) -> ActionState {
        let frame = Frame {
            down: down.iter().copied().collect(),
            went_down: went_down.iter().copied().collect(),
        };
        states.update(&map(), &frame);
        states.get("jump").expect("in the map")
    }

    #[test]
    fn a_press_held_and_let_go_is_one_press_and_one_release() {
        let mut states = ActionStates::default();
        let first = step(&mut states, &[SPACE], &[SPACE]);
        assert!(first.pressed() && first.is_held() && !first.released());
        let held = step(&mut states, &[SPACE], &[]);
        assert!(!held.pressed() && held.is_held());
        let up = step(&mut states, &[], &[]);
        assert!(up.released() && !up.is_held() && !up.pressed());
    }

    #[test]
    fn a_second_binding_going_down_while_held_is_not_a_second_press() {
        let mut states = ActionStates::default();
        step(&mut states, &[SPACE], &[SPACE]);
        let both = step(&mut states, &[SPACE, UP], &[UP]);
        assert!(!both.pressed());
        let one_left = step(&mut states, &[UP], &[]);
        assert!(!one_left.released(), "still held through the other key");
    }

    #[test]
    fn a_tap_inside_one_frame_presses_and_releases() {
        let mut states = ActionStates::default();
        let tap = step(&mut states, &[], &[SPACE]);
        assert!(tap.pressed() && tap.released() && !tap.is_held());
        let after = step(&mut states, &[], &[]);
        assert_eq!(after, ActionState::default());
    }

    #[test]
    fn strength_is_one_while_held() {
        let mut states = ActionStates::default();
        assert_eq!(step(&mut states, &[UP], &[UP]).strength(), 1.0);
        assert_eq!(step(&mut states, &[], &[]).strength(), 0.0);
    }

    /// A source whose only control is a stick at `value`.
    struct Stick(f32);

    impl BindingSource for Stick {
        fn value(&self, binding: Binding) -> f32 {
            match binding {
                Binding::PadAxis(_, direction) => direction.amount(self.0),
                _ => 0.0,
            }
        }
        fn went_down(&self, _: Binding) -> bool {
            false
        }
    }

    fn stick_map(deadzone: f32) -> ActionMap {
        use super::super::binding::{Direction, PadAxis};
        let mut map = ActionMap::new();
        map.bind(
            "right",
            Binding::PadAxis(PadAxis::LeftStickX, Direction::Positive),
        );
        map.set_deadzone("right", deadzone);
        map
    }

    #[test]
    fn a_stick_inside_the_dead_zone_is_not_held() {
        let mut states = ActionStates::default();
        states.update(&stick_map(0.25), &Stick(0.2));
        assert!(!states.get("right").expect("mapped").is_held());
    }

    #[test]
    fn strength_starts_at_zero_at_the_dead_zone_edge() {
        let mut states = ActionStates::default();
        states.update(&stick_map(0.25), &Stick(0.625));
        let right = states.get("right").expect("mapped");
        assert!(right.is_held() && right.pressed());
        assert!(
            (right.strength() - 0.5).abs() < 1e-6,
            "{}",
            right.strength()
        );
        states.update(&stick_map(0.25), &Stick(1.0));
        assert_eq!(states.get("right").map(|s| s.strength()), Some(1.0));
    }

    #[test]
    fn the_other_direction_of_a_stick_does_nothing() {
        let mut states = ActionStates::default();
        states.update(&stick_map(0.25), &Stick(-1.0));
        assert!(!states.get("right").expect("mapped").is_held());
    }

    #[test]
    fn an_action_removed_from_the_map_is_gone() {
        let mut states = ActionStates::default();
        step(&mut states, &[SPACE], &[SPACE]);
        states.update(&ActionMap::new(), &Frame::default());
        assert_eq!(states.get("jump"), None);
    }
}
