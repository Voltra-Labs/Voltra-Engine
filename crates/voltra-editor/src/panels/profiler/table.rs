//! The scopes as a tree with their times, then the counters.

use voltra_core::egui::{self, Ui};
use voltra_profile::Summary;

pub(super) fn show(ui: &mut Ui, summary: &Summary) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("profiler_scopes")
            .striped(true)
            .num_columns(6)
            .show(ui, |ui| {
                for heading in ["scope", "calls", "avg ms", "max ms", "own ms", "% frame"] {
                    ui.strong(heading);
                }
                ui.end_row();
                for scope in &summary.scopes {
                    // Indented by depth: the tree without a tree widget.
                    let indent = "    ".repeat(usize::from(scope.depth));
                    ui.label(format!("{indent}{}", scope.name))
                        .on_hover_text(&scope.path);
                    ui.label(format!("{:.1}", scope.calls));
                    ui.label(format!("{:.3}", scope.total.avg));
                    ui.label(format!("{:.3}", scope.total.max));
                    ui.label(format!("{:.3}", scope.own));
                    let share = if summary.frame.avg > 0.0 {
                        scope.total.avg / summary.frame.avg * 100.0
                    } else {
                        0.0
                    };
                    ui.label(format!("{share:.1}"));
                    ui.end_row();
                }
            });
        ui.separator();
        gpu(ui, summary);
        if summary.counters.is_empty() {
            return;
        }
        ui.separator();
        egui::Grid::new("profiler_counters")
            .striped(true)
            .num_columns(4)
            .show(ui, |ui| {
                for heading in ["counter", "avg", "max", "last"] {
                    ui.strong(heading);
                }
                ui.end_row();
                for counter in &summary.counters {
                    ui.label(counter.name);
                    ui.label(format!("{:.1}", counter.avg));
                    ui.label(format!("{}", counter.max));
                    ui.label(format!("{}", counter.last));
                    ui.end_row();
                }
            });
    });
}

/// Each pass's time on the GPU, which comes back frames after it ran.
fn gpu(ui: &mut Ui, summary: &Summary) {
    if summary.gpu.is_empty() {
        ui.weak("no GPU timings: the adapter cannot time passes, or nothing drew yet");
        return;
    }
    let total: f64 = summary.gpu.iter().map(|pass| pass.avg).sum();
    egui::Grid::new("profiler_gpu")
        .striped(true)
        .num_columns(3)
        .show(ui, |ui| {
            for heading in ["GPU pass", "avg ms", "max ms"] {
                ui.strong(heading);
            }
            ui.end_row();
            for pass in &summary.gpu {
                ui.label(pass.name);
                ui.label(format!("{:.3}", pass.avg));
                ui.label(format!("{:.3}", pass.max));
                ui.end_row();
            }
            ui.strong("all passes");
            ui.strong(format!("{total:.3}"));
            ui.end_row();
        });
}
