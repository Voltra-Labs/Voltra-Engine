//! The sections of what an entity draws or lights in the 3D world: its
//! mesh and the mesh's material file, a directional, point or spot light,
//! an environment.

use voltra_core::egui::{self, Ui};
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_scene::{DirectionalLight3D, Environment3D, MeshInstance3D, PointLight3D, SpotLight3D};

use super::super::{
    directional_light3d, environment3d, mesh_instance3d, mixed, point_light3d, spot_light3d,
    surface_material,
};
use super::gate;
use crate::editor::Editor;
use crate::multi_edit::MultiEdit;

/// Every 3D drawing section `entity` has; what to call the edit one of
/// them is in the middle of, if any.
pub(super) fn show(
    editor: &mut Editor,
    ui: &mut Ui,
    frame: &mut UiFrame<'_>,
    entity: Entity,
    multi: &mut MultiEdit,
) -> Option<&'static str> {
    let mut claim = None;
    let root = frame.textures.root().to_path_buf();
    let mut material = None;
    if let Some(instance) = gate(
        frame.world.get_mut::<MeshInstance3D>(entity),
        multi,
        MeshInstance3D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[MeshInstance3D::NAME]);
        claim = claim.or(mesh_instance3d::show(ui, instance, &root));
        material.clone_from(&instance.material);
    }
    // The material's file, once the component is let go: its fields edit
    // the file every mesh naming it shares, and the preview draws through
    // the frame.
    if let Some(path) = &material {
        egui::CollapsingHeader::new("surface material")
            .default_open(true)
            .show(ui, |ui| {
                surface_material::show(ui, frame, &mut editor.surface_doc, path);
            });
    }
    if let Some(light) = gate(
        frame.world.get_mut::<DirectionalLight3D>(entity),
        multi,
        DirectionalLight3D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[DirectionalLight3D::NAME]);
        claim = claim.or(directional_light3d::show(ui, light));
    }
    if let Some(light) = gate(
        frame.world.get_mut::<PointLight3D>(entity),
        multi,
        PointLight3D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[PointLight3D::NAME]);
        claim = claim.or(point_light3d::show(ui, light));
    }
    if let Some(light) = gate(
        frame.world.get_mut::<SpotLight3D>(entity),
        multi,
        SpotLight3D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[SpotLight3D::NAME]);
        claim = claim.or(spot_light3d::show(ui, light));
    }
    if let Some(environment) = gate(
        frame.world.get_mut::<Environment3D>(entity),
        multi,
        Environment3D::NAME,
    ) {
        ui.separator();
        mixed::marker(ui, multi, &[Environment3D::NAME]);
        claim = claim.or(environment3d::show(ui, environment));
    }
    claim
}
