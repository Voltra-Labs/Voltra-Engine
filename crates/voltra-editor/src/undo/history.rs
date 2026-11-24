//! The two stacks, the edit being built, and the cap on how far back it goes.

use std::collections::BTreeMap;

use voltra_ecs::World;
use voltra_scene::format::{record_scene_id, EntityRecord};
use voltra_scene::{ComponentRegistry, SceneId};

use super::edit::{Edit, EntityChange, Side};
use super::{SceneView, UndoContext, UndoHost};

/// How many entries the history keeps before it drops the oldest.
///
/// Blender ships 32 global steps and Unity and Godot are effectively unbounded.
/// 128 is chosen against this design's per-entry cost, which is the entities an
/// action touched rather than the whole scene: the common entry is one entity's
/// RON, a few hundred bytes. A constant rather than a literal, because a
/// preferences panel is the foreseeable second caller.
pub const MAX_EDITS: usize = 128;

/// One side of an edit, keyed by identity. `None` means "did not exist".
type Records = BTreeMap<SceneId, Option<EntityRecord>>;

/// The edit being built, and what it was called by whoever claimed it.
#[derive(Debug)]
struct Open {
    label: &'static str,
    before: Records,
    selected_before: Vec<SceneId>,
    /// Set by an explicit [`History::begin`]. An explicit edit closes when its
    /// caller says so, not when the watcher stops seeing changes.
    explicit: bool,
}

/// An entry and the number it was given when recorded.
#[derive(Debug)]
struct Entry {
    serial: u64,
    edit: Edit,
}

/// Everything that can be undone, and everything that has been.
#[derive(Debug, Default)]
pub struct History {
    done: Vec<Entry>,
    undone: Vec<Entry>,
    /// The serial the next recorded entry gets. Never reused, so a position
    /// names one state of the scene for as long as the editor runs.
    next_serial: u64,
    open: Option<Open>,
    /// What the ids being watched looked like when this frame started.
    watched: Records,
    watched_selected: Vec<SceneId>,
    /// What a claim this frame called the interaction.
    claim: Option<&'static str>,
    /// Set by an explicit edit or by an apply, so [`History::end_frame`] does
    /// not record the same change a second time.
    recorded_this_frame: bool,
}

impl History {
    /// Captures what `ids` look like now, before any panel has run.
    ///
    /// This is the `before` side of anything an interaction does during the
    /// frame. It has to be taken here rather than when a widget reports a drag:
    /// egui only reports one once the pointer has passed the drag threshold, by
    /// which point the widget has already written the new value.
    pub fn watch(&mut self, view: SceneView<'_>, ids: impl IntoIterator<Item = SceneId>) {
        // This call is what "a new frame" means. Clearing here rather than
        // relying on `end_frame` to consume the flag keeps an undo that fired
        // when no frame was watching — from a menu while playing, from a test —
        // out of the next frame, where it would swallow a real edit.
        self.recorded_this_frame = false;
        self.watched.clear();
        self.watched_selected = view.selected;
        for id in ids {
            match capture(view.world, view.registry, id) {
                Ok(record) => {
                    self.watched.insert(id, record);
                }
                Err(()) => {
                    self.clear();
                    return;
                }
            }
        }
    }

    /// Says the interaction responsible for this frame's changes is still live,
    /// and what to call it.
    pub fn claim(&mut self, label: &'static str) {
        self.claim = Some(label);
    }

    /// Opens an edit over ids the watcher does not cover.
    ///
    /// For actions whose entities are not the selection, or do not exist yet:
    /// spawn, delete, and clearing the scene.
    pub fn begin(
        &mut self,
        label: &'static str,
        view: SceneView<'_>,
        ids: impl IntoIterator<Item = SceneId>,
    ) {
        let mut before = Records::new();
        for id in ids {
            match capture(view.world, view.registry, id) {
                Ok(record) => {
                    before.insert(id, record);
                }
                Err(()) => {
                    self.clear();
                    return;
                }
            }
        }

        self.open = Some(Open {
            label,
            before,
            selected_before: view.selected,
            explicit: true,
        });
    }

    /// Closes the open edit.
    pub fn commit(&mut self, view: SceneView<'_>) {
        self.commit_including(view, []);
    }

    /// Closes the open edit, including ids that appeared while it was open.
    ///
    /// A spawn has no id to name at [`History::begin`] time; anything absent
    /// from the `before` map is recorded as not having existed, which is what
    /// makes the undo of a spawn a despawn.
    pub fn commit_including(
        &mut self,
        view: SceneView<'_>,
        ids: impl IntoIterator<Item = SceneId>,
    ) {
        let Some(mut open) = self.open.take() else {
            return;
        };
        for id in ids {
            open.before.entry(id).or_insert(None);
        }
        self.recorded_this_frame = true;
        self.push(open, view);
    }

    /// Folds this frame's watched changes into the open edit, and closes it
    /// unless a claim says the interaction is still going.
    pub fn end_frame(&mut self, view: SceneView<'_>) {
        let claim = self.claim.take();

        if std::mem::take(&mut self.recorded_this_frame) {
            return;
        }

        let changed = self.watched.iter().any(|(id, before)| {
            capture(view.world, view.registry, *id).is_ok_and(|now| now != *before)
        });

        if changed && self.open.is_none() {
            self.open = Some(Open {
                label: claim.unwrap_or("Edit"),
                before: std::mem::take(&mut self.watched),
                selected_before: self.watched_selected.clone(),
                explicit: false,
            });
        } else if changed {
            if let Some(open) = &mut self.open {
                // A second entity started changing part-way through the same
                // interaction. Its `before` is this frame's, which is the oldest
                // state the history ever saw it in.
                for (id, before) in std::mem::take(&mut self.watched) {
                    open.before.entry(id).or_insert(before);
                }
            }
        }

        // A claim keeps a watcher-opened edit open; an explicit one is closed by
        // its own caller and must not be closed here.
        let keep_open = self
            .open
            .as_ref()
            .is_some_and(|open| open.explicit || claim.is_some());
        if !keep_open {
            if let Some(open) = self.open.take() {
                self.push(open, view);
            }
        }
    }

    /// Puts the previous entry's `before` side back. Returns whether there was
    /// one.
    pub fn undo(&mut self, ctx: UndoContext<'_>, host: &mut dyn UndoHost) -> bool {
        let Some(entry) = self.done.pop() else {
            return false;
        };
        apply(&entry.edit, Side::Before, ctx, host);
        self.undone.push(entry);
        self.recorded_this_frame = true;
        true
    }

    /// Replays the entry the last undo put back. Returns whether there was one.
    pub fn redo(&mut self, ctx: UndoContext<'_>, host: &mut dyn UndoHost) -> bool {
        let Some(entry) = self.undone.pop() else {
            return false;
        };
        apply(&entry.edit, Side::After, ctx, host);
        self.done.push(entry);
        self.recorded_this_frame = true;
        true
    }

    /// What the next undo would put back.
    pub fn undo_label(&self) -> Option<&'static str> {
        self.done.last().map(|entry| entry.edit.label)
    }

    /// What the next redo would replay.
    pub fn redo_label(&self) -> Option<&'static str> {
        self.undone.last().map(|entry| entry.edit.label)
    }

    /// Forgets everything, including any edit in progress.
    ///
    /// `Scene ▸ Open` is the caller: every id in the stack belongs to the scene
    /// being closed.
    pub fn clear(&mut self) {
        self.done.clear();
        self.undone.clear();
        self.open = None;
        self.watched.clear();
    }

    /// Records nothing at the end of this frame, and forgets what it
    /// watched: the scene was replaced under it wholesale — set aside for a
    /// prefab, or brought back — so comparing the frame's start with its
    /// end would record the swap as an edit. The entries are kept; the next
    /// frame's [`History::watch`] starts afresh.
    pub fn skip_frame(&mut self) {
        self.watched.clear();
        self.open = None;
        self.claim = None;
        self.recorded_this_frame = true;
    }

    /// How many entries can be undone.
    ///
    /// The UI asks [`History::is_empty`] and [`History::undo_label`]; the count
    /// itself is only ever a test's question — "one entry for the whole drag,
    /// not one per frame" is the property this whole design exists for, and it
    /// cannot be stated without it.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.done.len()
    }

    /// Which state of the scene the history is at: the serial of the entry
    /// an undo would take back, `0` with none. Undo and redo move it; an edit
    /// moves it somewhere it has never been. What the editor compares with
    /// the position it saved at to know whether the scene has unsaved
    /// changes — Godot's answer, which makes undoing back to the save clean.
    pub fn position(&self) -> u64 {
        self.done.last().map_or(0, |entry| entry.serial)
    }

    /// Whether there is anything to undo.
    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }

    /// How many entries can be redone.
    pub fn redo_len(&self) -> usize {
        self.undone.len()
    }

    /// Captures the `after` side and pushes the entry, unless nothing changed.
    fn push(&mut self, open: Open, view: SceneView<'_>) {
        let mut entities = Vec::with_capacity(open.before.len());
        for (id, before) in open.before {
            match capture(view.world, view.registry, id) {
                Ok(after) => entities.push(EntityChange { id, before, after }),
                Err(()) => {
                    self.clear();
                    return;
                }
            }
        }

        let edit = Edit {
            label: open.label,
            entities,
            selected_before: open.selected_before,
            selected_after: view.selected,
        };

        if edit.is_noop() {
            return;
        }

        self.undone.clear();
        self.next_serial += 1;
        self.done.push(Entry {
            serial: self.next_serial,
            edit,
        });
        if self.done.len() > MAX_EDITS {
            self.done.remove(0);
        }
    }
}

/// One id's current state, or `Err` when it could not be serialized.
///
/// The error is deliberately unit: the caller's only response is to clear the
/// history, and the reason has already been logged where it was known.
fn capture(
    world: &World,
    registry: &ComponentRegistry,
    id: SceneId,
) -> Result<Option<EntityRecord>, ()> {
    match record_scene_id(world, registry, id) {
        Some(Ok(record)) => Ok(Some(record)),
        None => Ok(None),
        Some(Err(error)) => {
            // Dropping just this entry would leave a stack that lies: the next
            // undo would restore a state from before the action nobody
            // recorded, silently discarding it. Losing undo is visible; losing
            // work to a history that misrepresents itself is not.
            log::error!("undo history cleared: an entity could not be recorded: {error}");
            Err(())
        }
    }
}

/// Puts `side` of `edit` back, with the three consequences every apply has.
fn apply(edit: &Edit, side: Side, ctx: UndoContext<'_>, host: &mut dyn UndoHost) {
    // Before anything is despawned: a `Drag` holds an `Entity` and a grab
    // offset, and both are stale the moment an entity is respawned.
    ctx.gizmo.cancel_drag();

    if let Err(error) = edit.apply(side, host.world(), ctx.registry) {
        // Cannot happen in practice: this RON was produced in this process, by
        // this build, with this registry. Logged loudly all the same, because at
        // this point part of the scene may be on the wrong side of the edit.
        log::error!("could not apply an undo entry: {error}");
    }

    host.reset_physics();
    host.resolve_scene_assets();

    *ctx.selected = edit.selected_entities(side, host.world());
}

#[cfg(test)]
mod tests;
