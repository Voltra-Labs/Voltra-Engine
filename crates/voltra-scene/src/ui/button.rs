//! Buttons: screen-space elements that take a click, a hover and the focus.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::layout::UiLayout;
use super::rect::UiRect;
use crate::hierarchy;
use crate::sprite::Sprite;

/// Makes a [`UiRect`] element clickable and focusable, and tints its
/// `Sprite` by state.
///
/// Godot's `Button`, minus the text it carries: the label is a `Text`
/// child, and a click on the label reaches the button, as Godot's default
/// `mouse_filter` passes it. Colours rather than a style resource: a sprite
/// tint is what this engine's quads already have.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct UiButton {
    pub normal: [f32; 4],
    /// Under the pointer, or holding the focus.
    pub hovered: [f32; 4],
    /// Held down.
    pub pressed: [f32; 4],
    pub disabled_color: [f32; 4],
    /// Takes no click and no focus.
    #[serde(default)]
    pub disabled: bool,
}

impl Default for UiButton {
    fn default() -> Self {
        Self {
            normal: [0.2, 0.22, 0.28, 1.0],
            hovered: [0.3, 0.34, 0.44, 1.0],
            pressed: [0.14, 0.15, 0.2, 1.0],
            disabled_color: [0.2, 0.2, 0.2, 0.5],
            disabled: false,
        }
    }
}

/// The pointer this frame, in points from the top-left of the screen.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pointer {
    /// `None` when the cursor is outside the window or unknown.
    pub position: Option<Vec2>,
    pub went_down: bool,
    pub went_up: bool,
}

/// Keyboard and gamepad navigation this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Navigation {
    /// The direction asked for, screen space (y down), or `None`.
    pub direction: Option<Vec2>,
    /// Press the focused button.
    pub accept: bool,
}

/// Which button is hovered, held and focused, and which were clicked this
/// frame. One per game; the loop updates it before the tick reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UiButtons {
    hovered: Option<Entity>,
    pressed: Option<Entity>,
    focused: Option<Entity>,
    clicked: Vec<Entity>,
}

impl UiButtons {
    /// Whether `button` was clicked this frame — by the pointer, or by
    /// accept on the focus.
    pub fn clicked(&self, button: Entity) -> bool {
        self.clicked.contains(&button)
    }

    pub fn hovered(&self, button: Entity) -> bool {
        self.hovered == Some(button)
    }

    pub fn focused(&self) -> Option<Entity> {
        self.focused
    }

    /// Moves the focus to `button` — where a menu opening puts it, so the
    /// keyboard has somewhere to start.
    pub fn focus(&mut self, button: Option<Entity>) {
        self.focused = button;
    }

    /// Updates every button for one frame: hover, press, click, focus, and
    /// the tint each one's sprite shows.
    pub fn update(
        &mut self,
        world: &mut World,
        layout: &UiLayout,
        pointer: Pointer,
        navigation: Navigation,
    ) {
        self.clicked.clear();
        let usable = |world: &World, e: Entity| {
            world.get::<UiButton>(e).is_some_and(|b| !b.disabled) && layout.rect(e).is_some()
        };
        for slot in [&mut self.hovered, &mut self.pressed, &mut self.focused] {
            if slot.is_some_and(|e| !world.is_alive(e) || !usable(world, e)) {
                *slot = None;
            }
        }

        self.hovered = pointer
            .position
            .and_then(|at| super::element_at(world, layout, at))
            .and_then(|hit| button_owning(world, hit))
            .filter(|&e| usable(world, e));
        if pointer.went_down {
            self.pressed = self.hovered;
            if self.hovered.is_some() {
                self.focused = self.hovered;
            }
        }
        if pointer.went_up {
            if let Some(button) = self.pressed.take() {
                if self.hovered == Some(button) {
                    self.clicked.push(button);
                }
            }
        }

        if let Some(direction) = navigation.direction {
            self.focused = next_focus(world, layout, self.focused, direction, &usable);
        }
        if navigation.accept {
            if let Some(button) = self.focused {
                if !self.clicked.contains(&button) {
                    self.clicked.push(button);
                }
            }
        }

        self.tint(world);
    }

    fn tint(&self, world: &mut World) {
        let buttons: Vec<(Entity, UiButton)> =
            world.query::<UiButton>().map(|(e, b)| (e, *b)).collect();
        for (entity, button) in buttons {
            let Some(sprite) = world.get_mut::<Sprite>(entity) else {
                continue;
            };
            sprite.color = if button.disabled {
                button.disabled_color
            } else if self.pressed == Some(entity) {
                button.pressed
            } else if self.hovered == Some(entity) || self.focused == Some(entity) {
                button.hovered
            } else {
                button.normal
            };
        }
    }
}

/// The button `hit` is, or the nearest one above it — a click on a label
/// is a click on the button it labels.
fn button_owning(world: &World, hit: Entity) -> Option<Entity> {
    std::iter::once(hit)
        .chain(hierarchy::ancestors(world, hit))
        .find(|&e| world.get::<UiButton>(e).is_some())
}

/// The usable button nearest `from` in `direction`, by Godot's rule of
/// thumb: ahead of it, scored by distance with sideways drift weighing
/// double. With no focus yet, the top-left-most button.
fn next_focus(
    world: &World,
    layout: &UiLayout,
    from: Option<Entity>,
    direction: Vec2,
    usable: &impl Fn(&World, Entity) -> bool,
) -> Option<Entity> {
    let buttons: Vec<(Entity, Vec2)> = world
        .query::<UiButton>()
        .filter(|(e, _)| usable(world, *e))
        .filter_map(|(e, _)| layout.rect(e).map(|r| (e, r.center())))
        .collect();
    let Some(from) = from.and_then(|f| buttons.iter().find(|(e, _)| *e == f).copied()) else {
        return buttons
            .iter()
            .min_by(|a, b| {
                (a.1.y, a.1.x)
                    .partial_cmp(&(b.1.y, b.1.x))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(e, _)| *e);
    };
    let direction = direction.normalize_or_zero();
    if direction == Vec2::ZERO {
        return Some(from.0);
    }
    buttons
        .iter()
        .filter(|(e, _)| *e != from.0)
        .filter_map(|&(e, center)| {
            let offset = center - from.1;
            let ahead = offset.dot(direction);
            if ahead <= 0.0 {
                return None;
            }
            let sideways = (offset - direction * ahead).length();
            Some((e, ahead + 2.0 * sideways))
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(e, _)| e)
        .or(Some(from.0))
}

/// Whether `entity` can be a button at all: it needs a rect to be clicked.
pub fn is_button(world: &World, entity: Entity) -> bool {
    world.get::<UiButton>(entity).is_some() && world.get::<UiRect>(entity).is_some()
}

#[cfg(test)]
mod tests;
