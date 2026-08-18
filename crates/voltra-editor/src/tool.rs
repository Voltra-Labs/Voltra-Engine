//! Which transform the next drag in the viewport performs.
//!
//! Unity's model rather than Blender's: the tool is persistent state, and the
//! gizmo on screen tells you what a drag will do before you start it. Blender
//! binds the transform to a gesture instead — `G`/`R`/`S` begin a modal
//! transform the mouse drives until a click confirms — which is faster once
//! learned and invisible until someone tells you the letter. It also needs
//! modal input capture that `Input` does not have: swallowing every key while
//! active, surviving a lost window focus, unwinding on `Esc`. That can be added
//! over a working gizmo; the reverse is harder. Godot 2D made the same call and
//! has a proposal open to add Blender's on top.
//!
//! The letters are `W`, `E`, `R`, which Unity, Unreal and Godot 2D all agree
//! on. That agreement is why the scene camera gave up its bare `WASD` and its
//! `R`: see [`crate::camera`].

use voltra_core::egui::Key;

/// The active transform tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    /// Click to select, drag a handle to move.
    #[default]
    Translate,
    /// Drag the ring to turn the selection about its own origin.
    Rotate,
    /// Drag an arm to scale along one local axis, the centre to scale both.
    Scale,
}

impl Tool {
    /// Every tool, in the order their keys sit on the keyboard.
    ///
    /// One list rather than a `match` per call site: the key binding, the
    /// toolbar row and any future radial menu are the same three tools in the
    /// same order, and a fourth should not have to be added to three places.
    pub const ALL: [Self; 3] = [Self::Translate, Self::Rotate, Self::Scale];

    /// The key that selects this tool.
    pub fn key(self) -> Key {
        match self {
            Self::Translate => Key::W,
            Self::Rotate => Key::E,
            Self::Scale => Key::R,
        }
    }

    /// What the history calls one drag of this tool.
    ///
    /// A verb in the imperative, because it is read as "Undo Move".
    pub fn label(self) -> &'static str {
        match self {
            Self::Translate => "Move",
            Self::Rotate => "Rotate",
            Self::Scale => "Scale",
        }
    }

    /// The glyph the toolbar draws for this tool.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::Translate => "✥",
            Self::Rotate => "⟳",
            Self::Scale => "⤢",
        }
    }

    /// The tooltip the toolbar shows, key included.
    pub fn hint(self) -> &'static str {
        match self {
            Self::Translate => "Move (W) — drag an arm for one axis, the centre for both",
            Self::Rotate => "Rotate (E) — drag the ring; the selection turns about its origin",
            Self::Scale => "Scale (R) — drag an arm for one local axis, the centre for both",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_its_own_key() {
        let mut keys: Vec<Key> = Tool::ALL.iter().map(|tool| tool.key()).collect();
        keys.dedup();
        assert_eq!(keys.len(), Tool::ALL.len(), "two tools share a key");
    }

    #[test]
    fn every_tool_has_its_own_history_label() {
        // Two tools with one label would merge in the Edit menu, where the
        // label is the only thing telling one entry from the next.
        let mut labels: Vec<&str> = Tool::ALL.iter().map(|tool| tool.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), Tool::ALL.len(), "two tools share a label");
    }

    #[test]
    fn the_default_tool_is_translate() {
        assert_eq!(Tool::default(), Tool::Translate);
    }

    #[test]
    fn the_keys_are_the_ones_every_editor_uses() {
        // W/E/R is Unity's, Unreal's and Godot 2D's binding. Changing it is a
        // decision, not a refactor.
        assert_eq!(Tool::Translate.key(), Key::W);
        assert_eq!(Tool::Rotate.key(), Key::E);
        assert_eq!(Tool::Scale.key(), Key::R);
    }
}
