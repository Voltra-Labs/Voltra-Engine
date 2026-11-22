//! The Profiler window: what each frame cost, and where it went.
//!
//! Unity's Profiler window in miniature — a graph of frame times over the
//! last seconds and a hierarchy of scopes with their total and own time —
//! plus Godot's monitors as a list of counters. The numbers are this
//! thread's [`voltra_profile`] recorder: the editor's own loop, and the game
//! it is playing.

mod graph;
mod table;

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;

use crate::editor::Editor;

/// How many frames the table and the headline average over: two seconds at
/// 60 frames a second, steady enough to read, recent enough to react.
const SUMMARY_FRAMES: usize = 120;

/// Whether the window is up.
#[derive(Debug, Default)]
pub(crate) struct ProfilerPanel {
    open: bool,
}

impl ProfilerPanel {
    /// Puts the window up and starts recording, as Unity's does: opening a
    /// profiler to an empty graph would be a step for nothing.
    pub(crate) fn open(&mut self) {
        self.open = true;
        voltra_profile::set_enabled(true);
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }
}

pub(crate) fn show(editor: &mut Editor, ui: &mut Ui, _frame: &mut UiFrame<'_>) {
    if !editor.profiler.open {
        return;
    }
    let mut open = true;
    egui::Window::new("Profiler")
        .open(&mut open)
        .default_size([560.0, 560.0])
        .show(ui.ctx(), contents);
    // Closing the window leaves recording as it is: a tool may be reading it.
    editor.profiler.open = open;
}

fn contents(ui: &mut Ui) {
    ui.horizontal(|ui| {
        let mut recording = voltra_profile::is_enabled();
        if ui.checkbox(&mut recording, "Record").changed() {
            voltra_profile::set_enabled(recording);
        }
        if ui.button("Clear").clicked() {
            voltra_profile::clear();
        }
    });
    let summary = voltra_profile::summary(SUMMARY_FRAMES);
    if summary.frames == 0 {
        ui.weak(if voltra_profile::is_enabled() {
            "recording: the first frames are on their way"
        } else {
            "not recording: tick Record"
        });
        return;
    }
    let fps = if summary.frame.avg > 0.0 {
        1000.0 / summary.frame.avg
    } else {
        0.0
    };
    ui.label(format!(
        "frame {:.2} ms ({fps:.0} fps), worst {:.2} ms, over the last {} frames",
        summary.frame.avg, summary.frame.max, summary.frames
    ));
    voltra_profile::with_frames(|frames| graph::show(ui, frames));
    ui.separator();
    table::show(ui, &summary);
}
