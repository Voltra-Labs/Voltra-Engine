//! A list the screen fills again whenever what it shows changes: a copy
//! of the prefab's template row per item, the template kept, hidden, for
//! the next time.

use voltra_ecs::Entity;
use voltra_scene::{hierarchy, Name, Text, UiButton, UiHidden, UiRect};

use crate::app::Tick;

/// One list of the screen and the rows in it now.
#[derive(Debug, Default)]
pub(super) struct List {
    list: Option<Entity>,
    template: Option<Entity>,
    /// The template's text part, by name.
    label: &'static str,
    /// What each row is named, `<prefix>: <n>`.
    prefix: &'static str,
    rows: Vec<Entity>,
    /// What the rows show, to fill again only on a change.
    shown: Vec<(String, bool)>,
}

impl List {
    /// The list `list` of copies of `template`; the template hidden.
    pub(super) fn new(
        tick: &mut Tick<'_>,
        list: Option<Entity>,
        template: Option<Entity>,
        label: &'static str,
        prefix: &'static str,
    ) -> Self {
        if let Some(template) = template {
            hidden(tick, template, true);
        }
        Self {
            list,
            template,
            label,
            prefix,
            rows: Vec::new(),
            shown: Vec::new(),
        }
    }

    /// The rows, in order.
    pub(super) fn rows(&self) -> &[Entity] {
        &self.rows
    }

    /// The row `clicked` this frame, by index.
    pub(super) fn clicked(&self, tick: &Tick<'_>) -> Option<usize> {
        self.rows.iter().position(|&row| tick.ui.clicked(row))
    }

    /// Shows `items` — each its words and whether it is greyed — copying
    /// the template again only if they changed.
    pub(super) fn show(&mut self, tick: &mut Tick<'_>, items: Vec<(String, bool)>) {
        if items == self.shown {
            return;
        }
        let (Some(list), Some(template)) = (self.list, self.template) else {
            self.shown = items;
            return;
        };
        for row in self.rows.drain(..) {
            if tick.world.is_alive(row) {
                hierarchy::despawn_recursive(tick.world, row);
            }
        }
        for (slot, (words, greyed)) in items.iter().enumerate() {
            let copy = match tick.duplicate(template) {
                Ok(copy) => copy,
                Err(e) => {
                    log::error!("could not copy the lobby's {} row: {e}", self.prefix);
                    continue;
                }
            };
            let Some(row) = copy.root() else {
                continue;
            };
            let world = &mut *tick.world;
            world.remove::<UiHidden>(row);
            if let Err(e) = hierarchy::set_parent(world, row, list) {
                log::error!("could not place the lobby's {} row: {e}", self.prefix);
                hierarchy::despawn_recursive(world, row);
                continue;
            }
            world.insert(row, Name::new(format!("{}: {slot}", self.prefix)));
            if let Some(rect) = world.get_mut::<UiRect>(row) {
                rect.slot = i32::try_from(slot).unwrap_or(i32::MAX);
            }
            if let Some(button) = world.get_mut::<UiButton>(row) {
                button.disabled = *greyed;
            }
            let label = copy.entities.iter().copied().find(|&e| {
                world
                    .get::<Name>(e)
                    .is_some_and(|n| n.as_str() == self.label)
            });
            if let Some(text) = label.and_then(|e| world.get_mut::<Text>(e)) {
                text.content.clone_from(words);
            }
            self.rows.push(row);
        }
        self.shown = items;
    }
}

/// Hides or shows `entity`, if it is there.
pub(super) fn hidden(tick: &mut Tick<'_>, entity: Entity, hide: bool) {
    if tick.world.is_alive(entity) {
        voltra_scene::ui::hidden::set_hidden(tick.world, entity, hide);
    }
}
