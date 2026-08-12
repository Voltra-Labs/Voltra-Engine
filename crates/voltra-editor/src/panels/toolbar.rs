//! The transport panel: Play/Pause, Step and Stop.
//!
//! Layout and dispatch only. Every transition and every rule about which one is
//! legal lives in [`crate::play`]; this file decides which button is on screen
//! and calls the method behind it, exactly as [`super::menu_bar`] does.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;
use crate::play::PlayState;

/// Square transport buttons, in points.
///
/// Fixed rather than measured, so the row's width is known before it is laid
/// out — which is what lets it be centred in a single pass instead of a sizing
/// pass and a layout pass. Every editor draws these as equal squares anyway.
const BUTTON: f32 = 28.0;

/// How many buttons the row holds. Three, not four: the first is Play or Pause
/// depending on the state, as every transport control is.
const BUTTONS: f32 = 3.0;

pub fn show(editor: &mut Editor, ui: &mut Ui, frame: &mut UiFrame<'_>) {
    egui::Panel::top("transport").show(ui, |ui| {
        ui.horizontal(|ui| {
            let state = editor.play.state();
            let row = BUTTON * BUTTONS + ui.spacing().item_spacing.x * (BUTTONS - 1.0);
            ui.add_space(((ui.available_width() - row) * 0.5).max(0.0));

            let playing = state == PlayState::Playing;
            let play = ui
                .add(transport(if playing { "⏸" } else { "▶" }))
                .on_hover_text(if playing {
                    "Pause — stop stepping and leave the scene where it is"
                } else {
                    "Play — simulate the scene. Stop puts it back as it is now"
                });
            if play.clicked() {
                if playing {
                    editor.pause_play(frame);
                } else {
                    editor.start_play(frame);
                }
            }

            // Enabled only while paused. An enabled-but-inert button teaches the
            // wrong thing about what the state means.
            let step = ui
                .add_enabled(state == PlayState::Paused, transport("⏭"))
                .on_hover_text("Step — run exactly one fixed physics step");
            if step.clicked() {
                editor.step_play(frame);
            }

            // The destructive one, so its tooltip says what it discards.
            let stop = ui
                .add_enabled(state != PlayState::Editing, transport("⏹"))
                .on_hover_text(
                    "Stop — put the scene back as it was when play began, \
                     discarding every change made while playing",
                );
            if stop.clicked() {
                editor.stop_play(frame);
            }
        });
    });
}

/// One square transport button.
fn transport(glyph: &str) -> egui::Button<'_> {
    egui::Button::new(glyph).min_size(egui::Vec2::splat(BUTTON))
}
