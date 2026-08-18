//! The toolbar: the transform tools on the left, the transport in the middle.
//!
//! Layout and dispatch only. Every transition and every rule about which one is
//! legal lives in [`crate::play`]; this file decides which button is on screen
//! and calls the method behind it, exactly as [`super::menu_bar`] does.
//!
//! Tools left, transport centred, which is where Unity and Godot both put them.
//! The buttons exist for the same reason those editors keep theirs: `W`/`E`/`R`
//! is faster once known and undiscoverable until then, and the pressed button is
//! also the only always-visible answer to "which tool am I in".

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;
use crate::play::PlayState;
use crate::tool::Tool;

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
            // Measured before anything is drawn: the transport is centred in
            // the whole panel, not in what the tools left over. Centring on the
            // remaining width would slide the transport rightwards every time a
            // tool is added.
            let panel = ui.available_width();

            tools(editor, ui);

            let row = BUTTON * BUTTONS + ui.spacing().item_spacing.x * (BUTTONS - 1.0);
            let used = panel - ui.available_width();
            ui.add_space(((panel - row) * 0.5 - used).max(0.0));

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

/// The transform tools, the active one pressed.
///
/// Enabled in play mode as well as out of it: a gizmo drag during play is a
/// legitimate way to nudge a body and watch what the solver does with it, and
/// Stop puts the scene back regardless.
fn tools(editor: &mut Editor, ui: &mut Ui) {
    for tool in Tool::ALL {
        let button = egui::Button::selectable(editor.tool == tool, tool.glyph())
            .min_size(egui::Vec2::splat(BUTTON));
        if ui.add(button).on_hover_text(tool.hint()).clicked() {
            editor.tool = tool;
        }
    }
}

/// One square transport button.
fn transport(glyph: &str) -> egui::Button<'_> {
    egui::Button::new(glyph).min_size(egui::Vec2::splat(BUTTON))
}
