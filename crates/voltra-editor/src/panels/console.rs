//! The Console window: the log, filtered by level and by text.

use log::Level;
use voltra_core::egui::{self, Color32, RichText, Ui};

use crate::editor::Editor;

pub(crate) fn show(editor: &mut Editor, ui: &mut Ui) {
    editor.console.read();
    if !editor.console.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Console")
        .open(&mut open)
        .default_size([640.0, 320.0])
        .show(ui.ctx(), |ui| contents(editor, ui));
    editor.console.open = open;
}

fn contents(editor: &mut Editor, ui: &mut Ui) {
    let console = &mut editor.console;
    let [errors, warnings, infos] = console.counts();
    ui.horizontal(|ui| {
        for (level, label) in [
            (Level::Error, format!("errors {errors}")),
            (Level::Warn, format!("warnings {warnings}")),
            (Level::Info, format!("info {infos}")),
            (Level::Debug, "debug".to_owned()),
            (Level::Trace, "trace".to_owned()),
        ] {
            ui.selectable_value(&mut console.level, level, label)
                .on_hover_text("this level and louder");
        }
        ui.separator();
        ui.add(egui::TextEdit::singleline(&mut console.filter).hint_text("filter"));
        if ui.button("Clear").clicked() {
            console.clear();
        }
    });
    if console.missed() > 0 {
        ui.weak(format!(
            "{} lines came faster than they were read",
            console.missed()
        ));
    }
    ui.separator();
    let (level, filter) = (console.level, console.filter.clone());
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for line in console.shown(level, &filter) {
                let colour = match line.level {
                    Level::Error => Color32::from_rgb(240, 90, 90),
                    Level::Warn => Color32::from_rgb(230, 190, 80),
                    Level::Info => ui.visuals().text_color(),
                    Level::Debug | Level::Trace => ui.visuals().weak_text_color(),
                };
                ui.label(
                    RichText::new(format!(
                        "{:5} {}  {}",
                        line.level, line.target, line.message
                    ))
                    .monospace()
                    .color(colour),
                );
            }
        });
}
