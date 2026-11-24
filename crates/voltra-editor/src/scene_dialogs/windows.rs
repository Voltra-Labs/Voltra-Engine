//! The three scene windows: "save changes?", Open and Save As. Modal, so
//! nothing behind them edits the scene they are about.

use voltra_core::egui::{self, Context, Id, Key, Modal};
use voltra_core::UiFrame;

use super::{perform, request, show_save_as, OpenDialog, SaveAsDialog, Then};
use crate::editor::Editor;
use crate::scene_file;

/// Draws whichever window is up. One at a time: each is opened only from a
/// state with none up, or replaces the one that opened it.
pub(super) fn show(editor: &mut Editor, ctx: &Context, frame: &mut UiFrame<'_>) {
    if let Some(then) = editor.dialogs.unsaved.take() {
        unsaved(editor, ctx, frame, then);
    } else if let Some(dialog) = editor.dialogs.open.take() {
        open(editor, ctx, frame, dialog);
    } else if let Some(dialog) = editor.dialogs.save_as.take() {
        save_as(editor, ctx, frame, dialog);
    }
}

/// What the person answered.
enum Answer {
    Save,
    Discard,
    Cancel,
}

fn unsaved(editor: &mut Editor, ctx: &Context, frame: &mut UiFrame<'_>, then: Then) {
    let doing = match &then {
        Then::New => "making a new scene".to_owned(),
        Then::Open(path) => format!("opening {}", path.as_str()),
        Then::Quit => "closing".to_owned(),
        Then::Back => format!(
            "returning to {}",
            editor.prefab_mode.returns_to().unwrap_or("the scene")
        ),
    };
    let question = format!("Save changes to {} before {doing}?", editor.document.name());
    let error = editor.dialogs.unsaved_error.clone();
    let shown = Modal::new(Id::new("scene_unsaved")).show(ctx, |ui| {
        ui.heading("Unsaved changes");
        ui.label(question);
        if let Some(error) = &error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                return Some(Answer::Save);
            }
            if ui.button("Don't Save").clicked() {
                return Some(Answer::Discard);
            }
            ui.button("Cancel").clicked().then_some(Answer::Cancel)
        })
        .inner
    });
    let close = shown.should_close();
    let answer = shown.inner.or(close.then_some(Answer::Cancel));
    match answer {
        None => editor.dialogs.unsaved = Some(then),
        Some(Answer::Cancel) => {
            if then == Then::Quit {
                frame.cancel_close();
            }
        }
        Some(Answer::Discard) => {
            perform(editor, frame, then);
        }
        // Untitled: the file is named first, and `then` waits on that save.
        Some(Answer::Save) if editor.document.path().is_none() => {
            show_save_as(editor, Some(then));
        }
        Some(Answer::Save) => match scene_file::save(editor, frame) {
            Ok(_) => {
                perform(editor, frame, then);
            }
            Err(e) => {
                log::error!("{e}");
                editor.dialogs.unsaved_error = Some(e);
                editor.dialogs.unsaved = Some(then);
            }
        },
    }
}

fn open(editor: &mut Editor, ctx: &Context, frame: &mut UiFrame<'_>, mut dialog: OpenDialog) {
    let mut chosen = None;
    let mut cancel = false;
    let shown = Modal::new(Id::new("scene_open")).show(ctx, |ui| {
        ui.heading("Open scene");
        ui.horizontal(|ui| {
            ui.label("Filter");
            ui.text_edit_singleline(&mut dialog.filter).request_focus();
        });
        let filter = dialog.filter.to_lowercase();
        egui::ScrollArea::vertical()
            .max_height(320.0)
            .show(ui, |ui| {
                let mut any = false;
                for path in &dialog.scenes {
                    if !path.as_str().to_lowercase().contains(&filter) {
                        continue;
                    }
                    any = true;
                    if ui.selectable_label(false, path.as_str()).clicked() {
                        chosen = Some(path.clone());
                    }
                }
                if !any {
                    ui.weak(if dialog.scenes.is_empty() {
                        "no scenes under the asset root"
                    } else {
                        "no scene matches"
                    });
                }
            });
        if let Some(error) = &dialog.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        cancel = ui.button("Cancel").clicked();
    });
    if let Some(path) = chosen {
        // Through the prompt like every other way to Open.
        request(editor, frame, Then::Open(path));
    } else if !(cancel || shown.should_close()) {
        editor.dialogs.open = Some(dialog);
    }
}

fn save_as(editor: &mut Editor, ctx: &Context, frame: &mut UiFrame<'_>, mut dialog: SaveAsDialog) {
    let mut save = false;
    let mut cancel = false;
    let shown = Modal::new(Id::new("scene_save_as")).show(ctx, |ui| {
        ui.heading("Save scene as");
        ui.label("A path under the asset root, ending in .ron:");
        let field = ui.text_edit_singleline(&mut dialog.text);
        field.request_focus();
        save = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        if let Some(error) = &dialog.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.horizontal(|ui| {
            save |= ui.button("Save").clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    if cancel || (!save && shown.should_close()) {
        if dialog.then == Some(Then::Quit) {
            frame.cancel_close();
        }
        return;
    }
    if save {
        match scene_file::scene_path(&dialog.text) {
            Err(e) => dialog.error = Some(e),
            Ok(path)
                if scene_file::file_of(frame, &path).exists()
                    && dialog.overwrite.as_ref() != Some(&path)
                    && editor.document.path() != Some(&path) =>
            {
                dialog.error = Some(format!(
                    "{} already exists: Save again to replace it",
                    path.as_str()
                ));
                dialog.overwrite = Some(path);
            }
            Ok(path) => match scene_file::save_as(editor, frame, path) {
                Ok(()) => {
                    if let Some(then) = dialog.then.take() {
                        perform(editor, frame, then);
                    }
                    return;
                }
                Err(e) => dialog.error = Some(e),
            },
        }
    }
    editor.dialogs.save_as = Some(dialog);
}
