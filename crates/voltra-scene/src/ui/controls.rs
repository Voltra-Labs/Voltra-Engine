//! The state of every control — which is hovered, held and focused — and
//! what happened to each this frame.

mod focus;
mod scrolling;

use voltra_assets::Fonts;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::button::{State, UiButton};
use super::layout::UiLayout;
use super::scroll::UiScroll;
use super::slider::{UiSlider, UiSliderPart};
use super::text_field::{view, TextInput, UiTextField};
use super::toggle::UiToggle;
use super::UiRect;
use crate::hierarchy;
use crate::sprite::Sprite;

pub use focus::is_control;

/// The pointer this frame, in points from the top-left of the screen.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pointer {
    /// `None` when the cursor is outside the window or unknown.
    pub position: Option<Vec2>,
    pub went_down: bool,
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
}

/// Everything the controls are told in one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiInput {
    pub pointer: Pointer,
    pub navigation: Navigation,
    /// Typing, for the focused text field.
    pub text: TextInput,
}

/// Which control is hovered, held and focused, and which were clicked,
/// changed or submitted this frame. One per game; the loop updates it
/// before the tick reads it.
///
/// A control is an element with a [`UiButton`] (a [`UiToggle`] is one
/// too), a [`UiSlider`] or a [`UiTextField`]. They share one focus, moved
/// by the pointer and by the navigation, as Godot's viewport keeps one
/// focus owner for every `Control`.
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
}

impl UiControls {
    /// Whether `control` was clicked this frame — by the pointer, or by
    /// accept on the focus. A toggle clicked has flipped.
    pub fn clicked(&self, control: Entity) -> bool {
        self.clicked.contains(&control)
    }

    /// Whether `control`'s value changed this frame: a slider moved, a
    /// toggle flipped, a text field edited.
    pub fn changed(&self, control: Entity) -> bool {
        self.changed.contains(&control)
    }

    /// Whether Enter was pressed in the text field `control` this frame.
    pub fn submitted(&self, control: Entity) -> bool {
        self.submitted.contains(&control)
    }

    pub fn hovered(&self, control: Entity) -> bool {
        self.hovered == Some(control)
    }

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

    /// Moves the focus to `control` — where a menu opening puts it, so the
    /// keyboard has somewhere to start.
    pub fn focus(&mut self, control: Option<Entity>) {
        self.focused = control;
    }

    /// Updates every control for one frame: hover, press, drag, click,
    /// focus, the wheel, typing — then what each shows.
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
        let usable = |world: &World, e: Entity| focus::usable(world, layout, e);
        for slot in [&mut self.hovered, &mut self.pressed, &mut self.focused] {
            if slot.is_some_and(|e| !world.is_alive(e) || !usable(world, e)) {
                *slot = None;
            }
        }
        let pointer = input.pointer;
        // Taken before anything scrolls: the layout is of these offsets.
        let reaches = scrolling::reaches(world, layout);

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
            let edited = world
                .get_mut::<UiTextField>(field)
                .map(|f| f.take(&input.text))
                .unwrap_or_default();
            self.mark(field, edited.changed, edited.submitted);
        }
        for control in self.clicked.clone() {
            if let Some(toggle) = world.get_mut::<UiToggle>(control) {
                toggle.on = !toggle.on;
                self.changed.push(control);
            }
        }
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
        self.show(world, layout, fonts);
    }

    /// The held control following the pointer: a slider's value, a text
    /// field's caret — and its selection after the press.
    fn drag(
        &mut self,
        world: &mut World,
        layout: &UiLayout,
        fonts: Option<&Fonts>,
        held: Entity,
        at: Vec2,
        pointer: Pointer,
    ) {
        if let Some(track) = layout.rect(held) {
            if let Some(slider) = world.get_mut::<UiSlider>(held) {
                if slider.set_at(track, at) {
                    self.changed.push(held);
                }
                return;
            }
        }
        if let Some(index) = view::index_at(world, held, layout, fonts, at) {
            if let Some(field) = world.get_mut::<UiTextField>(held) {
                field.place_caret(index, pointer.extend || !pointer.went_down);
            }
        }
    }

    /// The navigation: a focused slider takes it along its track, a focused
    /// text field takes it sideways (its caret moves by the keys), and the
    /// rest moves the focus. Accept clicks a button and submits a field.
    fn navigate(&mut self, world: &mut World, layout: &UiLayout, navigation: Navigation) {
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

    fn mark(&mut self, control: Entity, changed: bool, submitted: bool) {
        if changed && !self.changed.contains(&control) {
            self.changed.push(control);
        }
        if submitted && !self.submitted.contains(&control) {
            self.submitted.push(control);
        }
    }

    /// What every control shows after this frame: buttons' faces, sliders'
    /// parts, fields' text.
    fn show(&self, world: &mut World, layout: &UiLayout, fonts: Option<&Fonts>) {
        let buttons: Vec<(Entity, UiButton)> =
            world.query::<UiButton>().map(|(e, b)| (e, *b)).collect();
        for (entity, button) in buttons {
            let state = if self.pressed == Some(entity) {
                State::Pressed
            } else if self.hovered == Some(entity) || self.focused == Some(entity) {
                State::Hovered
            } else {
                State::Normal
            };
            let toggle = world.get::<UiToggle>(entity).copied();
            if let Some(sprite) = world.get_mut::<Sprite>(entity) {
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

        view::refresh(world, layout, fonts, self.editing(world));
    }
}

#[cfg(test)]
mod tests;
