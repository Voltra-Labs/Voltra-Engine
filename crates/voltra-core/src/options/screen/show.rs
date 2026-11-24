//! The screen drawn from its state: every label and value as the settings
//! and the action map now are — once a frame, so a change made anywhere,
//! a reset or a reload of the map, shows.

use voltra_scene::{Text, UiSlider, UiToggle};

use super::rows::say;
use super::{labels, OptionsScreen, CLASH_COLOR};
use crate::app::Tick;

impl OptionsScreen {
    /// Every label and value as the settings and the map now are.
    pub(super) fn show_everything(&self, tick: &mut Tick<'_>) {
        let world = &mut *tick.world;
        say(
            world,
            self.display_label,
            &labels::display(self.settings.fullscreen),
        );
        let size = (!self.resolutions.is_empty()).then(|| {
            self.settings
                .windowed_size
                .unwrap_or_else(|| tick.window.windowed_size())
        });
        say(world, self.resolution_label, &labels::resolution(size));
        let can_vsync = tick.window.abilities().vsync;
        let vsync = self.settings.vsync || !can_vsync;
        if let Some(toggle) = self.vsync.and_then(|e| world.get_mut::<UiToggle>(e)) {
            toggle.on = vsync;
        }
        say(world, self.vsync_label, &labels::vsync(vsync, can_vsync));
        for volume in &self.volumes {
            let value = self.settings.volume(&volume.bus);
            if let Some(slider) = world.get_mut::<UiSlider>(volume.row.control) {
                slider.set(value);
            }
            say(world, volume.row.label, &labels::volume(&volume.bus, value));
        }
        let listed: Vec<(&str, &str)> = self
            .bindings
            .iter()
            .map(|b| (b.action.as_str(), b.title.as_str()))
            .collect();
        for (index, row) in self.bindings.iter().enumerate() {
            let (words, clashes) = if self.waiting == Some(index) {
                (labels::waiting(&row.title), Vec::new())
            } else {
                labels::binding(tick.actions, &row.action, &row.title, &listed)
            };
            say(world, row.row.label, &words);
            if let Some(text) = row.row.label.and_then(|e| world.get_mut::<Text>(e)) {
                text.color = if clashes.is_empty() {
                    row.color
                } else {
                    CLASH_COLOR
                };
            }
        }
    }
}
