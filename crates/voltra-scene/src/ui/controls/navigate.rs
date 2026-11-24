//! The keyboard and pad navigation: the focus moved, a slider nudged, a
//! button pressed by accept.

use voltra_ecs::World;
use voltra_render::glam::Vec2;

use super::{focus, Navigation, UiControls};
use crate::ui::button::UiButton;
use crate::ui::layout::UiLayout;
use crate::ui::slider::UiSlider;
use crate::ui::text_field::UiTextField;

impl UiControls {
    /// The navigation: a focused slider takes it along its track, a focused
    /// text field takes it sideways (its caret moves by the keys), and the
    /// rest moves the focus. Accept clicks a button and submits a field.
    pub(super) fn navigate(
        &mut self,
        world: &mut World,
        layout: &UiLayout,
        navigation: Navigation,
    ) {
        if let Some(direction) = navigation.direction {
            let mut moved = direction;
            if let Some(focused) = self.focused {
                if let Some(slider) = world.get_mut::<UiSlider>(focused) {
                    if let Some(steps) = slider.steps_for(direction) {
                        if slider.nudge(steps) {
                            self.changed.push(focused);
                        }
                        moved = Vec2::ZERO;
                    }
                } else if world.get::<UiTextField>(focused).is_some() {
                    moved.x = 0.0;
                }
            }
            if moved != Vec2::ZERO {
                self.focused = focus::next(world, layout, self.focused, moved);
            }
        }
        if navigation.accept {
            if let Some(focused) = self.focused {
                if world.get::<UiButton>(focused).is_some() {
                    if !self.clicked.contains(&focused) {
                        self.clicked.push(focused);
                    }
                } else if world.get::<UiTextField>(focused).is_some() {
                    self.mark(focused, false, true);
                }
            }
        }
    }
}
