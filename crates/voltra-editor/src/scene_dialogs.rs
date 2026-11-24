//! Replacing the open scene, and closing the editor, without losing work.
//!
//! Every way to New, Open or Quit — the menu, the keys, a double-click in the
//! asset browser, the window's close button — goes through [`request`]: with
//! unsaved changes it asks first (Save, Don't Save, Cancel, as every editor
//! does), otherwise it goes ahead. Open and Save As have windows of their
//! own; see [`windows`].
//!
//! So does leaving prefab mode ([`Then::Back`]), and a New, Open or Quit
//! asked for in it returns level by level first, each document asked about
//! in turn: the scene set aside under a prefab has unsaved changes of its
//! own, and none of the three may drop them unasked.

mod windows;

use voltra_assets::browse::EntryKind;
use voltra_assets::AssetPath;
use voltra_core::egui::{Context, Key, KeyboardShortcut, Modifiers, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;
use crate::prefab_mode::Pending;
use crate::scene_file;

/// What happens once the open scene may be let go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Then {
    New,
    Open(AssetPath),
    Quit,
    /// Leave the prefab being edited for the document under it.
    Back,
}

/// Which of the scene windows is up, and what it holds.
#[derive(Debug, Default)]
pub(crate) struct SceneDialogs {
    /// "Save changes?", and what waits on the answer.
    unsaved: Option<Then>,
    /// Why the save the prompt asked for failed, shown in it.
    unsaved_error: Option<String>,
    open: Option<OpenDialog>,
    save_as: Option<SaveAsDialog>,
    /// The scene the command line named, opened on the first frame: opening
    /// needs the frame's stores.
    startup: Option<AssetPath>,
}

#[derive(Debug)]
struct OpenDialog {
    filter: String,
    /// Every scene under the asset root, read when the window opened.
    scenes: Vec<AssetPath>,
    error: Option<String>,
}

#[derive(Debug)]
struct SaveAsDialog {
    text: String,
    error: Option<String>,
    /// The path the person was warned exists; saving to it again overwrites.
    overwrite: Option<AssetPath>,
    /// What waits on the save — a Quit whose "save changes?" said Save on
    /// an untitled scene.
    then: Option<Then>,
}

impl SceneDialogs {
    /// Opens `path` on the first frame, in place of whatever `main` put in
    /// the world.
    pub(crate) fn open_at_start(&mut self, path: AssetPath) {
        self.startup = Some(path);
    }

    /// Whether a scene window is up: what a close request must not stack
    /// another prompt on.
    fn busy(&self) -> bool {
        self.unsaved.is_some() || self.open.is_some() || self.save_as.is_some()
    }
}

/// Lets the open scene go for `then` — at once when nothing would be lost,
/// otherwise once the person has answered "save changes?".
pub(crate) fn request(editor: &mut Editor, frame: &mut UiFrame<'_>, then: Then) {
    if editor.document.is_dirty(&editor.history) {
        editor.dialogs.unsaved = Some(then);
        editor.dialogs.unsaved_error = None;
    } else {
        perform(editor, frame, then);
    }
}

/// Does `then`, the open scene already let go. A scene that will not open
/// is logged and leaves the open one as it was.
fn perform(editor: &mut Editor, frame: &mut UiFrame<'_>, then: Then) {
    if editor.prefab_mode.is_active() && then != Then::Back {
        // The prefab was answered for; the document under it is asked about
        // next, and so on down to the scene, which `then` is about.
        match crate::prefab_mode::leave(editor, frame) {
            Ok(_) => request(editor, frame, then),
            Err(e) => log::error!("{e}"),
        }
        return;
    }
    match then {
        Then::Back => {
            if let Err(e) = crate::prefab_mode::leave(editor, frame) {
                log::error!("{e}");
            }
        }
        Then::New => scene_file::new_scene(editor, frame),
        Then::Open(path) => {
            if let Err(e) = scene_file::open(editor, frame, &path) {
                log::error!("{e}");
            }
        }
        Then::Quit => frame.close(),
    }
}

/// Saves to the scene's file, or asks for one when it has none.
pub(crate) fn save(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    if editor.document.path().is_none() {
        show_save_as(editor, None);
        return;
    }
    match scene_file::save(editor, frame) {
        Ok(path) => log::info!("scene saved to {}", path.as_str()),
        Err(e) => log::error!("{e}"),
    }
}

/// Puts up the Open window over every scene under the asset root.
pub(crate) fn show_open(editor: &mut Editor, frame: &UiFrame<'_>) {
    editor.dialogs.open = Some(OpenDialog {
        filter: String::new(),
        scenes: voltra_assets::browse::find(frame.textures.root(), EntryKind::Scene),
        error: None,
    });
}

/// Puts up the Save As window, starting from the scene's own path.
///
/// Not in prefab mode, as in Unity's: the prefab is its file, and its
/// instances name that file. Save writes it.
pub(crate) fn show_save_as(editor: &mut Editor, then: Option<Then>) {
    if editor.prefab_mode.is_active() {
        log::warn!("a prefab is saved to its own file: Save writes it, and Save As is not offered");
        return;
    }
    let text = editor
        .document
        .path()
        .map_or(scene_file::UNTITLED, AssetPath::as_str)
        .to_owned();
    editor.dialogs.save_as = Some(SaveAsDialog {
        text,
        error: None,
        overwrite: None,
        then,
    });
}

/// The frame's part: the title, a close the person asked for, the scene
/// keys, and whichever window is up.
pub(crate) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    if let Some(path) = editor.dialogs.startup.take() {
        if let Err(e) = scene_file::open(editor, frame, &path) {
            log::error!("{e}");
        }
    }
    // What a panel asked of prefab mode last frame, before any panel draws
    // the world it replaces.
    match editor.prefab_mode.take_pending() {
        Some(Pending::Edit(path)) => {
            if let Err(e) = crate::prefab_mode::enter(editor, frame, &path) {
                log::error!("{e}");
            }
        }
        Some(Pending::Back) if !editor.dialogs.busy() => request(editor, frame, Then::Back),
        _ => {}
    }
    frame.set_title(crate::prefab_mode::title(editor));
    if frame.close_requested() && !editor.dialogs.busy() {
        request(editor, frame, Then::Quit);
    }
    if !editor.dialogs.busy() {
        keys(editor, ui.ctx(), frame);
    }
    windows::show(editor, ui.ctx(), frame);
}

/// Ctrl+N, Ctrl+O, Ctrl+S and Ctrl+Shift+S, wherever the focus is: a scene
/// command is not text a field could want.
fn keys(editor: &mut Editor, ctx: &Context, frame: &mut UiFrame<'_>) {
    let shortcut = |modifiers, key| KeyboardShortcut::new(modifiers, key);
    let (new, open, save_as, save) = ctx.input_mut(|input| {
        (
            input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::N)),
            input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::O)),
            // Before plain Ctrl+S, which would otherwise take it.
            input.consume_shortcut(&shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)),
            input.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::S)),
        )
    });
    if new {
        request(editor, frame, Then::New);
    } else if open {
        show_open(editor, frame);
    } else if save_as {
        show_save_as(editor, None);
    } else if save {
        self::save(editor, frame);
    }
}
