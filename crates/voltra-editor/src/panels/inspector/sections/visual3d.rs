//! The sections of what an entity draws in the 3D world: its mesh.

use voltra_core::egui::Ui;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::MeshInstance3D;

use super::super::mesh_instance3d;
use super::super::mixed;
use super::gate;
use crate::multi_edit::MultiEdit;

/// Every 3D drawing section `entity` has; what to call the edit one of
/// them is in the middle of, if any.
pub(super) fn show(
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    entity: Entity,
    multi: &mut MultiEdit,
) -> Option<&'static str> {
    let mut claim = None;
    if let Some(instance) = gate(
        frame.world.get_mut::<MeshInstance3D>(entity),
        multi,
        MeshInstance3D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[MeshInstance3D::NAME]);
        claim = claim.or(mesh_instance3d::show(ui, instance));
    }
    claim
}
