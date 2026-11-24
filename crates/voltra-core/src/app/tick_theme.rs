//! A game's tick swapping the UI theme.

use voltra_scene::ui::theme::ThemeError;
use voltra_scene::UiTheme;

use super::Tick;

impl Tick<'_> {
    /// Reads the theme at `path` (relative to the asset root, a
    /// `.theme.ron`) and makes it the controls' look from the next frame:
    /// every control that keeps none of its own is restyled. A theme that
    /// does not read is reported and the current one kept.
    pub fn set_theme(&mut self, path: &str) -> Result<(), ThemeError> {
        let theme = UiTheme::load(&self.prefabs.root().join(path))?;
        self.ui.set_theme(Some(theme));
        Ok(())
    }
}
