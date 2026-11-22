use super::*;
use crate::gizmo::Gizmo;
use crate::selection::Selection;
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
    fn resolve_scene_assets(&mut self) {
        self.resolves += 1;
    }
}

/// The editor fields an undo touches, kept together.
struct Fixture {
    registry: ComponentRegistry,
    selected: Selection,
    gizmo: Gizmo,
}

impl Fixture {
    fn new() -> Self {
        Self {
            registry: ComponentRegistry::with_defaults(),
            selected: Selection::default(),
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
        selected: fixture.selected.ids(&host.world),
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
    fixture
        .selected
        .set(voltra_scene::format::entity_with_id(&host.world, id));

    history.begin("Delete", view(&host, &fixture), [id]);
    let entity = voltra_scene::format::entity_with_id(&host.world, id).expect("there");
    host.world.despawn(entity);
    fixture.selected.clear();
    history.commit(view(&host, &fixture));

    history.undo(fixture.ctx(), &mut host);

    assert_eq!(x_of(&host.world, id), Some(6.0));
    assert_eq!(
        fixture.selected.primary(),
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
