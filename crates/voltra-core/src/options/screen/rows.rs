//! The screen's lists stamped out from the prefab's template rows: one
//! slider per bus, one row per action.

use voltra_ecs::{Entity, World};
use voltra_scene::{hierarchy, Name, Text, UiRect, UiStack};

use crate::app::Tick;

/// One copy of the template row.
#[derive(Debug)]
pub(super) struct Row {
    /// The copy's root: the slider, or the button.
    pub(super) control: Entity,
    /// Its text, if the template had one.
    pub(super) label: Option<Entity>,
}

/// One copy of `template` per item of `names`, under `list`, in that order,
/// each named `<prefix>: <item>`; then the template goes, and the list is
/// sized to hold them all. `label` names the template's text part. Each row
/// comes with its item: one that would not copy is left out, and said.
pub(super) fn stamp(
    tick: &mut Tick<'_>,
    list: Entity,
    template: Entity,
    label: &str,
    prefix: &str,
    names: &[String],
) -> Vec<(String, Row)> {
    let mut rows = Vec::with_capacity(names.len());
    for (slot, item) in names.iter().enumerate() {
        let copy = match tick.duplicate(template) {
            Ok(copy) => copy,
            Err(e) => {
                log::error!("could not copy the options row for {item:?}: {e}");
                continue;
            }
        };
        let Some(control) = copy.root() else {
            continue;
        };
        let world = &mut *tick.world;
        if let Err(e) = hierarchy::set_parent(world, control, list) {
            log::error!("could not place the options row for {item:?}: {e}");
            hierarchy::despawn_recursive(world, control);
            continue;
        }
        world.insert(control, Name::new(format!("{prefix}: {item}")));
        if let Some(rect) = world.get_mut::<UiRect>(control) {
            rect.slot = i32::try_from(slot).unwrap_or(i32::MAX);
        }
        let label = copy.entities.iter().copied().find(|&e| {
            world.get::<Name>(e).is_some_and(|n| n.as_str() == label)
                && world.get::<Text>(e).is_some()
        });
        rows.push((item.clone(), Row { control, label }));
    }
    hierarchy::despawn_recursive(tick.world, template);
    fit(
        tick.world,
        list,
        template_height(&rows, tick.world),
        rows.len(),
    );
    rows
}

fn template_height(rows: &[(String, Row)], world: &World) -> f32 {
    rows.first()
        .and_then(|(_, row)| world.get::<UiRect>(row.control))
        .map_or(0.0, |rect| rect.offset_max.y - rect.offset_min.y)
}

/// Makes a fixed-height list tall enough for `count` rows of `height` — or,
/// if it scrolls, leaves it as authored: scrolling is what it is for.
fn fit(world: &mut World, list: Entity, height: f32, count: usize) {
    if world.get::<voltra_scene::UiScroll>(list).is_some() || count == 0 {
        return;
    }
    let (spacing, padding) = world.get::<UiStack>(list).map_or((0.0, 0.0), |stack| {
        (stack.spacing, stack.padding.top + stack.padding.bottom)
    });
    let needed = count as f32 * height + (count - 1) as f32 * spacing + padding;
    if let Some(rect) = world.get_mut::<UiRect>(list) {
        rect.offset_max.y = rect.offset_min.y + needed;
    }
}

/// Sets the text of `label`, if there is one.
pub(super) fn say(world: &mut World, label: Option<Entity>, words: &str) {
    if let Some(text) = label.and_then(|e| world.get_mut::<Text>(e)) {
        if text.content != words {
            text.content = words.to_owned();
        }
    }
}
