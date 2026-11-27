//! What every control shows after a frame.

use voltra_assets::Fonts;
use voltra_ecs::{Entity, World};

use super::UiControls;
use crate::hierarchy;
use crate::sprite::Sprite2D;
use crate::text::Text;
use crate::ui::button::{State, UiButton};
use crate::ui::layout::UiLayout;
use crate::ui::slider::{UiSlider, UiSliderPart};
use crate::ui::text_field::view;
use crate::ui::toggle::UiToggle;
use crate::ui::{UiDropdown, UiRect};

impl UiControls {
    /// What every control shows after this frame: buttons' faces, sliders'
    /// parts, fields' text, dropdowns' choices.
    pub(super) fn show(&self, world: &mut World, layout: &UiLayout, fonts: Option<&Fonts>) {
        let buttons: Vec<(Entity, UiButton)> =
            world.query::<UiButton>().map(|(e, b)| (e, *b)).collect();
        for (entity, button) in buttons {
            let state = if self.pressed == Some(entity) {
                State::Pressed
            } else if self.hovered == Some(entity)
                || self.focused == Some(entity)
                || self.open_list == Some(entity)
            {
                State::Hovered
            } else {
                State::Normal
            };
            let toggle = world.get::<UiToggle>(entity).copied();
            if let Some(sprite) = world.get_mut::<Sprite2D>(entity) {
                button.show(state, toggle.as_ref(), sprite);
            }
        }

        let parts: Vec<(Entity, UiSliderPart)> = world
            .query::<UiSliderPart>()
            .map(|(e, p)| (e, *p))
            .collect();
        for (entity, part) in parts {
            let Some(slider) = hierarchy::parent_of(world, entity)
                .and_then(|parent| world.get::<UiSlider>(parent).copied())
            else {
                continue;
            };
            if let Some(rect) = world.get_mut::<UiRect>(entity) {
                slider.place(part, rect);
            }
        }

        let dropdowns: Vec<(Entity, String, bool)> = world
            .query::<UiDropdown>()
            .map(|(e, d)| (e, d.selected_text().to_owned(), self.open_list == Some(e)))
            .collect();
        for (entity, choice, open) in dropdowns {
            if let Some(dropdown) = world.get_mut::<UiDropdown>(entity) {
                dropdown.open = open;
            }
            if let Some(text) = world.get_mut::<Text>(entity) {
                if text.content != choice {
                    text.content = choice;
                }
            }
        }

        view::refresh(world, layout, fonts, self.editing(world));
    }
}
