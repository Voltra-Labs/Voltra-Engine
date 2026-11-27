//! A list the screen fills again whenever what it shows changes: a copy
//! of the prefab's template row per item, the template kept, hidden, for
//! the next time.

use voltra_ecs::{Entity, World};
use voltra_scene::text::markup;
use voltra_scene::{hierarchy, Name, Text, UiButton, UiHidden, UiRect};

use super::labels::Row;
use crate::app::Tick;

/// One list of the screen and the rows in it now.
#[derive(Debug, Default)]
pub(super) struct List {
    list: Option<Entity>,
    template: Option<Entity>,
    /// The template's text part for the name, by name.
    label: &'static str,
    /// The template's text part for the detail, by name. A prefab without
    /// one shows it after the name, in the label.
    detail: &'static str,
    /// What each row is named, `<prefix>: <n>`.
    prefix: &'static str,
    rows: Vec<Entity>,
    /// What the rows show, to fill again only on a change.
    shown: Vec<Row>,
}

impl List {
    /// The list `list` of copies of `template`; the template hidden.
    pub(super) fn new(
        tick: &mut Tick<'_>,
        list: Option<Entity>,
        template: Option<Entity>,
        (label, detail): (&'static str, &'static str),
        prefix: &'static str,
    ) -> Self {
        if let Some(template) = template {
            hidden(tick, template, true);
        }
        Self {
            list,
            template,
            label,
            detail,
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

    /// Shows `items`, copying the template again only if they changed.
    pub(super) fn show(&mut self, tick: &mut Tick<'_>, items: Vec<Row>) {
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
        for (slot, item) in items.iter().enumerate() {
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
                button.disabled = item.greyed;
            }
            let part = |world: &World, name: &str| {
                copy.entities
                    .iter()
                    .copied()
                    .find(|&e| world.get::<Name>(e).is_some_and(|n| n.as_str() == name))
            };
            let label = part(world, self.label);
            match part(world, self.detail) {
                Some(detail) => {
                    set_words(world, label, &item.name);
                    set_words(world, Some(detail), &item.detail);
                }
                None => {
                    let both = format!("{}  {}", item.name, item.detail);
                    set_words(world, label, &both);
                }
            }
            self.rows.push(row);
        }
        self.shown = items;
    }
}

/// Sets `entity`'s text to `words` as written: escaped where the text is
/// rich, so someone else's name is never read as tags.
pub(super) fn set_words(world: &mut World, entity: Option<Entity>, words: &str) {
    if let Some(text) = entity.and_then(|e| world.get_mut::<Text>(e)) {
        let shown = if text.rich {
            markup::escape(words)
        } else {
            words.into()
        };
        if text.content != shown {
            text.content = shown.into_owned();
        }
    }
}

/// Hides or shows `entity`, if it is there.
pub(super) fn hidden(tick: &mut Tick<'_>, entity: Entity, hide: bool) {
    if tick.world.is_alive(entity) {
        voltra_scene::ui::hidden::set_hidden(tick.world, entity, hide);
    }
}
