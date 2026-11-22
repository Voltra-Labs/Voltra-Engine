//! What the editor has selected: a set of entities in the order they were
//! picked, the last of them the primary.
//!
//! Unity's `Selection.objects` with its `activeObject`, Godot's selection
//! with its last-selected node. The primary is what the inspector shows and
//! what anything that acts on one entity — painting, editing a shape,
//! making a prefab — acts on; the whole set is what the gizmo moves, Delete
//! removes and the clipboard copies. Ordered rather than hashed so the
//! primary is well defined and an undo puts the same one back.

use voltra_ecs::{Entity, World};
use voltra_scene::format::entity_with_id;
use voltra_scene::SceneId;

/// What a click on an entity does to the selection, by the keys held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Click {
    /// No key: just that entity.
    Replace,
    /// Ctrl (Cmd on a Mac): in if it was out, out if it was in.
    Toggle,
    /// Shift: everything from the primary to it, in the order the rows are
    /// shown — or, where there is no order (the viewport), add it.
    Extend,
}

impl Click {
    /// The click the held `modifiers` make. Ctrl wins over Shift, as in a
    /// file manager.
    pub(crate) fn from_modifiers(modifiers: voltra_core::egui::Modifiers) -> Self {
        if modifiers.command {
            Self::Toggle
        } else if modifiers.shift {
            Self::Extend
        } else {
            Self::Replace
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    /// Each entity once, in the order picked; the primary last.
    entities: Vec<Entity>,
}

impl Selection {
    /// Just `entity`.
    #[cfg(test)]
    pub(crate) fn single(entity: Entity) -> Self {
        Self {
            entities: vec![entity],
        }
    }

    /// The entity the inspector shows: the one picked last.
    pub(crate) fn primary(&self) -> Option<Entity> {
        self.entities.last().copied()
    }

    /// [`primary`](Self::primary), if `world` still holds it.
    pub(crate) fn live_primary(&self, world: &World) -> Option<Entity> {
        self.primary().filter(|&entity| world.is_alive(entity))
    }

    pub(crate) fn contains(&self, entity: Entity) -> bool {
        self.entities.contains(&entity)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.entities.len()
    }

    /// Every selected entity, the primary last.
    pub(crate) fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter().copied()
    }

    /// Selects `entity` alone, or nothing.
    pub(crate) fn set(&mut self, entity: Option<Entity>) {
        self.entities.clear();
        self.entities.extend(entity);
    }

    pub(crate) fn clear(&mut self) {
        self.entities.clear();
    }

    /// Adds `entity` as the primary, moving it to the end if it was already
    /// selected.
    pub(crate) fn add(&mut self, entity: Entity) {
        self.entities.retain(|&e| e != entity);
        self.entities.push(entity);
    }

    /// Removes `entity` if it was selected, adds it as the primary if not —
    /// a Ctrl-click.
    pub(crate) fn toggle(&mut self, entity: Entity) {
        if self.contains(entity) {
            self.entities.retain(|&e| e != entity);
        } else {
            self.entities.push(entity);
        }
    }

    /// Applies a `click` on `entity`. `order` is the order the entities are
    /// shown in, for a Shift-click's range; empty, a Shift-click adds. The
    /// clicked entity ends up the primary unless the click took it out.
    pub(crate) fn click(&mut self, entity: Entity, click: Click, order: &[Entity]) {
        match click {
            Click::Replace => self.set(Some(entity)),
            Click::Toggle => self.toggle(entity),
            Click::Extend => {
                let from = self
                    .primary()
                    .and_then(|p| order.iter().position(|&e| e == p));
                let to = order.iter().position(|&e| e == entity);
                let (Some(from), Some(to)) = (from, to) else {
                    self.add(entity);
                    return;
                };
                // The range replaces what was selected, a file manager's
                // Shift-click, walked toward the clicked row so it ends last.
                self.clear();
                if from <= to {
                    order[from..=to].iter().for_each(|&e| self.add(e));
                } else {
                    order[to..=from].iter().rev().for_each(|&e| self.add(e));
                }
            }
        }
    }

    /// Takes in `entities` — a box drawn over the viewport — on top of what
    /// is selected when `extend` (Shift or Ctrl held), instead of it
    /// otherwise. The last of them becomes the primary.
    pub(crate) fn take(&mut self, entities: &[Entity], extend: bool) {
        if !extend {
            self.clear();
        }
        entities.iter().for_each(|&e| self.add(e));
    }

    /// Forgets what `world` no longer holds.
    pub(crate) fn retain_alive(&mut self, world: &World) {
        self.entities.retain(|&entity| world.is_alive(entity));
    }

    /// The selection by identity, the primary last: what an edit or a play
    /// snapshot keeps, since an `Entity` will not survive the respawn that
    /// puts it back. An entity without an id is left out.
    pub(crate) fn ids(&self, world: &World) -> Vec<SceneId> {
        self.entities
            .iter()
            .filter_map(|&entity| world.get::<SceneId>(entity).copied())
            .collect()
    }

    /// The entities `world` holds under `ids`, in their order; an id nothing
    /// carries any more is left out.
    pub(crate) fn from_ids(world: &World, ids: &[SceneId]) -> Self {
        let mut selection = Self::default();
        for &id in ids {
            if let Some(entity) = entity_with_id(world, id) {
                selection.add(entity);
            }
        }
        selection
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_of(n: usize) -> (World, Vec<Entity>) {
        let mut world = World::new();
        let entities = (0..n)
            .map(|_| {
                let e = world.spawn();
                world.insert(e, SceneId::new());
                e
            })
            .collect();
        (world, entities)
    }

    #[test]
    fn the_last_picked_is_the_primary_and_picking_again_moves_it_last() {
        let (_, e) = world_of(3);
        let mut selection = Selection::single(e[0]);
        selection.add(e[1]);
        selection.add(e[2]);
        assert_eq!(selection.primary(), Some(e[2]));
        selection.add(e[0]);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [e[1], e[2], e[0]]);
        assert_eq!(selection.len(), 3);
    }

    #[test]
    fn a_toggle_adds_what_is_out_and_removes_what_is_in() {
        let (_, e) = world_of(2);
        let mut selection = Selection::single(e[0]);
        selection.toggle(e[1]);
        assert_eq!(selection.primary(), Some(e[1]));
        selection.toggle(e[1]);
        assert_eq!(selection.primary(), Some(e[0]));
        selection.toggle(e[0]);
        assert!(selection.is_empty());
    }

    #[test]
    fn set_replaces_and_clear_empties() {
        let (_, e) = world_of(2);
        let mut selection = Selection::single(e[0]);
        selection.set(Some(e[1]));
        assert_eq!(selection.iter().collect::<Vec<_>>(), [e[1]]);
        selection.set(None);
        assert!(selection.is_empty());
    }

    #[test]
    fn ids_round_trip_in_order_and_forget_what_is_gone() {
        let (mut world, e) = world_of(3);
        let mut selection = Selection::single(e[2]);
        selection.add(e[0]);
        selection.add(e[1]);
        let ids = selection.ids(&world);
        world.despawn(e[0]);
        let back = Selection::from_ids(&world, &ids);
        assert_eq!(back.iter().collect::<Vec<_>>(), [e[2], e[1]]);
        assert_eq!(back.primary(), Some(e[1]));
        let mut alive = selection.clone();
        alive.retain_alive(&world);
        assert_eq!(alive.iter().collect::<Vec<_>>(), [e[2], e[1]]);
        assert_eq!(alive.live_primary(&world), Some(e[1]));
    }

    #[test]
    fn a_shift_click_selects_the_rows_between_ending_on_the_clicked_one() {
        let (_, e) = world_of(5);
        let mut selection = Selection::single(e[1]);
        selection.click(e[3], Click::Extend, &e);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [e[1], e[2], e[3]]);
        // Upward from the primary, which is now e[3].
        selection.click(e[0], Click::Extend, &e);
        assert_eq!(
            selection.iter().collect::<Vec<_>>(),
            [e[3], e[2], e[1], e[0]]
        );
        // No order (the viewport): Shift adds.
        let mut selection = Selection::single(e[1]);
        selection.click(e[4], Click::Extend, &[]);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [e[1], e[4]]);
    }

    #[test]
    fn a_click_replaces_and_a_ctrl_click_toggles() {
        let (_, e) = world_of(2);
        let mut selection = Selection::single(e[0]);
        selection.click(e[1], Click::Toggle, &e);
        assert_eq!(selection.len(), 2);
        selection.click(e[0], Click::Replace, &e);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [e[0]]);
    }

    #[test]
    fn a_box_replaces_or_extends() {
        let (_, e) = world_of(3);
        let mut selection = Selection::single(e[0]);
        selection.take(&[e[1], e[2]], false);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [e[1], e[2]]);
        selection.take(&[e[0]], true);
        assert_eq!(selection.iter().collect::<Vec<_>>(), [e[1], e[2], e[0]]);
    }

    #[test]
    fn ctrl_wins_over_shift() {
        use voltra_core::egui::Modifiers;
        assert_eq!(Click::from_modifiers(Modifiers::NONE), Click::Replace);
        assert_eq!(Click::from_modifiers(Modifiers::SHIFT), Click::Extend);
        assert_eq!(
            Click::from_modifiers(Modifiers::COMMAND | Modifiers::SHIFT),
            Click::Toggle
        );
    }
}
