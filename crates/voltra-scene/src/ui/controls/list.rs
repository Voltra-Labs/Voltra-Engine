//! An open dropdown list, which has the input while it is open.

use voltra_ecs::World;

use super::{UiControls, UiInput};
use crate::ui::layout::UiLayout;
use crate::ui::UiDropdown;

impl UiControls {
    /// Gives the frame's input to the open list, if there is one; whether
    /// it took it. The pointer highlights an item and a click picks it, a
    /// click anywhere else closes the list unchanged — the dropdown's own
    /// box included, as a second click on a closed one opened it; up and
    /// down move the highlight, accept picks it, cancel closes the list.
    pub(super) fn list_input(
        &mut self,
        world: &mut World,
        layout: &UiLayout,
        input: &UiInput,
    ) -> bool {
        let Some(owner) = self.open_list else {
            return false;
        };
        let (Some(rect), Some(dropdown)) = (layout.rect(owner), world.get_mut::<UiDropdown>(owner))
        else {
            self.open_list = None;
            return false;
        };
        if !dropdown.open {
            // Closed from outside — a tick, or the options changed.
            self.open_list = None;
            return false;
        }
        self.hovered = Some(owner);
        self.pressed = None;
        let list = dropdown.list(rect, layout.screen());
        let pointer = input.pointer;
        let row = list.and_then(|list| pointer.position.and_then(|at| list.row_at(at)));
        if let Some(row) = row {
            dropdown.highlighted =
                (dropdown.first + row).min(dropdown.options.len().saturating_sub(1));
        }
        let mut close = false;
        let mut changed = false;
        if pointer.went_down {
            match row {
                Some(_) => changed = dropdown.pick(),
                None => dropdown.open = false,
            }
            close = true;
        }
        let navigation = input.navigation;
        if !close {
            if let Some(direction) = navigation.direction {
                if direction.y > 0.0 {
                    dropdown.step(1);
                } else if direction.y < 0.0 {
                    dropdown.step(-1);
                }
            }
            if navigation.accept {
                changed = dropdown.pick();
                close = true;
            } else if navigation.cancel {
                dropdown.open = false;
                close = true;
                self.cancelled = true;
            }
        }
        if close {
            self.open_list = None;
            self.focused = Some(owner);
        }
        if changed {
            self.mark(owner, true, false);
        }
        true
    }
}
