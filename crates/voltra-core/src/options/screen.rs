//! The options screen while it is open: spawned from its prefab, kept in
//! step with what the player changes, saved and taken away when closed.

mod config;
mod display;
mod labels;
mod rows;
mod show;

pub use config::{OptionsConfig, OptionsError, Outcome};

use std::time::Duration;

use voltra_assets::AssetPath;
use voltra_ecs::Entity;
use voltra_scene::{hierarchy, PrefabInstance, PrefabPart, Text, Transform2D};
use voltra_scene::{UiButton, UiDropdown, UiSlider, UiToggle};

use super::settings::Settings;
use crate::app::Tick;
use crate::prefab_parts::Parts;
use crate::window::Fullscreen;
use rows::Row;

/// The action that backs out: Escape, or a pad's East, in the default map.
const CANCEL: &str = "ui_cancel";

/// What a row that clashes with another is drawn in.
const CLASH_COLOR: [f32; 4] = [1.0, 0.45, 0.4, 1.0];

/// One action's row of the controls list.
#[derive(Debug)]
struct BindingRow {
    action: String,
    title: String,
    row: Row,
    /// The label's colour as authored, put back once a clash is gone.
    color: [f32; 4],
}

/// One bus's slider.
#[derive(Debug)]
struct VolumeRow {
    bus: String,
    row: Row,
}

/// The options screen, open: the entities it spawned and what they stand
/// for. Hold one while it shows, call [`update`](Self::update) every frame
/// from the per-frame tick, and drop it when that answers
/// [`Outcome::Closed`].
///
/// Every change is carried out as it is made — the window from the next
/// frame, a volume at once, a binding from the next frame — and kept when
/// the screen closes.
#[derive(Debug)]
pub struct OptionsScreen {
    root: Entity,
    /// A dropdown of [`modes`](Self::modes).
    display_mode: Option<Entity>,
    /// A dropdown of [`resolutions`](Self::resolutions).
    resolution: Option<Entity>,
    vsync: Option<Entity>,
    vsync_label: Option<Entity>,
    reset: Option<Entity>,
    back: Option<Entity>,
    volumes: Vec<VolumeRow>,
    bindings: Vec<BindingRow>,
    resolutions: Vec<(u32, u32)>,
    /// The display modes offered, at the chosen size.
    modes: Vec<Fullscreen>,
    settings: Settings,
    /// The row whose action takes the next control pressed.
    waiting: Option<usize>,
}

impl OptionsScreen {
    /// Spawns the screen over whatever is showing, fills its lists, and
    /// gives the keyboard and pad its first control.
    pub fn open(tick: &mut Tick<'_>, config: &OptionsConfig) -> Result<Self, OptionsError> {
        let path = AssetPath::new(&config.prefab)
            .map_err(|_| OptionsError::BadPath(config.prefab.clone()))?;
        let prefab = tick.prefab(&path);
        let instance = tick
            .spawn(prefab, Transform2D::default())
            .map_err(OptionsError::Spawn)?;
        let root = instance
            .root()
            .ok_or_else(|| OptionsError::Empty(config.prefab.clone()))?;
        // The screen is the code's from here on: a prefab sync must not
        // put back the texts and values it writes.
        for &entity in &instance.entities {
            tick.world.remove::<PrefabInstance>(entity);
            tick.world.remove::<PrefabPart>(entity);
        }
        let parts = Parts::of(tick.world, &instance.entities, "options");

        let mut settings = Settings::current(tick);
        if tick.saves.dir().is_some() {
            match Settings::load(tick.saves) {
                Ok(Some(kept)) => settings.windowed_size = kept.windowed_size,
                Ok(None) => {}
                Err(e) => log::warn!("{e}; the screen starts from the game as it runs"),
            }
        }
        let size = settings
            .windowed_size
            .unwrap_or_else(|| tick.window.windowed_size());
        let resolutions = display::resolutions(tick.window, size);
        let modes = display::modes(tick.window, size);

        let buses = config
            .buses
            .clone()
            .unwrap_or_else(|| tick.audio.buses().names().map(str::to_owned).collect());
        let volumes = match (parts.get("Volumes"), parts.get("Volume row")) {
            (Some(list), Some(template)) => {
                rows::stamp(tick, list, template, "Volume label", "Volume", &buses)
                    .into_iter()
                    .map(|(bus, row)| VolumeRow { bus, row })
                    .collect()
            }
            _ => Vec::new(),
        };

        let actions: Vec<(String, String)> = if config.actions.is_empty() {
            tick.actions
                .names()
                .filter(|name| !name.starts_with("ui_"))
                .map(|name| (name.to_owned(), labels::humanize(name)))
                .collect()
        } else {
            config.actions.clone()
        };
        let names: Vec<String> = actions.iter().map(|(name, _)| name.clone()).collect();
        let bindings = match (parts.get("Bindings"), parts.get("Binding row")) {
            (Some(list), Some(template)) => {
                rows::stamp(tick, list, template, "Binding label", "Binding", &names)
                    .into_iter()
                    .map(|(action, row)| {
                        let title = actions
                            .iter()
                            .find(|(name, _)| *name == action)
                            .map_or_else(|| labels::humanize(&action), |(_, t)| t.clone());
                        let color = row
                            .label
                            .and_then(|e| tick.world.get::<Text>(e))
                            .map_or([1.0; 4], |text| text.color);
                        BindingRow {
                            action,
                            title,
                            row,
                            color,
                        }
                    })
                    .collect()
            }
            _ => Vec::new(),
        };

        let screen = Self {
            root,
            display_mode: parts.get("Display mode"),
            resolution: parts.get("Resolution"),
            vsync: parts.get("VSync"),
            vsync_label: parts.get("VSync label"),
            reset: parts.get("Reset controls"),
            back: parts.get("Back"),
            volumes,
            bindings,
            resolutions,
            modes,
            settings,
            waiting: None,
        };
        screen.fill_dropdowns(tick);
        screen.show_everything(tick);
        tick.ui.focus(screen.display_mode.or(screen.back));
        Ok(screen)
    }

    /// The screen's root entity: everything it spawned is under it.
    pub fn root(&self) -> Entity {
        self.root
    }

    /// Whether a row is waiting for the control to bind.
    pub fn is_rebinding(&self) -> bool {
        self.waiting.is_some()
    }

    /// One frame: what the player changed, carried out. Answers
    /// [`Outcome::Closed`] once Back is pressed or the cancel action backs
    /// out — the screen saved and taken away by then.
    pub fn update(&mut self, tick: &mut Tick<'_>) -> Outcome {
        if !tick.world.is_alive(self.root) {
            log::warn!("the options screen was despawned under it; closing");
            return Outcome::Closed;
        }
        if let Some(row) = self.waiting {
            self.take_binding(tick, row);
            return Outcome::Open;
        }
        // A cancel the controls spent closing a dropdown's list, or one
        // pressed while a list is open, is not the screen's.
        let cancel = tick.input.action(CANCEL).pressed()
            && !tick.ui.cancelled()
            && tick.ui.open_list().is_none();
        if clicked(tick, self.back) || cancel {
            self.close(tick);
            return Outcome::Closed;
        }
        if let Some(index) = picked(tick, self.display_mode) {
            if let Some(&mode) = self.modes.get(index) {
                self.settings.fullscreen = mode;
                tick.window.set_fullscreen(mode);
            }
        }
        if let Some(index) = picked(tick, self.resolution) {
            self.pick_resolution(tick, index);
        }
        if let Some(toggle) = self.vsync.filter(|&e| tick.ui.changed(e)) {
            if tick.window.abilities().vsync {
                self.settings.vsync = tick.world.get::<UiToggle>(toggle).is_some_and(|t| t.on);
                tick.window.set_vsync(self.settings.vsync);
            }
        }
        for volume in &self.volumes {
            if !tick.ui.changed(volume.row.control) {
                continue;
            }
            let Some(value) = tick
                .world
                .get::<UiSlider>(volume.row.control)
                .map(|s| s.value)
            else {
                continue;
            };
            self.settings.volumes.insert(volume.bus.clone(), value);
            let bus = tick.audio.bus(&volume.bus);
            tick.audio.set_bus_volume(bus, value, Duration::ZERO);
        }
        if let Some(row) = self
            .bindings
            .iter()
            .position(|binding| tick.ui.clicked(binding.row.control))
        {
            self.waiting = Some(row);
        }
        if clicked(tick, self.reset) {
            tick.reset_controls();
        }
        self.show_everything(tick);
        Outcome::Open
    }

    /// Keeps the settings and the controls, and takes the screen away. A
    /// save that fails is logged: the changes still hold for this run.
    pub fn close(&mut self, tick: &mut Tick<'_>) {
        if let Err(e) = self.settings.save(tick.saves) {
            log::error!("could not keep the options: {e}");
        }
        if let Err(e) = tick.save_controls() {
            log::error!("could not keep the controls: {e}");
        }
        if tick.world.is_alive(self.root) {
            hierarchy::despawn_recursive(tick.world, self.root);
        }
        tick.ui.focus(None);
        self.waiting = None;
    }

    /// The row waiting for a control: the cancel action backs out, anything
    /// else pressed becomes the action's binding of its kind.
    fn take_binding(&mut self, tick: &mut Tick<'_>, index: usize) {
        let Some(row) = self.bindings.get(index) else {
            self.waiting = None;
            return;
        };
        let control = row.row.control;
        if tick.input.action(CANCEL).pressed() {
            self.waiting = None;
        } else if let Some(binding) = tick.input.pressed_binding() {
            tick.actions.assign(&row.action, binding);
            log::info!("{} is now {binding}", row.action);
            self.waiting = None;
        } else {
            return;
        }
        // The press may have moved the focus on; it goes back to the row.
        tick.ui.focus(Some(control));
        self.show_everything(tick);
    }

    /// The size picked on the resolution row, else the windowed size.
    fn chosen_size(&self, tick: &Tick<'_>) -> (u32, u32) {
        self.settings
            .windowed_size
            .unwrap_or_else(|| tick.window.windowed_size())
    }

    fn pick_resolution(&mut self, tick: &mut Tick<'_>, index: usize) {
        let Some(&size) = self.resolutions.get(index) else {
            return;
        };
        self.settings.windowed_size = Some(size);
        tick.window.set_size(size.0, size.1);
        if let Some(mode) = display::exclusive_at(tick.window, self.settings.fullscreen, size) {
            self.settings.fullscreen = mode;
            tick.window.set_fullscreen(mode);
        }
        // The exclusive mode offered is the one at the new size.
        self.modes = display::modes(tick.window, size);
        self.fill_dropdowns(tick);
    }

    /// The dropdowns' lists: the modes and the sizes, as words. Where the
    /// size is not the game's (the web) the resolution row says so and
    /// takes no clicks.
    fn fill_dropdowns(&self, tick: &mut Tick<'_>) {
        let world = &mut *tick.world;
        if let Some(dropdown) = self
            .display_mode
            .and_then(|e| world.get_mut::<UiDropdown>(e))
        {
            dropdown.options = self.modes.iter().map(|&m| labels::mode(m)).collect();
        }
        let Some(row) = self.resolution else {
            return;
        };
        let fixed = self.resolutions.is_empty();
        if let Some(dropdown) = world.get_mut::<UiDropdown>(row) {
            dropdown.options = if fixed {
                vec![labels::SIZE_SET_BY_PAGE.to_owned()]
            } else {
                self.resolutions.iter().map(|&s| labels::size(s)).collect()
            };
        }
        if let Some(button) = world.get_mut::<UiButton>(row) {
            button.disabled = fixed;
        }
    }
}

/// The option the dropdown `control` was set to this frame, if it was.
fn picked(tick: &Tick<'_>, control: Option<Entity>) -> Option<usize> {
    let control = control.filter(|&e| tick.ui.changed(e))?;
    tick.world.get::<UiDropdown>(control).map(|d| d.selected)
}

/// Whether `control`, if the screen has it, was clicked this frame.
fn clicked(tick: &Tick<'_>, control: Option<Entity>) -> bool {
    control.is_some_and(|e| tick.ui.clicked(e))
}

#[cfg(test)]
mod tests;

/// The screen's state the tests read.
#[cfg(test)]
impl OptionsScreen {
    fn settings(&self) -> &Settings {
        &self.settings
    }

    fn mode(&self) -> crate::window::Fullscreen {
        self.settings.fullscreen
    }
}
