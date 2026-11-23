//! The game's controls, fed the frame's pointer, navigation and typing —
//! and the window told to take IME input while a text field has the focus.

use voltra_render::glam::Vec2;
use voltra_scene::ui::{Navigation, Pointer, UiInput};
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
    /// Updates hover, press, click, focus and typing before the tick reads
    /// them.
    ///
    /// In a window with no editor around it only: the editor's viewport is a
    /// panel, and its HUD is something being authored, not played — clicking
    /// it selects, it does not press. With no window yet there is no screen
    /// to lay the controls out on, unless a headless run gave one.
    pub(super) fn update_ui_controls(&mut self) {
        if self.ui.is_some() {
            return;
        }
        let (screen, scale) = match (self.window.as_ref(), self.headless_screen) {
            (Some(window), _) => {
                let scale = window.scale_factor() as f32;
                let scale = if scale.is_finite() && scale > 0.0 {
                    scale
                } else {
                    1.0
                };
                let size = window.inner_size();
                (
                    Vec2::new(size.width as f32, size.height as f32) / scale,
                    scale,
                )
            }
            (None, Some(screen)) => (screen, 1.0),
            (None, None) => return,
        };
        let layout = UiLayout::from_world(&self.world, screen);
        let input = UiInput {
            pointer: pointer_of(&self.input, scale),
            navigation: navigation_of(&self.input, &self.actions),
            text: self.input.typing().text().clone(),
        };
        self.ui_controls
            .update(&mut self.world, &layout, self.fonts.as_ref(), &input);

        // The IME only while typing: a game's keys are its own otherwise,
        // and on some platforms an IME swallows them.
        let editing = self.ui_controls.editing(&self.world);
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if editing.is_some() != self.ime_allowed {
            self.ime_allowed = editing.is_some();
            window.set_ime_allowed(self.ime_allowed);
        }
        if let Some(rect) = editing.and_then(|field| layout.rect(field)) {
            // Where the IME puts its candidate window: under the field.
            let at = rect.min * scale;
            let size = rect.size() * scale;
            window.set_ime_cursor_area(
                winit::dpi::PhysicalPosition::new(at.x as f64, at.y as f64),
                winit::dpi::PhysicalSize::new(size.x as f64, size.y as f64),
            );
        }
    }
}

/// The mouse in points, with this frame's left-button edges, the wheel and
/// whether Shift is held.
pub(super) fn pointer_of(input: &Input, scale: f32) -> Pointer {
    let position = input.cursor_position() / scale;
    Pointer {
        position: position.is_finite().then_some(position),
        went_down: input.was_mouse_button_pressed(MouseButton::Left),
        went_up: input.was_mouse_button_released(MouseButton::Left),
        scroll: Vec2::new(0.0, input.scroll_delta()),
        extend: input.typing().shift(),
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
