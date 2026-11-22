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
