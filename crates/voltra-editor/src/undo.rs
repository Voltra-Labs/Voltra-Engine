//! Undoing and redoing scene edits.

pub mod edit;
pub mod history;
pub mod shortcut;

use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_scene::{ComponentRegistry, SceneId};

use crate::gizmo::Gizmo;

pub use history::History;

/// What the history needs from the running application to put a scene back.
///
/// A trait rather than [`UiFrame`] itself, for the two reasons
/// [`crate::play::PlayHost`] is one: a `UiFrame` cannot exist without a
/// `wgpu::Device`, so this is what makes undo testable at all, and it names
/// exactly what undo may touch — which is narrower than a panel, and narrower
/// than play, whose `set_simulating` and `request_steps` have no business here.
pub trait UndoHost {
    /// The scene, which an applied edit spawns into, despawns from and writes.
    fn world(&mut self) -> &mut World;
    /// Forgets the accumulated impulses and the clock's banked time.
    ///
    /// The solver's contact cache is keyed by `Entity`, and an undone delete
    /// revives its entity under a new handle. Nothing is simulating while
    /// editing, so the cost is a warm start that was not being used.
    fn reset_physics(&mut self);
    /// Re-resolves every sprite's GPU handle from its path.
    ///
    /// A record carries the path and `Sprite::texture_handle` is
    /// `#[serde(skip)]`, so without this an undone texture edit draws flat
    /// white.
    fn resolve_scene_assets(&mut self);
}

impl UndoHost for UiFrame<'_> {
    fn world(&mut self) -> &mut World {
        self.world
    }

    fn reset_physics(&mut self) {
        UiFrame::reset_physics(self);
    }

    fn resolve_scene_assets(&mut self) {
        UiFrame::resolve_scene_assets(self);
    }
}

/// What the history reads when it captures a side of an edit.
pub struct SceneView<'a> {
    pub world: &'a World,
    /// Capture and apply must agree on which component types exist. The editor
    /// owns one registry and passes it here rather than building one per call,
    /// so that is guaranteed rather than remembered.
    pub registry: &'a ComponentRegistry,
    /// The selection by identity: the `Entity` will not survive an undo that
    /// respawns it.
    pub selected: Option<SceneId>,
}

/// The editor state an undo or a redo writes besides the world.
pub struct UndoContext<'a> {
    pub registry: &'a ComponentRegistry,
    pub selected: &'a mut Option<Entity>,
    /// Cancelled by every apply: a `Drag` holds an `Entity` and a grab offset,
    /// and an undo that respawns the entity leaves both stale.
    pub gizmo: &'a mut Gizmo,
}

/// The scene identity of `entity`, if it has one.
pub fn selected_id(world: &World, entity: Option<Entity>) -> Option<SceneId> {
    entity.and_then(|e| world.get::<SceneId>(e).copied())
}
