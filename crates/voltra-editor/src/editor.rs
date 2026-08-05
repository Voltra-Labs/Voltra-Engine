//! The editor's shared state and its frame layout.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;

use crate::panels;

/// Editor state that outlives a frame.
///
/// egui is immediate mode and remembers nothing between frames, so anything the
/// editor still needs next frame — the selection — has to be kept here.
#[derive(Default)]
pub struct Editor {
    pub(crate) selected: Option<Entity>,
}

impl Editor {
    /// Lays out the whole editor. Called once per frame with the root `Ui`.
    pub fn ui(&mut self, ui: &mut Ui, frame: &mut UiFrame<'_>) {
        panels::menu_bar::show(self, ui, frame);
        panels::hierarchy::show(self, ui, frame);
        panels::inspector::show(self, ui, frame);
        // Last, so it takes whatever room the docked panels left rather than
        // the other way round.
        panels::viewport::show(self, ui, frame);
    }
}
