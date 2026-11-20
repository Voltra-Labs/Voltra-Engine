//! The game's buttons, fed the frame's pointer and navigation.

use voltra_render::glam::Vec2;
use voltra_scene::ui::{Navigation, Pointer};
use voltra_scene::UiLayout;
use winit::event::MouseButton;

use super::App;
use crate::input::{ActionMap, Input};

/// The actions that move the focus and press, in [`DEFAULT_INPUT_MAP`]'s
/// names. A map without them simply navigates nothing.
///
/// [`DEFAULT_INPUT_MAP`]: super::DEFAULT_INPUT_MAP
const UP: &str = "ui_up";
const DOWN: &str = "ui_down";
const LEFT: &str = "ui_left";
const RIGHT: &str = "ui_right";
const ACCEPT: &str = "ui_accept";

impl App {
    /// Updates hover, press, click and focus before the tick reads them.
    ///
    /// In a window with no editor around it only: the editor's viewport is a
    /// panel, and its HUD is something being authored, not played — clicking
    /// it selects, it does not press. With no window yet there is no screen
    /// to lay the buttons out on.
    pub(super) fn update_ui_buttons(&mut self) {
        if self.ui.is_some() {
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let scale = window.scale_factor() as f32;
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let size = window.inner_size();
        let screen = Vec2::new(size.width as f32, size.height as f32) / scale;
        let layout = UiLayout::from_world(&self.world, screen);
        let pointer = pointer_of(&self.input, scale);
        let navigation = navigation_of(&self.input, &self.actions);
        self.ui_buttons
            .update(&mut self.world, &layout, pointer, navigation);
    }
}

/// The mouse in points, with this frame's left-button edges.
pub(super) fn pointer_of(input: &Input, scale: f32) -> Pointer {
    let position = input.cursor_position() / scale;
    Pointer {
        position: position.is_finite().then_some(position),
        went_down: input.was_mouse_button_pressed(MouseButton::Left),
        went_up: input.was_mouse_button_released(MouseButton::Left),
    }
}

/// Which way the `ui_*` actions push the focus this frame, y down.
pub(super) fn navigation_of(input: &Input, actions: &ActionMap) -> Navigation {
    let pressed = |name: &str| actions.contains(name) && input.action(name).pressed();
    let mut direction = Vec2::ZERO;
    if pressed(UP) {
        direction.y -= 1.0;
    }
    if pressed(DOWN) {
        direction.y += 1.0;
    }
    if pressed(LEFT) {
        direction.x -= 1.0;
    }
    if pressed(RIGHT) {
        direction.x += 1.0;
    }
    Navigation {
        direction: (direction != Vec2::ZERO).then_some(direction),
        accept: pressed(ACCEPT),
    }
}

#[cfg(test)]
mod tests {
    use winit::keyboard::KeyCode;

    use super::*;
    use crate::input::Binding;

    fn map() -> ActionMap {
        let mut map = ActionMap::new();
        map.bind(DOWN, Binding::Key(KeyCode::ArrowDown));
        map.bind(ACCEPT, Binding::Key(KeyCode::Enter));
        map
    }

    #[test]
    fn a_ui_action_becomes_a_screen_direction() {
        let mut input = Input::new();
        input.on_key(KeyCode::ArrowDown, true, false);
        input.update_actions(&map());
        let nav = navigation_of(&input, &map());
        assert_eq!(nav.direction, Some(Vec2::Y), "down is +y on screen");
        assert!(!nav.accept);
    }

    #[test]
    fn accept_presses() {
        let mut input = Input::new();
        input.on_key(KeyCode::Enter, true, false);
        input.update_actions(&map());
        assert!(navigation_of(&input, &map()).accept);
    }

    #[test]
    fn a_map_without_ui_actions_navigates_nothing() {
        let input = Input::new();
        let nav = navigation_of(&input, &ActionMap::new());
        assert_eq!(nav, Navigation::default());
    }

    #[test]
    fn the_pointer_is_in_points() {
        let mut input = Input::new();
        input.on_cursor_moved(Vec2::new(300.0, 150.0));
        input.on_mouse_button(MouseButton::Left, true);
        let pointer = pointer_of(&input, 1.5);
        assert_eq!(pointer.position, Some(Vec2::new(200.0, 100.0)));
        assert!(pointer.went_down);
    }
}
