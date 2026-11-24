//! Pressing an action from code, as a player would.

use super::{ActionMap, Binding, Direction, Input};

/// Holds `action` down — or lets it go — through the first of its bindings
/// in `actions`: a key, a mouse button, a pad button, a stick pushed fully
/// its way. A game reads it exactly as it reads a player, dead zones and
/// all. Answers whether the map has the action with a binding to drive.
pub fn drive_action(input: &mut Input, actions: &ActionMap, action: &str, down: bool) -> bool {
    let Some(&binding) = actions.bindings(action).first() else {
        return false;
    };
    match binding {
        Binding::Key(code) => input.on_key(code, down, false),
        Binding::Mouse(button) => input.on_mouse_button(button, down),
        Binding::Pad(button) => input.on_pad_button(0, button, down),
        Binding::PadAxis(axis, direction) => {
            let full = match direction {
                Direction::Positive => 1.0,
                Direction::Negative => -1.0,
            };
            input.on_pad_axis(0, axis, if down { full } else { 0.0 });
        }
        Binding::Touch(zone, direction) => {
            // A finger of its own, by an id no platform hands out: down in
            // the zone's middle and slid fully this way, or lifted.
            const DRIVEN: u64 = u64::MAX;
            let screen = input.screen();
            let start = zone.centre(screen);
            if down {
                let radius = super::stick_radius(screen);
                let offset = match direction {
                    super::TouchDir::Hold => voltra_render::glam::Vec2::ZERO,
                    super::TouchDir::Left => voltra_render::glam::Vec2::new(-radius, 0.0),
                    super::TouchDir::Right => voltra_render::glam::Vec2::new(radius, 0.0),
                    super::TouchDir::Up => voltra_render::glam::Vec2::new(0.0, -radius),
                    super::TouchDir::Down => voltra_render::glam::Vec2::new(0.0, radius),
                };
                input.on_touch(DRIVEN, super::TouchPhase::Started, start);
                input.on_touch(DRIVEN, super::TouchPhase::Moved, start + offset);
            } else {
                input.on_touch(DRIVEN, super::TouchPhase::Ended, start);
            }
        }
    }
    true
}

/// Actions held for a number of frames: what a tool driving a running game
/// asks for. Ticked once a frame, before the frame reads its input.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HeldActions {
    held: Vec<(String, u32)>,
}

impl HeldActions {
    /// Holds `action` for the next `frames` frames, from the next one.
    pub fn hold(&mut self, action: &str, frames: u32) {
        self.held.retain(|(name, _)| name != action);
        self.held.push((action.to_owned(), frames));
    }

    /// Presses what is still held, and lets go of what has run its frames.
    pub fn tick(&mut self, input: &mut Input, actions: &ActionMap) {
        self.held.retain_mut(|(action, left)| {
            if *left == 0 {
                drive_action(input, actions, action, false);
                return false;
            }
            drive_action(input, actions, action, true);
            *left -= 1;
            true
        });
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyCode;

    fn map() -> ActionMap {
        let mut map = ActionMap::new();
        map.bind("jump", Binding::Key(KeyCode::Space));
        map
    }

    #[test]
    fn a_held_action_is_down_for_its_frames_then_let_go() {
        let (mut input, actions) = (Input::new(), map());
        let mut held = HeldActions::default();
        held.hold("jump", 2);
        for frame in 0..2 {
            held.tick(&mut input, &actions);
            assert!(input.is_key_down(KeyCode::Space), "frame {frame}");
            input.end_frame();
        }
        held.tick(&mut input, &actions);
        assert!(!input.is_key_down(KeyCode::Space));
        assert!(held.is_empty());
    }

    #[test]
    fn holding_again_restarts_the_count_and_an_unbound_action_drives_nothing() {
        let (mut input, actions) = (Input::new(), map());
        let mut held = HeldActions::default();
        held.hold("jump", 1);
        held.hold("jump", 3);
        for _ in 0..3 {
            held.tick(&mut input, &actions);
        }
        assert!(input.is_key_down(KeyCode::Space), "still held on the third");
        assert!(!drive_action(&mut input, &actions, "fly", true));
    }
}
