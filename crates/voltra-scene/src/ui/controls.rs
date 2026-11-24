//! The state of every control — which is hovered, held and focused — and
//! what happened to each this frame.

mod drag;
mod focus;
mod list;
mod navigate;
mod scrolling;
mod show;
mod tips;

use std::collections::HashSet;
use std::sync::Arc;

use voltra_assets::Fonts;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::button::UiButton;
use super::layout::UiLayout;
use super::scroll::UiScroll;
use super::text_field::{view, TextInput, UiTextField};
use super::theme::{self, UiTheme};
use super::toggle::UiToggle;
use super::{tabs, UiDropdown, UiTab};
use crate::text::Text;

pub use focus::is_control;

/// The pointer this frame, in points from the top-left of the screen.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pointer {
    /// `None` when the cursor is outside the window or unknown.
    pub position: Option<Vec2>,
    /// Whether the primary button went down this frame.
    pub went_down: bool,
    /// Whether the primary button came up this frame.
    pub went_up: bool,
    /// Wheel lines this frame: positive is up, and right.
    pub scroll: Vec2,
    /// Shift held: a click in a text field extends its selection.
    pub extend: bool,
}

/// Keyboard and gamepad navigation this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Navigation {
    /// The direction asked for, screen space (y down), or `None`.
    pub direction: Option<Vec2>,
    /// Press the focused control.
    pub accept: bool,
    /// Back out: closes an open dropdown list (`ui_cancel`).
    pub cancel: bool,
}

/// Everything the controls are told in one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiInput {
    /// The pointer's position and buttons.
    pub pointer: Pointer,
    /// Keyboard and gamepad navigation.
    pub navigation: Navigation,
    /// Typing, for the focused text field.
    pub text: TextInput,
    /// Seconds since the last update, which a tooltip's delay counts.
    pub delta: f32,
}

/// Which control is hovered, held and focused, and which were clicked,
/// changed or submitted this frame. One per game; the loop updates it
/// before the tick reads it.
///
/// A control is an element with a [`UiButton`] (a [`UiToggle`], a
/// [`UiTab`] and a [`UiDropdown`] are one too), a [`UiSlider`](super::UiSlider) or a
/// [`UiTextField`]. They share one focus, moved by the pointer and by the
/// navigation, as Godot's viewport keeps one focus owner for every
/// `Control`. It also holds the [`UiTheme`] the controls take.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiControls {
    hovered: Option<Entity>,
    pressed: Option<Entity>,
    focused: Option<Entity>,
    clicked: Vec<Entity>,
    changed: Vec<Entity>,
    submitted: Vec<Entity>,
    /// The focus the last update brought into view: a focus moved since,
    /// by the navigation or by [`focus`](Self::focus), is scrolled to.
    revealed: Option<Entity>,
    /// The dropdown whose list is open.
    open_list: Option<Entity>,
    /// Whether this frame's cancel closed a list.
    cancelled: bool,
    /// The element whose tooltip is counting, and for how long.
    tip: Option<(Entity, f32)>,
    theme: Option<Arc<UiTheme>>,
    /// The elements the theme has been written into.
    styled: HashSet<Entity>,
}

impl UiControls {
    /// Whether `control` was clicked this frame — by the pointer, or by
    /// accept on the focus. A toggle clicked has flipped.
    pub fn clicked(&self, control: Entity) -> bool {
        self.clicked.contains(&control)
    }

    /// Whether `control`'s value changed this frame: a slider moved, a
    /// toggle flipped, a text field edited, a dropdown's choice picked, a
    /// tabs container's page switched.
    pub fn changed(&self, control: Entity) -> bool {
        self.changed.contains(&control)
    }

    /// Whether Enter was pressed in the text field `control` this frame.
    pub fn submitted(&self, control: Entity) -> bool {
        self.submitted.contains(&control)
    }

    /// Whether the pointer is over `control`.
    pub fn hovered(&self, control: Entity) -> bool {
        self.hovered == Some(control)
    }

    /// The control holding the focus, if any.
    pub fn focused(&self) -> Option<Entity> {
        self.focused
    }

    /// The text field being typed in, if one has the focus: while there is
    /// one, the loop takes text and IME input, and a game may want its own
    /// keys left alone.
    pub fn editing(&self, world: &World) -> Option<Entity> {
        self.focused
            .filter(|&e| world.get::<UiTextField>(e).is_some())
    }

    /// The dropdown whose list is open, if one is: while it is, the
    /// navigation is the list's.
    pub fn open_list(&self) -> Option<Entity> {
        self.open_list
    }

    /// Whether this frame's cancel was spent closing a dropdown's list — a
    /// screen backing out on cancel should not, this frame.
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    /// Moves the focus to `control` — where a menu opening puts it, so the
    /// keyboard has somewhere to start.
    pub fn focus(&mut self, control: Option<Entity>) {
        self.focused = control;
    }

    /// Makes `theme` the look of every control that keeps none of its own,
    /// from the next update: every element is styled again. `None` stops
    /// theming new elements; those already styled keep the last look.
    pub fn set_theme(&mut self, theme: Option<UiTheme>) {
        self.theme = theme.map(Arc::new);
        self.styled.clear();
    }

    /// The theme controls take, if one is set.
    pub fn theme(&self) -> Option<&UiTheme> {
        self.theme.as_deref()
    }

    /// Updates every control for one frame: the theme, hover, press, drag,
    /// click, focus, the wheel, typing, an open list, tabs, tooltips — then
    /// what each shows.
    ///
    /// `fonts` measures text fields — where a click puts the caret, how far
    /// the text scrolls — and may be `None`, which puts the caret at the end.
    pub fn update(
        &mut self,
        world: &mut World,
        layout: &UiLayout,
        fonts: Option<&Fonts>,
        input: &UiInput,
    ) {
        self.clicked.clear();
        self.changed.clear();
        self.submitted.clear();
        self.cancelled = false;
        if let Some(theme) = self.theme.clone() {
            theme::apply(world, &theme, &mut self.styled);
        }
        let usable = |world: &World, e: Entity| focus::usable(world, layout, e);
        for slot in [
            &mut self.hovered,
            &mut self.pressed,
            &mut self.focused,
            &mut self.open_list,
        ] {
            if slot.is_some_and(|e| !world.is_alive(e) || !usable(world, e)) {
                *slot = None;
            }
        }
        // Taken before anything scrolls: the layout is of these offsets.
        let reaches = scrolling::reaches(world, layout);

        if !self.list_input(world, layout, input) {
            self.input(world, layout, fonts, input);
        }
        tabs::apply(world);
        if self.focused != self.revealed {
            if let Some(focused) = self.focused {
                scrolling::reveal(world, layout, focused);
            }
            self.revealed = self.focused;
        }
        for (entity, reach) in reaches {
            if let Some(scroll) = world.get_mut::<UiScroll>(entity) {
                scroll.clamp(reach);
            }
        }
        self.tips(world, layout, input);
        self.show(world, layout, fonts);
    }

    /// The frame's input when no list has it: hover, press, drag, click,
    /// the wheel, the navigation, typing — and what clicks do to toggles,
    /// tabs and dropdowns.
    fn input(
        &mut self,
        world: &mut World,
        layout: &UiLayout,
        fonts: Option<&Fonts>,
        input: &UiInput,
    ) {
        let usable = |world: &World, e: Entity| focus::usable(world, layout, e);
        let pointer = input.pointer;
        self.hovered = pointer
            .position
            .and_then(|at| super::element_at(world, layout, at))
            .and_then(|hit| focus::control_owning(world, hit))
            .filter(|&e| usable(world, e));
        if pointer.went_down {
            self.pressed = self.hovered;
            if self.hovered.is_some() {
                self.focused = self.hovered;
            } else if self.editing(world).is_some() {
                // A click off a text field ends typing in it.
                self.focused = None;
            }
        }
        if let (Some(held), Some(at)) = (self.pressed, pointer.position) {
            self.drag(world, layout, fonts, held, at, pointer);
        }
        if pointer.went_up {
            if let Some(control) = self.pressed.take() {
                if self.hovered == Some(control) && world.get::<UiButton>(control).is_some() {
                    self.clicked.push(control);
                }
            }
        }
        if pointer.scroll != Vec2::ZERO {
            if let Some(at) = pointer.position {
                scrolling::wheel(world, layout, at, pointer.scroll);
            }
        }

        self.navigate(world, layout, input.navigation);
        if let Some(field) = self.editing(world) {
            // Measured as the field's text is drawn, so the arrows step in
            // visual order through mixed-direction text.
            let style = world.get::<Text>(field).cloned();
            let measure = |content: &str| -> Option<Vec<f32>> {
                view::stops(style.as_ref()?, content, fonts?)
            };
            let edited = world
                .get_mut::<UiTextField>(field)
                .map(|f| f.take_measured(&input.text, &measure))
                .unwrap_or_default();
            self.mark(field, edited.changed, edited.submitted);
        }
        for control in self.clicked.clone() {
            if let Some(dropdown) = world.get_mut::<UiDropdown>(control) {
                dropdown.open_list();
                if dropdown.open {
                    self.open_list = Some(control);
                }
                continue;
            }
            if world.get::<UiTab>(control).is_some() {
                if let Some(container) = tabs::select(world, control) {
                    self.mark(container, true, false);
                }
                continue;
            }
            if let Some(toggle) = world.get_mut::<UiToggle>(control) {
                toggle.on = !toggle.on;
                self.changed.push(control);
            }
        }
    }

    fn mark(&mut self, control: Entity, changed: bool, submitted: bool) {
        if changed && !self.changed.contains(&control) {
            self.changed.push(control);
        }
        if submitted && !self.submitted.contains(&control) {
            self.submitted.push(control);
        }
    }
}

#[cfg(test)]
mod tests;
