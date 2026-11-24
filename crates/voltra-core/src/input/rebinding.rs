//! What an options screen's rebinding list needs of a map beyond
//! [`ActionMap::rebind`]: a binding of one kind replaced, the other actions
//! left alone, and the clashes that leaves found — to be shown, not
//! silently undone.
//!
//! `rebind` takes the control off every other action, which is right for a
//! game's own "press a key" and wrong for a screen: binding jump to the key
//! that walks would quietly unbind walking. Unity's rebinding UI sample and
//! Godot's projects show the clash and let the player resolve it, which is
//! what [`ActionMap::assign`] and [`ActionMap::conflicts`] are for.

use super::binding::Binding;
use super::map::ActionMap;

/// Which kind of control a binding is: a row of a rebinding list keeps one
/// of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingKind {
    /// A key or a mouse button.
    KeyboardMouse,
    /// A gamepad button or a stick direction.
    Gamepad,
    /// A part of a touch screen.
    Touch,
}

impl Binding {
    /// The kind of control this is.
    pub fn kind(&self) -> BindingKind {
        match self {
            Self::Key(_) | Self::Mouse(_) => BindingKind::KeyboardMouse,
            Self::Pad(_) | Self::PadAxis(..) => BindingKind::Gamepad,
            Self::Touch(..) => BindingKind::Touch,
        }
    }
}

impl ActionMap {
    /// `action`'s first binding of `kind`: what a rebinding list shows.
    pub fn first_of_kind(&self, action: &str, kind: BindingKind) -> Option<Binding> {
        self.bindings(action)
            .iter()
            .copied()
            .find(|binding| binding.kind() == kind)
    }

    /// Makes `binding` `action`'s first of its kind — replacing the one
    /// there, or added when there is none — and nothing else: another action
    /// bound to the same control keeps it ([`conflicts`](Self::conflicts)
    /// says which). The action is made if the map lacks it.
    pub fn assign(&mut self, action: &str, binding: Binding) {
        let bindings = &mut self.actions.entry(action.to_owned()).or_default().bindings;
        let kind = binding.kind();
        match bindings.iter().position(|b| b.kind() == kind) {
            Some(first) => {
                bindings[first] = binding;
                // The same control further down the list is now a repeat.
                let mut seen = false;
                bindings.retain(|b| {
                    let repeat = *b == binding && seen;
                    seen |= *b == binding;
                    !repeat
                });
            }
            None => bindings.push(binding),
        }
    }

    /// The actions among `among`, other than `action`, that `binding` also
    /// drives — in `among`'s order.
    pub fn conflicts<'a>(
        &self,
        action: &str,
        binding: Binding,
        among: impl IntoIterator<Item = &'a str>,
    ) -> Vec<&'a str> {
        among
            .into_iter()
            .filter(|other| *other != action && self.bindings(other).contains(&binding))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use winit::keyboard::KeyCode;

    use super::*;
    use crate::input::PadButton;

    fn map() -> ActionMap {
        let mut map = ActionMap::new();
        for b in [KeyCode::Space, KeyCode::KeyW] {
            map.bind("jump", Binding::Key(b));
        }
        map.bind("jump", Binding::Pad(PadButton::South));
        map.bind("left", Binding::Key(KeyCode::KeyA));
        map
    }

    #[test]
    fn assign_replaces_the_first_of_its_kind_only() {
        let mut map = map();
        map.assign("jump", Binding::Key(KeyCode::KeyJ));
        assert_eq!(
            map.bindings("jump"),
            &[
                Binding::Key(KeyCode::KeyJ),
                Binding::Key(KeyCode::KeyW),
                Binding::Pad(PadButton::South)
            ]
        );
        map.assign("jump", Binding::Pad(PadButton::North));
        assert_eq!(
            map.first_of_kind("jump", BindingKind::Gamepad),
            Some(Binding::Pad(PadButton::North))
        );
        map.assign("left", Binding::Pad(PadButton::West));
        assert_eq!(map.bindings("left").len(), 2, "a kind it lacked is added");
    }

    #[test]
    fn assigning_a_control_further_down_the_list_leaves_no_repeat() {
        let mut map = map();
        map.assign("jump", Binding::Key(KeyCode::KeyW));
        assert_eq!(
            map.bindings("jump"),
            &[Binding::Key(KeyCode::KeyW), Binding::Pad(PadButton::South)]
        );
    }

    #[test]
    fn a_clash_is_kept_and_reported() {
        let mut map = map();
        map.assign("jump", Binding::Key(KeyCode::KeyA));
        assert_eq!(map.bindings("left"), &[Binding::Key(KeyCode::KeyA)]);
        let listed = ["jump", "left"];
        assert_eq!(
            map.conflicts("jump", Binding::Key(KeyCode::KeyA), listed),
            vec!["left"]
        );
        assert!(map
            .conflicts("jump", Binding::Key(KeyCode::KeyW), listed)
            .is_empty());
    }
}
