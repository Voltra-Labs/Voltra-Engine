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
    selected_before: Option<SceneId>,
    /// Set by an explicit [`History::begin`]. An explicit edit closes when its
    /// caller says so, not when the watcher stops seeing changes.
    explicit: bool,
}

/// Everything that can be undone, and everything that has been.
#[derive(Debug, Default)]
pub struct History {
    done: Vec<Edit>,
    undone: Vec<Edit>,
    open: Option<Open>,
    /// What the ids being watched looked like when this frame started.
    watched: Records,
    watched_selected: Option<SceneId>,
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
                selected_before: self.watched_selected,
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
        let Some(edit) = self.done.pop() else {
            return false;
        };
        apply(&edit, Side::Before, ctx, host);
        self.undone.push(edit);
        self.recorded_this_frame = true;
        true
    }

    /// Replays the entry the last undo put back. Returns whether there was one.
    pub fn redo(&mut self, ctx: UndoContext<'_>, host: &mut dyn UndoHost) -> bool {
        let Some(edit) = self.undone.pop() else {
            return false;
        };
        apply(&edit, Side::After, ctx, host);
        self.done.push(edit);
        self.recorded_this_frame = true;
        true
    }

    /// What the next undo would put back.
    pub fn undo_label(&self) -> Option<&'static str> {
        self.done.last().map(|edit| edit.label)
    }

    /// What the next redo would replay.
    pub fn redo_label(&self) -> Option<&'static str> {
        self.undone.last().map(|edit| edit.label)
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

    /// How many entries can be undone.
    pub fn len(&self) -> usize {
        self.done.len()
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
        self.done.push(edit);
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
    host.resolve_sprite_textures();

    *ctx.selected = edit.selected_entity(side, host.world());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gizmo::Gizmo;
    use crate::undo::selected_id;
    use voltra_ecs::Entity;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Sprite, Transform};

    /// An [`UndoHost`] with no `App` and no GPU behind it.
    struct FakeHost {
        world: World,
        resets: u32,
        resolves: u32,
    }

    impl FakeHost {
        fn new() -> Self {
            Self {
                world: World::new(),
                resets: 0,
                resolves: 0,
            }
        }
    }

    impl UndoHost for FakeHost {
        fn world(&mut self) -> &mut World {
            &mut self.world
        }
        fn reset_physics(&mut self) {
            self.resets += 1;
        }
        fn resolve_sprite_textures(&mut self) {
            self.resolves += 1;
        }
    }

    /// The editor fields an undo touches, kept together.
    struct Fixture {
        registry: ComponentRegistry,
        selected: Option<Entity>,
        gizmo: Gizmo,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                registry: ComponentRegistry::with_defaults(),
                selected: None,
                gizmo: Gizmo::default(),
            }
        }

        fn ctx(&mut self) -> UndoContext<'_> {
            UndoContext {
                registry: &self.registry,
                selected: &mut self.selected,
                gizmo: &mut self.gizmo,
            }
        }
    }

    fn view<'a>(host: &'a FakeHost, fixture: &'a Fixture) -> SceneView<'a> {
        SceneView {
            world: &host.world,
            registry: &fixture.registry,
            selected: selected_id(&host.world, fixture.selected),
        }
    }

    /// Spawns a sprite at `x` and returns its identity.
    fn spawn(world: &mut World, x: f32) -> SceneId {
        let entity = world.spawn();
        let id = SceneId::new();
        world.insert(entity, id);
        world.insert(entity, Transform::from_translation(Vec2::new(x, 0.0)));
        world.insert(entity, Sprite::default());
        id
    }

    fn move_to(world: &mut World, id: SceneId, x: f32) {
        let entity = voltra_scene::format::entity_with_id(world, id).expect("it is there");
        world
            .get_mut::<Transform>(entity)
            .expect("it has one")
            .translation = Vec2::new(x, 0.0);
    }

    fn x_of(world: &World, id: SceneId) -> Option<f32> {
        let entity = voltra_scene::format::entity_with_id(world, id)?;
        Some(world.get::<Transform>(entity)?.translation.x)
    }

    #[test]
    fn a_watched_change_with_no_claim_becomes_one_entry() {
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 1.0);

        history.watch(view(&host, &fixture), [id]);
        move_to(&mut host.world, id, 5.0);
        history.end_frame(view(&host, &fixture));

        assert_eq!(history.len(), 1);
        assert!(history.undo(fixture.ctx(), &mut host));
        assert_eq!(x_of(&host.world, id), Some(1.0));
    }

    #[test]
    fn a_claim_every_frame_makes_one_entry_not_one_per_frame() {
        // The property the whole design exists for: a gizmo drag is one Ctrl+Z,
        // not one per frame of the drag.
        let mut host = FakeHost::new();
        let fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 0.0);

        for step in 1..=5 {
            history.watch(view(&host, &fixture), [id]);
            move_to(&mut host.world, id, step as f32);
            history.claim("Move");
            history.end_frame(view(&host, &fixture));
        }
        assert_eq!(history.len(), 0, "still open while the claims arrive");

        // The release frame: no claim.
        history.watch(view(&host, &fixture), [id]);
        history.end_frame(view(&host, &fixture));

        assert_eq!(history.len(), 1);
        assert_eq!(history.undo_label(), Some("Move"));
    }

    #[test]
    fn undoing_a_drag_returns_to_where_it_started() {
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 0.0);

        for step in 1..=3 {
            history.watch(view(&host, &fixture), [id]);
            move_to(&mut host.world, id, step as f32);
            history.claim("Move");
            history.end_frame(view(&host, &fixture));
        }
        history.watch(view(&host, &fixture), [id]);
        history.end_frame(view(&host, &fixture));

        history.undo(fixture.ctx(), &mut host);
        assert_eq!(
            x_of(&host.world, id),
            Some(0.0),
            "not 2.0: the whole drag is one entry"
        );
    }

    #[test]
    fn nothing_changed_pushes_nothing() {
        let mut host = FakeHost::new();
        let fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 1.0);

        history.watch(view(&host, &fixture), [id]);
        history.claim("Move");
        history.end_frame(view(&host, &fixture));
        history.watch(view(&host, &fixture), [id]);
        history.end_frame(view(&host, &fixture));

        assert_eq!(
            history.len(),
            0,
            "a grab that moved nothing is not an entry"
        );
    }

    #[test]
    fn redo_replays_the_change() {
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 1.0);

        history.watch(view(&host, &fixture), [id]);
        move_to(&mut host.world, id, 5.0);
        history.end_frame(view(&host, &fixture));

        history.undo(fixture.ctx(), &mut host);
        assert_eq!(x_of(&host.world, id), Some(1.0));
        assert!(history.redo(fixture.ctx(), &mut host));
        assert_eq!(x_of(&host.world, id), Some(5.0));
    }

    #[test]
    fn a_new_edit_clears_the_redo_stack() {
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 1.0);

        history.watch(view(&host, &fixture), [id]);
        move_to(&mut host.world, id, 2.0);
        history.end_frame(view(&host, &fixture));
        history.undo(fixture.ctx(), &mut host);
        assert_eq!(history.redo_len(), 1);

        history.watch(view(&host, &fixture), [id]);
        move_to(&mut host.world, id, 9.0);
        history.end_frame(view(&host, &fixture));

        assert_eq!(
            history.redo_len(),
            0,
            "the branch that was redone away is gone"
        );
    }

    #[test]
    fn undo_and_redo_report_whether_there_was_anything_to_do() {
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();

        assert!(!history.undo(fixture.ctx(), &mut host));
        assert!(!history.redo(fixture.ctx(), &mut host));
    }

    #[test]
    fn an_explicit_edit_records_a_spawn() {
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();

        history.begin("Spawn sprite", view(&host, &fixture), []);
        let id = spawn(&mut host.world, 4.0);
        history.commit_including(view(&host, &fixture), [id]);

        assert_eq!(history.len(), 1);
        history.undo(fixture.ctx(), &mut host);
        assert_eq!(host.world.entity_count(), 0);
        history.redo(fixture.ctx(), &mut host);
        assert_eq!(x_of(&host.world, id), Some(4.0));
    }

    #[test]
    fn an_explicit_edit_records_a_delete_and_restores_the_selection() {
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 6.0);
        fixture.selected = voltra_scene::format::entity_with_id(&host.world, id);

        history.begin("Delete", view(&host, &fixture), [id]);
        let entity = voltra_scene::format::entity_with_id(&host.world, id).expect("there");
        host.world.despawn(entity);
        fixture.selected = None;
        history.commit(view(&host, &fixture));

        history.undo(fixture.ctx(), &mut host);

        assert_eq!(x_of(&host.world, id), Some(6.0));
        assert_eq!(
            fixture.selected,
            voltra_scene::format::entity_with_id(&host.world, id),
            "undoing a delete selects what it brought back"
        );
    }

    #[test]
    fn an_explicit_edit_stops_the_watcher_recording_it_twice() {
        let mut host = FakeHost::new();
        let fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 1.0);

        history.watch(view(&host, &fixture), [id]);
        history.begin("Delete", view(&host, &fixture), [id]);
        let entity = voltra_scene::format::entity_with_id(&host.world, id).expect("there");
        host.world.despawn(entity);
        history.commit(view(&host, &fixture));
        history.end_frame(view(&host, &fixture));

        assert_eq!(history.len(), 1, "one delete, one entry");
    }

    #[test]
    fn an_undo_does_not_become_an_entry() {
        // The watcher runs at the top of the frame and the shortcut fires inside
        // it, so without the recorded flag every Ctrl+Z would push the undo
        // itself and the second Ctrl+Z would redo it forever.
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 1.0);

        history.watch(view(&host, &fixture), [id]);
        move_to(&mut host.world, id, 5.0);
        history.end_frame(view(&host, &fixture));

        history.watch(view(&host, &fixture), [id]);
        history.undo(fixture.ctx(), &mut host);
        history.end_frame(view(&host, &fixture));

        assert_eq!(history.len(), 0);
        assert_eq!(history.redo_len(), 1);
    }

    #[test]
    fn the_cap_drops_the_oldest_entry() {
        let mut host = FakeHost::new();
        let fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 0.0);

        for step in 1..=(MAX_EDITS + 10) {
            history.watch(view(&host, &fixture), [id]);
            move_to(&mut host.world, id, step as f32);
            history.end_frame(view(&host, &fixture));
        }

        assert_eq!(history.len(), MAX_EDITS);
    }

    #[test]
    fn an_apply_resets_physics_and_re_resolves_textures() {
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 1.0);

        history.watch(view(&host, &fixture), [id]);
        move_to(&mut host.world, id, 5.0);
        history.end_frame(view(&host, &fixture));
        history.undo(fixture.ctx(), &mut host);

        assert_eq!(host.resets, 1, "the contact cache is keyed by Entity");
        assert_eq!(
            host.resolves, 1,
            "a record carries the path, not the handle"
        );
    }

    #[test]
    fn clear_empties_both_stacks() {
        let mut host = FakeHost::new();
        let mut fixture = Fixture::new();
        let mut history = History::default();
        let id = spawn(&mut host.world, 1.0);

        history.watch(view(&host, &fixture), [id]);
        move_to(&mut host.world, id, 2.0);
        history.end_frame(view(&host, &fixture));
        history.undo(fixture.ctx(), &mut host);

        history.clear();

        assert_eq!(history.len(), 0);
        assert_eq!(history.redo_len(), 0);
        assert_eq!(history.undo_label(), None);
        assert_eq!(history.redo_label(), None);
    }
}
