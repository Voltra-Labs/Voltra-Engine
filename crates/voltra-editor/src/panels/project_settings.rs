//! The Project Settings window: the input map, the sorting layers and the
//! build manifest, edited as the RON files they are and saved only when
//! they read.

use std::collections::HashMap;

use voltra_core::egui::{self, Color32, RichText, Ui};

use crate::editor::Editor;
use crate::project_settings::{self, SettingsFile};

/// The window, its tab and each file's text being edited.
#[derive(Default)]
pub(crate) struct ProjectSettingsWindow {
    pub(crate) open: bool,
    tab: usize,
    /// Each file's text as edited, read when the window first shows it.
    buffers: HashMap<usize, String>,
    /// What the last save or check said, per file.
    status: HashMap<usize, Result<String, String>>,
}

impl ProjectSettingsWindow {
    pub(crate) fn open(&mut self) {
        self.open = true;
        // Read afresh: the files may have changed on disk since.
        self.buffers.clear();
        self.status.clear();
    }
}

/// `assets` is the asset root the editor runs on.
pub(crate) fn show(editor: &mut Editor, ui: &mut Ui, assets: &std::path::Path) {
    if !editor.settings.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Project Settings")
        .open(&mut open)
        .default_size([560.0, 480.0])
        .show(ui.ctx(), |ui| {
            let window = &mut editor.settings;
            ui.horizontal(|ui| {
                for (i, file) in SettingsFile::ALL.iter().enumerate() {
                    ui.selectable_value(&mut window.tab, i, file.label());
                }
            });
            ui.separator();
            let file = SettingsFile::ALL[window.tab];
            let path = file.path(assets);
            ui.weak(path.display().to_string());
            let text = window.buffers.entry(window.tab).or_insert_with(|| {
                project_settings::read(file, assets).unwrap_or_else(|e| {
                    log::warn!("{e}");
                    String::new()
                })
            });
            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 60.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(text)
                            .code_editor()
                            .desired_width(f32::INFINITY)
                            .desired_rows(16),
                    );
                });
            let (mut save, mut check, mut revert) = (false, false, false);
            ui.horizontal(|ui| {
                save = ui.button("Save").clicked();
                check = ui.button("Check").clicked();
                revert = ui.button("Revert").clicked();
            });
            if save {
                let result = project_settings::write(file, assets, text)
                    .map(|path| format!("saved {}", path.display()));
                window.status.insert(window.tab, result);
            } else if check {
                let result = file.check(text, &path).map(|()| "reads".to_owned());
                window.status.insert(window.tab, result);
            } else if revert {
                window.buffers.remove(&window.tab);
                window.status.remove(&window.tab);
            }
            match window.status.get(&window.tab) {
                Some(Ok(message)) => {
                    ui.label(RichText::new(message).color(Color32::from_rgb(120, 200, 120)));
                }
                Some(Err(error)) => {
                    ui.label(RichText::new(error).color(Color32::from_rgb(240, 90, 90)));
                }
                None => {}
            }
        });
    editor.settings.open = open;
}
