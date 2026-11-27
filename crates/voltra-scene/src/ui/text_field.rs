//! A line of text the player types: a caret, a selection, IME composition.

mod edit;
pub(crate) mod view;
mod visual;

/// A one-line text box: click it, or move the focus to it, and type.
///
/// Godot's `LineEdit`. The entity's `Text` is the style — font, size,
/// colour, alignment — and the field writes into its content what it
/// shows: the text, the IME's composition at the caret while there is one,
/// or the `placeholder`, drawn fainter, when it is empty. Its `Sprite2D`, if
/// any, is the box. The text scrolls sideways to keep the caret in the box
/// and is cut to it.
///
/// The keys edit it — arrows, Home and End move the caret, with Shift
/// they select, with Ctrl they go a word at a time; Backspace and Delete;
/// Ctrl+A selects everything — and the pointer places the caret and drags
/// a selection. Typed text comes from the window's text events, and IME
/// composition through winit's `Ime` events, which the loop allows only
/// while a field has the focus. Enter submits. One line: a newline or
/// other control character typed or pasted is left out.
///
/// The caret and selection live in the component, not the scene file: a
/// loaded field starts with the caret at the end.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiTextField {
    #[serde(default)]
    /// What the field holds.
    pub text: String,
    /// Shown while the text is empty.
    #[serde(default)]
    pub placeholder: String,
    /// The most characters it takes; `None` for no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_chars: Option<usize>,
    /// Behind selected text.
    #[serde(default = "highlight")]
    pub selection_color: [f32; 4],
    /// Where typing goes, in characters from the start.
    #[serde(skip)]
    pub caret: usize,
    /// The other end of the selection; the caret's own place when nothing
    /// is selected.
    #[serde(skip)]
    pub anchor: usize,
    /// The IME's text being composed at the caret, not yet committed.
    #[serde(skip)]
    pub preedit: Option<Preedit>,
    /// Points the text is scrolled left by, to keep the caret in view.
    #[serde(skip)]
    pub scroll: f32,
    /// Whether it has the focus, as the last update left it: what the draw
    /// reads to show a caret.
    #[serde(skip)]
    pub focused: bool,
}

fn highlight() -> [f32; 4] {
    [0.25, 0.45, 0.9, 0.6]
}

impl Default for UiTextField {
    fn default() -> Self {
        Self {
            text: String::new(),
            placeholder: String::new(),
            max_chars: None,
            selection_color: highlight(),
            caret: 0,
            anchor: 0,
            preedit: None,
            scroll: 0.0,
            focused: false,
        }
    }
}

/// Text an IME is composing, shown at the caret until it commits.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preedit {
    /// The text being composed.
    pub text: String,
    /// The IME's own cursor in it, as byte offsets, or `None` to hide it.
    pub cursor: Option<(usize, usize)>,
}

/// One editing key, as the window reported it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditKey {
    /// What the key does.
    pub edit: Edit,
    /// Shift held: move the caret and keep the selection's other end.
    pub extend: bool,
    /// Ctrl (Option on macOS) held: a word at a time.
    pub word: bool,
}

impl EditKey {
    /// A key doing `edit`, with no modifier held.
    pub fn plain(edit: Edit) -> Self {
        Self {
            edit,
            extend: false,
            word: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// What an editing key does.
pub enum Edit {
    /// Moves the caret one place left.
    Left,
    /// Moves the caret one place right.
    Right,
    /// Moves the caret to the start of the text.
    Home,
    /// Moves the caret to the end of the text.
    End,
    /// Deletes the selection, or what is before the caret.
    Backspace,
    /// Deletes the selection, or what is after the caret.
    Delete,
    /// Selects the whole text.
    SelectAll,
    /// Enter: the text is done.
    Submit,
}

/// The typing this frame, for the focused field.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextInput {
    /// Text typed or committed by the IME, in order.
    pub typed: String,
    /// Editing keys pressed, in order.
    pub keys: Vec<EditKey>,
    /// The IME's composition, when it changed this frame: an empty text
    /// clears it.
    pub preedit: Option<Preedit>,
}

impl TextInput {
    /// Whether nothing was typed, pressed or composed.
    pub fn is_empty(&self) -> bool {
        self.typed.is_empty() && self.keys.is_empty() && self.preedit.is_none()
    }
}

impl UiTextField {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "UiTextField";

    /// A field holding `text`, the caret at its end.
    pub fn with_text(text: impl Into<String>) -> Self {
        let mut field = Self::default();
        field.text = text.into();
        field.caret = field.text.chars().count();
        field.anchor = field.caret;
        field
    }

    /// What it shows: the text with the composition at the caret, or the
    /// placeholder, and whether that is the placeholder.
    pub fn display(&self) -> (String, bool) {
        let preedit = self.preedit.as_ref().filter(|p| !p.text.is_empty());
        if self.text.is_empty() && preedit.is_none() {
            return (self.placeholder.clone(), true);
        }
        let Some(preedit) = preedit else {
            return (self.text.clone(), false);
        };
        let at = self.byte_at(self.caret);
        let mut shown = String::with_capacity(self.text.len() + preedit.text.len());
        shown.push_str(&self.text[..at]);
        shown.push_str(&preedit.text);
        shown.push_str(&self.text[at..]);
        (shown, false)
    }

    /// Where the caret is drawn in what [`display`](Self::display) shows,
    /// in characters: after the composition's own cursor while composing.
    pub fn display_caret(&self) -> usize {
        match self.preedit.as_ref().filter(|p| !p.text.is_empty()) {
            Some(preedit) => {
                let inside = preedit
                    .cursor
                    .and_then(|(_, end)| preedit.text.get(..end))
                    .map_or(preedit.text.chars().count(), |s| s.chars().count());
                self.caret + inside
            }
            None => self.caret,
        }
    }

    /// The composition's span in what is shown, in characters, if any.
    pub fn display_preedit(&self) -> Option<(usize, usize)> {
        let preedit = self.preedit.as_ref().filter(|p| !p.text.is_empty())?;
        Some((self.caret, self.caret + preedit.text.chars().count()))
    }
}

#[cfg(test)]
mod tests;
