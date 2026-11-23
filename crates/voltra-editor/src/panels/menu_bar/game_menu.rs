//! The Game menu: which game is loaded, and loading or unloading it.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;

pub(super) fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    ui.menu_button("Game", |ui| {
        match (editor.game.loaded(), editor.game.problem()) {
            (Some(path), _) => {
                let name = path.file_name().map_or_else(
                    || path.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                ui.label(format!("Playing {name}"))
                    .on_hover_text(path.display().to_string());
            }
            (None, Some(problem)) => {
                ui.colored_label(egui::Color32::LIGHT_RED, "No game loaded")
                    .on_hover_text(problem.to_owned());
            }
            (None, None) => {
                ui.label("No game")
                    .on_hover_text("build.ron names no game crate; Play runs the physics alone");
            }
        }
        if editor.game.reload.building() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Building…");
            });
        } else if let Some(errors) = editor.game.reload.failure() {
            ui.colored_label(egui::Color32::LIGHT_RED, "Build failed: the old code runs")
                .on_hover_text(errors.to_owned());
            ui.label(egui::RichText::new(errors).small().monospace());
        }
        ui.separator();
        let loaded = editor.game.loaded().is_some();
        if ui
            .add_enabled(
                editor.game.reload.can_build() && !editor.game.reload.building(),
                egui::Button::new("Rebuild game"),
            )
            .on_hover_text("build the game's crate and swap it in, as a change to its sources does")
            .on_disabled_hover_text("building, or a game named by a library rather than a crate")
            .clicked()
        {
            editor.game.reload.rebuild();
            ui.close();
        }
        if ui
            .button(if loaded { "Reload game" } else { "Load game" })
            .on_hover_text(
                "load the game library again — after `cargo build -p voltra-editor -p <game>`",
            )
            .clicked()
        {
            crate::game::load(editor, frame);
            ui.close();
        }
        if ui
            .add_enabled(loaded, egui::Button::new("Unload game"))
            .on_hover_text(
                "its components stay in the scene, kept as they are, until it is loaded again",
            )
            .clicked()
        {
            crate::game::unload(editor, frame);
            ui.close();
        }
    });
}
