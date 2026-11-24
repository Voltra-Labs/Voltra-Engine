//! Which tooltip shows: the hovered element's after its delay, else the
//! focused control's.

use voltra_ecs::{Entity, World};

use super::{UiControls, UiInput};
use crate::hierarchy;
use crate::ui::layout::UiLayout;
use crate::ui::UiTooltip;

impl UiControls {
    /// Counts the tooltip under the pointer — or, with the pointer on none,
    /// the focused control's — and shows it once its delay has passed;
    /// hides every other. None shows while a list is open.
    pub(super) fn tips(&mut self, world: &mut World, layout: &UiLayout, input: &UiInput) {
        let owner = |world: &World, from: Entity| {
            std::iter::once(from)
                .chain(hierarchy::ancestors(world, from))
                .find(|&e| world.get::<UiTooltip>(e).is_some())
        };
        let target = if self.open_list.is_some() {
            None
        } else {
            input
                .pointer
                .position
                .and_then(|at| crate::ui::element_at(world, layout, at))
                .and_then(|hit| owner(world, hit))
                .or_else(|| self.focused.and_then(|f| owner(world, f)))
        };
        let delta = if input.delta.is_finite() {
            input.delta.max(0.0)
        } else {
            0.0
        };
        self.tip = match (self.tip, target) {
            (Some((was, time)), Some(now)) if was == now => Some((now, time + delta)),
            (_, Some(now)) => Some((now, 0.0)),
            (_, None) => None,
        };
        let shown = self.tip.and_then(|(entity, time)| {
            let delay = world.get::<UiTooltip>(entity)?.delay;
            (time >= delay).then_some(entity)
        });
        for (entity, tooltip) in world.query_mut::<UiTooltip>() {
            let show = shown == Some(entity);
            if tooltip.shown != show {
                tooltip.shown = show;
            }
        }
    }
}
