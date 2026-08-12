//! Whether the editor is authoring the scene or simulating it.
//!
//! The state machine, the snapshot Stop puts back, and the transitions between
//! them. Kept apart from the panel that drives it because all of this is pure
//! logic over a `World`: the toolbar is layout and dispatch, and everything
//! here that can be got wrong is testable without egui or a GPU.
//!
//! Splits into `play.rs` + `play/` the moment it grows a second concept — the
//! likely one being the per-entity diff a future "keep simulation changes"
//! would need.

use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_scene::format::{from_scene_file, to_scene_file, SceneFile};
use voltra_scene::{ComponentRegistry, SceneId};

use crate::gizmo::Gizmo;

/// Whether the editor is authoring the scene or simulating it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    #[default]
    Editing,
    Playing,
    Paused,
}

/// The scene as it was when play began, and what Stop puts back.
#[derive(Debug)]
pub struct PlaySnapshot {
    scene: SceneFile,
    /// Which entity was selected, by scene identity rather than by `Entity` —
    /// the restore despawns and respawns, so every handle is stale after it.
    selected: Option<SceneId>,
}

/// What a play transition needs from the running application.
///
/// A trait rather than [`UiFrame`] itself, for two reasons. It is what makes
/// the transitions testable at all — a `UiFrame` cannot exist without a
/// `wgpu::Device` — and it names exactly what play is allowed to touch, which
/// is narrower than what a panel can reach.
pub trait PlayHost {
    /// The scene: snapshotted, despawned and respawned by a transition.
    fn world(&mut self) -> &mut World;
    /// Whether each frame runs the physics steps it owes.
    fn set_simulating(&mut self, simulating: bool);
    /// Runs `count` fixed steps on the next frame regardless of the switch.
    fn request_steps(&mut self, count: u32);
    /// Forgets the accumulated impulses and the clock's banked time.
    fn reset_physics(&mut self);
    /// Re-resolves every sprite's GPU handle from its path.
    ///
    /// A restore respawns every entity, and `Sprite::texture_handle` is
    /// `#[serde(skip)]` — a handle addresses a slot in this session's
    /// `Textures` and means nothing across a round trip. Without this every
    /// textured sprite draws flat white after Stop.
    fn resolve_sprite_textures(&mut self);
}

impl PlayHost for UiFrame<'_> {
    fn world(&mut self) -> &mut World {
        self.world
    }

    fn set_simulating(&mut self, simulating: bool) {
        UiFrame::set_simulating(self, simulating);
    }

    fn request_steps(&mut self, count: u32) {
        UiFrame::request_steps(self, count);
    }

    fn reset_physics(&mut self) {
        UiFrame::reset_physics(self);
    }

    fn resolve_sprite_textures(&mut self) {
        UiFrame::resolve_sprite_textures(self);
    }
}

/// The editor state a transition reads or writes besides the world.
pub struct PlayContext<'a> {
    /// Snapshot and restore must use the same registry as each other. The
    /// editor owns one and passes it here rather than building one per call,
    /// so that is guaranteed rather than remembered.
    pub registry: &'a ComponentRegistry,
    pub selected: &'a mut Option<Entity>,
    pub gizmo: &'a mut Gizmo,
}

/// The editor's play state and the scene it will put back.
#[derive(Debug, Default)]
pub struct Play {
    state: PlayState,
    /// `Some` from the Play that left `Editing` until the Stop that consumes it.
    snapshot: Option<PlaySnapshot>,
}

impl Play {
    pub fn state(&self) -> PlayState {
        self.state
    }

    /// Enters `Playing`, snapshotting the scene if there is not one already.
    ///
    /// From `Paused` this must **not** re-snapshot: that would silently make
    /// the paused mid-air scene the thing Stop restores.
    ///
    /// A snapshot that cannot be taken refuses the transition. `to_scene_file`
    /// returns a `Result` and the failure is real — a component whose
    /// `Serialize` fails — and entering play without a way back is the one
    /// outcome this whole mode exists to prevent. So the error path is the
    /// design, not an afterthought: it logs and stays in `Editing`.
    ///
    /// Illegal transitions are no-ops rather than errors. The toolbar shows
    /// Play or Pause and never both, so a no-op here keeps the buttons from
    /// having to know the state twice.
    pub fn play(&mut self, ctx: PlayContext<'_>, host: &mut dyn PlayHost) {
        match self.state {
            PlayState::Playing => return,
            PlayState::Paused => {}
            PlayState::Editing => {
                let scene = match to_scene_file(host.world(), ctx.registry) {
                    Ok(scene) => scene,
                    Err(e) => {
                        log::error!(
                            "staying in edit mode: the scene could not be snapshotted: {e}"
                        );
                        return;
                    }
                };
                let selected = selected_id(host.world(), *ctx.selected);
                self.snapshot = Some(PlaySnapshot { scene, selected });
            }
        }

        self.state = PlayState::Playing;
        host.set_simulating(true);
    }

    /// Stops stepping and leaves the scene exactly where it is.
    ///
    /// No snapshot is taken or consumed. Pause is a switch on simulation, not
    /// on state that has to be saved.
    pub fn pause(&mut self, host: &mut dyn PlayHost) {
        if self.state != PlayState::Playing {
            return;
        }
        self.state = PlayState::Paused;
        host.set_simulating(false);
    }

    /// Runs exactly one fixed step, in `Paused` and nowhere else.
    ///
    /// Step outside play is not a stepper: there would be no snapshot to put
    /// back, so a single step would be an edit nothing can undo.
    pub fn step(&mut self, host: &mut dyn PlayHost) {
        if self.state != PlayState::Paused {
            return;
        }
        host.request_steps(1);
    }

    /// Leaves play and puts the snapshot back, discarding everything play did.
    ///
    /// Not undo, and it must not be described as one: there is no undo stack in
    /// this editor. It restores the snapshot, and nothing else.
    ///
    /// The order is load-bearing:
    ///
    /// 1. **Simulation off**, before anything is despawned, so no step can run
    ///    against a half-restored world.
    /// 2. **Cancel any gizmo drag** — a `Drag` holds an `Entity` and a grab
    ///    offset, and both are stale the moment step 3 runs.
    /// 3. **Despawn** every entity carrying a [`SceneId`], which is the same
    ///    rule `Scene ▸ Clear` uses and the same one the snapshot captured by.
    /// 4. **Restore** the snapshot.
    /// 5. **Reset physics**: impulses and the clock's banked time.
    /// 6. **Re-resolve textures**, because every handle died with step 3.
    /// 7. **Re-resolve the selection** by identity.
    ///
    /// An entity with no `SceneId` is transient by that same definition and is
    /// left alone in both directions — never captured, never despawned.
    pub fn stop(&mut self, ctx: PlayContext<'_>, host: &mut dyn PlayHost) {
        if self.state == PlayState::Editing {
            return;
        }

        host.set_simulating(false);
        self.state = PlayState::Editing;
        ctx.gizmo.cancel_drag();

        let Some(snapshot) = self.snapshot.take() else {
            // Unreachable: `Editing` returned above, and every other state was
            // entered by a Play that stored one. Logged rather than asserted,
            // because the alternative is panicking an editor over a scene that
            // is still perfectly usable where it stands.
            log::error!("stopped play with no snapshot; the scene is left as it is");
            *ctx.selected = None;
            return;
        };

        let scene_entities: Vec<Entity> = host.world().query::<SceneId>().map(|(e, _)| e).collect();
        for entity in scene_entities {
            host.world().despawn(entity);
        }

        if let Err(e) = from_scene_file(&snapshot.scene, ctx.registry, host.world()) {
            // Cannot happen in practice: this file was produced in this
            // process, by this build, with this registry, moments ago, so
            // neither the version check nor an unknown component can trip.
            // Logged loudly rather than swallowed all the same — at this point
            // the scene is gone, and the user needs telling rather than an
            // empty viewport to wonder at.
            log::error!("could not restore the scene after stopping play: {e}");
        }

        host.reset_physics();
        host.resolve_sprite_textures();

        // An entity selected during play that did not exist at snapshot time
        // leaves the selection empty. There is nothing left for it to name.
        *ctx.selected = snapshot
            .selected
            .and_then(|id| entity_with_id(host.world(), id));
    }
}

/// The scene identity of `entity`, if it has one.
fn selected_id(world: &World, entity: Option<Entity>) -> Option<SceneId> {
    entity.and_then(|e| world.get::<SceneId>(e).copied())
}

/// The entity carrying `id`, if the world holds one.
fn entity_with_id(world: &World, id: SceneId) -> Option<Entity> {
    world
        .query::<SceneId>()
        .find(|(_, other)| **other == id)
        .map(|(entity, _)| entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ron::value::RawValue;
    use voltra_render::glam::Vec2;
    use voltra_scene::{Sprite, Transform, UnknownComponents};

    /// A [`PlayHost`] with no `App` and no GPU behind it.
    ///
    /// The counters are what the transitions' side effects are asserted
    /// against: a call this fake records is a call `UiFrame` would forward to
    /// the real switch.
    struct FakeHost {
        world: World,
        simulating: bool,
        steps: u32,
        resets: u32,
        resolves: u32,
    }

    impl FakeHost {
        fn new() -> Self {
            Self {
                world: World::new(),
                simulating: false,
                steps: 0,
                resets: 0,
                resolves: 0,
            }
        }
    }

    impl PlayHost for FakeHost {
        fn world(&mut self) -> &mut World {
            &mut self.world
        }
        fn set_simulating(&mut self, simulating: bool) {
            self.simulating = simulating;
        }
        fn request_steps(&mut self, count: u32) {
            self.steps += count;
        }
        fn reset_physics(&mut self) {
            self.resets += 1;
        }
        fn resolve_sprite_textures(&mut self) {
            self.resolves += 1;
        }
    }

    /// The three editor fields a transition touches, kept together so a test
    /// body reads as the transition it exercises.
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

        fn ctx(&mut self) -> PlayContext<'_> {
            PlayContext {
                registry: &self.registry,
                selected: &mut self.selected,
                gizmo: &mut self.gizmo,
            }
        }
    }

    /// An entity that is part of the scene: it carries an identity.
    fn spawn_scene_entity(world: &mut World, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, SceneId::new());
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, Sprite::default());
        entity
    }

    fn translation(world: &World, entity: Entity) -> Vec2 {
        world
            .get::<Transform>(entity)
            .expect("the transform is there")
            .translation
    }

    /// The one entity in the world, found by identity rather than by handle.
    fn only_entity(world: &World) -> Entity {
        let mut all = world.query::<SceneId>().map(|(e, _)| e);
        let entity = all.next().expect("exactly one scene entity");
        assert!(all.next().is_none(), "exactly one scene entity");
        entity
    }

    #[test]
    fn play_from_editing_takes_a_snapshot() {
        let mut host = FakeHost::new();
        spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let mut fixture = Fixture::new();
        let mut play = Play::default();

        play.play(fixture.ctx(), &mut host);

        assert_eq!(play.state(), PlayState::Playing);
        assert!(play.snapshot.is_some());
        assert!(host.simulating);
    }

    #[test]
    fn play_from_paused_does_not_replace_the_snapshot() {
        // The regression that would make Stop restore a mid-air scene: pause is
        // a switch on simulation, not a new starting point.
        let mut host = FakeHost::new();
        let entity = spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let mut fixture = Fixture::new();
        let mut play = Play::default();

        play.play(fixture.ctx(), &mut host);
        play.pause(&mut host);
        host.world
            .get_mut::<Transform>(entity)
            .expect("the transform is there")
            .translation = Vec2::new(0.0, -5.0);
        play.play(fixture.ctx(), &mut host);

        play.stop(fixture.ctx(), &mut host);

        let restored = only_entity(&host.world);
        assert_eq!(translation(&host.world, restored), Vec2::ZERO);
    }

    #[test]
    fn pause_stops_the_simulation_without_leaving_play() {
        let mut host = FakeHost::new();
        spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let mut fixture = Fixture::new();
        let mut play = Play::default();
        play.play(fixture.ctx(), &mut host);

        play.pause(&mut host);

        assert_eq!(play.state(), PlayState::Paused);
        assert!(!host.simulating);
        assert!(play.snapshot.is_some(), "pause must not consume it");
    }

    #[test]
    fn step_in_editing_does_nothing() {
        // Step outside play is not a stepper: there is no snapshot to put back.
        let mut host = FakeHost::new();
        let mut play = Play::default();

        play.step(&mut host);

        assert_eq!(play.state(), PlayState::Editing);
        assert_eq!(host.steps, 0);
    }

    #[test]
    fn step_while_paused_asks_for_exactly_one() {
        let mut host = FakeHost::new();
        spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let mut fixture = Fixture::new();
        let mut play = Play::default();
        play.play(fixture.ctx(), &mut host);
        play.pause(&mut host);

        play.step(&mut host);

        assert_eq!(host.steps, 1);
        assert_eq!(play.state(), PlayState::Paused, "a step is not a resume");
        assert!(!host.simulating);
    }

    #[test]
    fn stop_in_editing_does_nothing() {
        let mut host = FakeHost::new();
        spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let before = host.world.entity_count();
        let mut fixture = Fixture::new();
        let mut play = Play::default();

        play.stop(fixture.ctx(), &mut host);

        assert_eq!(play.state(), PlayState::Editing);
        assert_eq!(host.world.entity_count(), before);
        assert_eq!(host.resets, 0, "nothing was simulated, nothing to reset");
    }

    #[test]
    fn a_failed_snapshot_leaves_the_editor_editing() {
        // Entering play without a way back is the one outcome this mode exists
        // to prevent, so the error path is the design.
        let mut host = FakeHost::new();
        let entity = spawn_scene_entity(&mut host.world, Vec2::ZERO);
        host.world.insert(entity, Unserialisable);
        let mut fixture = Fixture::new();
        fixture
            .registry
            .register::<Unserialisable>("Unserialisable");
        let mut play = Play::default();

        play.play(fixture.ctx(), &mut host);

        assert_eq!(play.state(), PlayState::Editing);
        assert!(play.snapshot.is_none());
        assert!(!host.simulating);
    }

    #[test]
    fn stop_restores_a_moved_transform() {
        let mut host = FakeHost::new();
        let entity = spawn_scene_entity(&mut host.world, Vec2::new(1.0, 2.0));
        let mut fixture = Fixture::new();
        let mut play = Play::default();
        play.play(fixture.ctx(), &mut host);
        host.world
            .get_mut::<Transform>(entity)
            .expect("the transform is there")
            .translation = Vec2::new(-9.0, -9.0);

        play.stop(fixture.ctx(), &mut host);

        let restored = only_entity(&host.world);
        assert_eq!(translation(&host.world, restored), Vec2::new(1.0, 2.0));
        assert_eq!(play.state(), PlayState::Editing);
        assert!(!host.simulating);
    }

    #[test]
    fn stop_despawns_what_play_spawned() {
        let mut host = FakeHost::new();
        spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let mut fixture = Fixture::new();
        let mut play = Play::default();
        play.play(fixture.ctx(), &mut host);
        spawn_scene_entity(&mut host.world, Vec2::new(5.0, 5.0));

        play.stop(fixture.ctx(), &mut host);

        assert_eq!(host.world.query::<SceneId>().count(), 1);
        let survivor = only_entity(&host.world);
        assert_eq!(translation(&host.world, survivor), Vec2::ZERO);
    }

    #[test]
    fn stop_respawns_what_play_despawned() {
        let mut host = FakeHost::new();
        let entity = spawn_scene_entity(&mut host.world, Vec2::new(3.0, 0.0));
        let id = *host.world.get::<SceneId>(entity).expect("it has one");
        let mut fixture = Fixture::new();
        let mut play = Play::default();
        play.play(fixture.ctx(), &mut host);
        host.world.despawn(entity);

        play.stop(fixture.ctx(), &mut host);

        let restored = entity_with_id(&host.world, id).expect("it must come back");
        assert_eq!(translation(&host.world, restored), Vec2::new(3.0, 0.0));
    }

    #[test]
    fn an_entity_without_a_scene_id_is_left_alone() {
        // Transient by the same definition `Scene ▸ Clear` uses: not captured
        // by the snapshot, and not despawned by the restore.
        let mut host = FakeHost::new();
        spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let transient = host.world.spawn();
        host.world
            .insert(transient, Transform::from_translation(Vec2::new(7.0, 7.0)));
        let mut fixture = Fixture::new();
        let mut play = Play::default();

        play.play(fixture.ctx(), &mut host);
        play.stop(fixture.ctx(), &mut host);

        assert!(
            host.world.is_alive(transient),
            "it must survive the restore"
        );
        assert_eq!(translation(&host.world, transient), Vec2::new(7.0, 7.0));
    }

    #[test]
    fn unknown_components_survive_the_round_trip() {
        // The snapshot goes through the same format a save does, so a scene
        // authored by a newer build must not lose anything to a play session.
        let mut host = FakeHost::new();
        let entity = spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let value: Box<RawValue> = RawValue::from_ron("(mass: 5.0)")
            .expect("valid RON")
            .to_owned();
        let mut unknown = UnknownComponents::default();
        unknown.0.insert("Physics".to_owned(), value.clone());
        host.world.insert(entity, unknown);
        let mut fixture = Fixture::new();
        let mut play = Play::default();

        play.play(fixture.ctx(), &mut host);
        play.stop(fixture.ctx(), &mut host);

        let restored = only_entity(&host.world);
        let kept = host
            .world
            .get::<UnknownComponents>(restored)
            .expect("the unknown component must come back");
        assert_eq!(kept.0.get("Physics"), Some(&value));
    }

    #[test]
    fn the_selection_survives_by_scene_id() {
        // Every `Entity` handle is stale after a despawn-and-respawn, so the
        // selection is matched by identity or not at all.
        let mut host = FakeHost::new();
        let entity = spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let id = *host.world.get::<SceneId>(entity).expect("it has one");
        let mut fixture = Fixture::new();
        fixture.selected = Some(entity);
        let mut play = Play::default();

        play.play(fixture.ctx(), &mut host);
        play.stop(fixture.ctx(), &mut host);

        let selected = fixture.selected.expect("the selection must survive");
        assert_eq!(host.world.get::<SceneId>(selected), Some(&id));
    }

    #[test]
    fn a_selection_made_during_play_is_cleared_by_stop() {
        let mut host = FakeHost::new();
        spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let mut fixture = Fixture::new();
        let mut play = Play::default();
        play.play(fixture.ctx(), &mut host);
        fixture.selected = Some(spawn_scene_entity(&mut host.world, Vec2::new(2.0, 0.0)));

        play.stop(fixture.ctx(), &mut host);

        assert_eq!(
            fixture.selected, None,
            "it named an entity the snapshot never held"
        );
    }

    #[test]
    fn stop_resets_physics_and_re_resolves_textures() {
        // The impulses and the clock's debt belong to a world that no longer
        // exists, and every texture handle died with the despawn.
        let mut host = FakeHost::new();
        spawn_scene_entity(&mut host.world, Vec2::ZERO);
        let mut fixture = Fixture::new();
        let mut play = Play::default();
        play.play(fixture.ctx(), &mut host);

        play.stop(fixture.ctx(), &mut host);

        assert_eq!(host.resets, 1);
        assert_eq!(host.resolves, 1);
    }

    /// A component whose `Serialize` always fails — the one real way
    /// `to_scene_file` can error.
    struct Unserialisable;

    impl serde::Serialize for Unserialisable {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            use serde::ser::Error as _;
            Err(S::Error::custom("this component refuses to serialise"))
        }
    }

    impl<'de> serde::Deserialize<'de> for Unserialisable {
        fn deserialize<D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
            use serde::de::Error as _;
            Err(D::Error::custom("this component refuses to deserialise"))
        }
    }
}
