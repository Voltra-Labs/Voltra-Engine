//! What was typed this frame: text, editing keys and the IME's composition
//! — what a focused text field reads.

use voltra_scene::ui::text_field::{Edit, EditKey, Preedit};
use voltra_scene::ui::TextInput;
use winit::keyboard::KeyCode;

/// The frame's typing, and the modifiers it was typed with.
///
/// Text comes two ways, as egui-winit takes it: the text a key press
/// produced, and what the IME commits. A press with Ctrl or the logo key
/// held types nothing — it is a shortcut — and neither does one whose text
/// is a control character, which the editing keys cover. Editing keys count
/// their repeats: Backspace held keeps deleting.
#[derive(Debug, Clone, Default)]
pub struct Typing {
    input: TextInput,
    shift: bool,
    ctrl: bool,
    alt: bool,
    logo: bool,
}

impl Typing {
    /// The modifiers now held.
    pub fn on_modifiers(&mut self, shift: bool, ctrl: bool, alt: bool, logo: bool) {
        (self.shift, self.ctrl, self.alt, self.logo) = (shift, ctrl, alt, logo);
    }

    /// A key went down (or repeated, `pressed` either way) with the text it
    /// produced, if any. Releases type nothing.
    pub fn on_key(&mut self, code: KeyCode, pressed: bool, text: Option<&str>) {
        if !pressed {
            return;
        }
        let shortcut = self.ctrl || self.logo;
        let edit = match code {
            KeyCode::ArrowLeft => Some(Edit::Left),
            KeyCode::ArrowRight => Some(Edit::Right),
            KeyCode::Home => Some(Edit::Home),
            KeyCode::End => Some(Edit::End),
            KeyCode::Backspace => Some(Edit::Backspace),
            KeyCode::Delete => Some(Edit::Delete),
            KeyCode::Enter | KeyCode::NumpadEnter => Some(Edit::Submit),
            KeyCode::KeyA if shortcut => Some(Edit::SelectAll),
            _ => None,
        };
        if let Some(edit) = edit {
            self.input.keys.push(EditKey {
                edit,
                extend: self.shift,
                // Ctrl on Windows and Linux, Option on macOS.
                word: self.ctrl || self.alt,
            });
            return;
        }
        if let Some(text) = text {
            if !shortcut && !text.is_empty() && !text.chars().any(char::is_control) {
                self.input.typed.push_str(text);
            }
        }
    }

    /// The IME's composition changed; an empty text clears it.
    pub fn on_preedit(&mut self, text: &str, cursor: Option<(usize, usize)>) {
        self.input.preedit = Some(Preedit {
            text: text.to_owned(),
            cursor,
        });
    }

    /// The IME committed `text`.
    pub fn on_commit(&mut self, text: &str) {
        self.input.typed.push_str(text);
        self.input.preedit = Some(Preedit::default());
    }

    /// The IME went away: whatever it was composing is dropped.
    pub fn on_ime_disabled(&mut self) {
        self.input.preedit = Some(Preedit::default());
    }

    /// This frame's typing, for the controls.
    pub fn text(&self) -> &TextInput {
        &self.input
    }

    /// Whether shift is held.
    pub fn shift(&self) -> bool {
        self.shift
    }

    /// Clears the frame's typing; the modifiers stay held.
    pub fn end_frame(&mut self) {
        self.input = TextInput::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_type_their_text_and_editing_keys_edit() {
        let mut typing = Typing::default();
        typing.on_key(KeyCode::KeyH, true, Some("h"));
        typing.on_key(KeyCode::KeyI, true, Some("i"));
        typing.on_key(KeyCode::KeyI, false, Some("i"));
        typing.on_key(KeyCode::Backspace, true, Some("\u{8}"));
        typing.on_key(KeyCode::Backspace, true, Some("\u{8}"));
        assert_eq!(typing.text().typed, "hi");
        assert_eq!(
            typing.text().keys,
            vec![
                EditKey::plain(Edit::Backspace),
                EditKey::plain(Edit::Backspace)
            ],
            "a repeat deletes again"
        );
        typing.end_frame();
        assert!(typing.text().is_empty());
    }

    #[test]
    fn modifiers_select_by_word_and_shortcuts_type_nothing() {
        let mut typing = Typing::default();
        typing.on_modifiers(true, true, false, false);
        typing.on_key(KeyCode::ArrowLeft, true, None);
        typing.on_key(KeyCode::KeyC, true, Some("c"));
        typing.on_key(KeyCode::KeyA, true, Some("a"));
        assert_eq!(typing.text().typed, "");
        assert_eq!(
            typing.text().keys,
            vec![
                EditKey {
                    edit: Edit::Left,
                    extend: true,
                    word: true
                },
                EditKey {
                    edit: Edit::SelectAll,
                    extend: true,
                    word: true
                },
            ]
        );
    }

    #[test]
    fn the_ime_composes_then_commits() {
        let mut typing = Typing::default();
        typing.on_preedit("に", Some((3, 3)));
        assert_eq!(
            typing.text().preedit.as_ref().map(|p| p.text.as_str()),
            Some("に")
        );
        typing.on_commit("日");
        assert_eq!(typing.text().typed, "日");
        assert_eq!(typing.text().preedit, Some(Preedit::default()));
    }
}
