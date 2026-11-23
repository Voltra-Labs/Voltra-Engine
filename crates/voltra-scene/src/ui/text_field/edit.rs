//! The editing a field takes: typing, deleting, moving and selecting, in
//! characters rather than bytes so a caret never lands inside one.

use super::{Edit, EditKey, Preedit, TextInput, UiTextField};

/// What an edit did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Edited {
    /// The text changed.
    pub changed: bool,
    /// Enter was pressed.
    pub submitted: bool,
}

impl UiTextField {
    fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// The byte where character `index` starts, or the end.
    pub(crate) fn byte_at(&self, index: usize) -> usize {
        self.text
            .char_indices()
            .nth(index)
            .map_or(self.text.len(), |(byte, _)| byte)
    }

    /// The caret and anchor kept inside the text: a scene edit or a
    /// shortened text may have left them past its end.
    fn settle(&mut self) {
        let len = self.len();
        self.caret = self.caret.min(len);
        self.anchor = self.anchor.min(len);
    }

    /// The selected characters, start to end; empty when nothing is.
    pub fn selection(&self) -> std::ops::Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    pub fn selected(&self) -> &str {
        let range = self.selection();
        &self.text[self.byte_at(range.start)..self.byte_at(range.end)]
    }

    /// Takes the selection out; `true` if there was one.
    fn delete_selection(&mut self) -> bool {
        let range = self.selection();
        if range.is_empty() {
            return false;
        }
        let (from, to) = (self.byte_at(range.start), self.byte_at(range.end));
        self.text.replace_range(from..to, "");
        self.caret = range.start;
        self.anchor = range.start;
        true
    }

    /// Types `typed` at the caret over the selection: control characters
    /// left out, and no more than `max_chars` kept. `true` if the text
    /// changed.
    pub fn insert(&mut self, typed: &str) -> bool {
        self.settle();
        let clean: String = typed.chars().filter(|c| !c.is_control()).collect();
        if clean.is_empty() {
            return false;
        }
        let mut changed = self.delete_selection();
        let room = self
            .max_chars
            .map_or(usize::MAX, |max| max.saturating_sub(self.len()));
        let kept: String = clean.chars().take(room).collect();
        if !kept.is_empty() {
            let at = self.byte_at(self.caret);
            self.text.insert_str(at, &kept);
            self.caret += kept.chars().count();
            self.anchor = self.caret;
            changed = true;
        }
        changed
    }

    /// Puts the caret at character `index`, keeping the anchor when
    /// `extend` — a shift-click or a drag.
    pub fn place_caret(&mut self, index: usize, extend: bool) {
        self.caret = index.min(self.len());
        if !extend {
            self.anchor = self.caret;
        }
    }

    /// Applies one key.
    pub fn apply(&mut self, key: EditKey) -> Edited {
        self.settle();
        let mut edited = Edited::default();
        let chars: Vec<char> = self.text.chars().collect();
        match key.edit {
            Edit::Left | Edit::Right if !key.extend && !self.selection().is_empty() => {
                // An arrow collapses a selection to its side, as everywhere.
                let range = self.selection();
                let to = if key.edit == Edit::Left {
                    range.start
                } else {
                    range.end
                };
                self.place_caret(to, false);
            }
            Edit::Left => {
                let to = if key.word {
                    word_start(&chars, self.caret)
                } else {
                    self.caret.saturating_sub(1)
                };
                self.place_caret(to, key.extend);
            }
            Edit::Right => {
                let to = if key.word {
                    word_end(&chars, self.caret)
                } else {
                    (self.caret + 1).min(chars.len())
                };
                self.place_caret(to, key.extend);
            }
            Edit::Home => self.place_caret(0, key.extend),
            Edit::End => self.place_caret(chars.len(), key.extend),
            Edit::SelectAll => {
                self.anchor = 0;
                self.caret = chars.len();
            }
            Edit::Backspace | Edit::Delete => {
                if self.delete_selection() {
                    edited.changed = true;
                } else {
                    let (from, to) = match (key.edit, key.word) {
                        (Edit::Backspace, false) => (self.caret.saturating_sub(1), self.caret),
                        (Edit::Backspace, true) => (word_start(&chars, self.caret), self.caret),
                        (_, false) => (self.caret, (self.caret + 1).min(chars.len())),
                        (_, true) => (self.caret, word_end(&chars, self.caret)),
                    };
                    if from < to {
                        self.anchor = from;
                        self.caret = to;
                        edited.changed = self.delete_selection();
                    }
                }
            }
            Edit::Submit => edited.submitted = true,
        }
        edited
    }

    /// Takes the IME's composition, as it changed.
    pub fn set_preedit(&mut self, preedit: Preedit) {
        self.preedit = (!preedit.text.is_empty()).then_some(preedit);
    }

    /// Applies a frame's typing: the composition, the typed text, then the
    /// keys, in that order — a commit arrives with the composition it ends
    /// already cleared.
    pub fn take(&mut self, input: &TextInput) -> Edited {
        let mut edited = Edited::default();
        if let Some(preedit) = &input.preedit {
            self.set_preedit(preedit.clone());
        }
        if !input.typed.is_empty() {
            // Text committed ends any composition still shown.
            self.preedit = None;
            edited.changed |= self.insert(&input.typed);
        }
        for &key in &input.keys {
            let one = self.apply(key);
            edited.changed |= one.changed;
            edited.submitted |= one.submitted;
        }
        edited
    }
}

/// The start of the word before `at`: back over spaces, then over the word.
fn word_start(chars: &[char], at: usize) -> usize {
    let mut i = at.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

/// The end of the word after `at`: over spaces, then over the word.
fn word_end(chars: &[char], at: usize) -> usize {
    let mut i = at.min(chars.len());
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    while i < chars.len() && !chars[i].is_whitespace() {
        i += 1;
    }
    i
}
