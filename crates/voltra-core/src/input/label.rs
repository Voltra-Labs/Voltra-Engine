//! A binding as a player reads it: `J`, `Up`, `Mouse Left`, `Pad South` —
//! what a rebinding list and a "press J to jump" prompt show.

use std::fmt;

use super::binding::{Binding, Direction};

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(code) => {
                let name = format!("{code:?}");
                let short = name
                    .strip_prefix("Key")
                    .or_else(|| name.strip_prefix("Digit"))
                    .or_else(|| name.strip_prefix("Arrow"))
                    .filter(|rest| !rest.is_empty())
                    .unwrap_or(&name);
                f.write_str(short)
            }
            Self::Mouse(button) => write!(f, "Mouse {button:?}"),
            Self::Pad(button) => write!(f, "Pad {button:?}"),
            Self::PadAxis(axis, direction) => {
                let sign = match direction {
                    Direction::Positive => '+',
                    Direction::Negative => '-',
                };
                write!(f, "Pad {axis:?}{sign}")
            }
            Self::Touch(zone, dir) => write!(f, "Touch {zone:?} {dir:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use winit::event::MouseButton;
    use winit::keyboard::KeyCode;

    use super::*;
    use crate::input::{PadAxis, PadButton};

    #[test]
    fn a_binding_reads_as_its_control() {
        let label = |b: Binding| b.to_string();
        assert_eq!(label(Binding::Key(KeyCode::KeyJ)), "J");
        assert_eq!(label(Binding::Key(KeyCode::Digit1)), "1");
        assert_eq!(label(Binding::Key(KeyCode::ArrowUp)), "Up");
        assert_eq!(label(Binding::Key(KeyCode::Space)), "Space");
        assert_eq!(label(Binding::Mouse(MouseButton::Left)), "Mouse Left");
        assert_eq!(label(Binding::Pad(PadButton::South)), "Pad South");
        assert_eq!(
            label(Binding::PadAxis(PadAxis::LeftStickX, Direction::Negative)),
            "Pad LeftStickX-"
        );
    }
}
