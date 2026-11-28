//! A 3D mesh: which one, and its colour.

use voltra_assets::{BuiltinMesh, MeshPath};
use voltra_core::egui::{self, RichText, Ui};
use voltra_scene::MeshInstance3D;

use super::path_field::{self, PathEdit};

/// The mesh — a built-in one from the dropdown, or a file by path — and
/// its colour.
pub(super) fn show(ui: &mut Ui, instance: &mut MeshInstance3D) -> Option<&'static str> {
    ui.label(RichText::new(MeshInstance3D::NAME).strong())
        .on_hover_text("drawn at the entity's Transform3D; every copy of a mesh is one draw");

    let mut claim = None;
    egui::Grid::new("mesh_instance3d")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("mesh");
            let shown = match &instance.mesh {
                MeshPath::Builtin(mesh) => mesh.name().to_owned(),
                MeshPath::File(_) => "file".to_owned(),
            };
            egui::ComboBox::from_id_salt("mesh_instance3d_mesh")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for mesh in BuiltinMesh::ALL {
                        let chosen = instance.mesh == MeshPath::Builtin(mesh);
                        if ui.selectable_label(chosen, mesh.name()).clicked() && !chosen {
                            instance.mesh = MeshPath::Builtin(mesh);
                            claim = Some("Set mesh");
                        }
                    }
                });
            ui.end_row();

            ui.label("file").on_hover_text(
                "a mesh file under the asset root; one that will not load draws a magenta cube",
            );
            let committed = match &instance.mesh {
                MeshPath::File(path) => path.as_str().to_owned(),
                MeshPath::Builtin(_) => String::new(),
            };
            match path_field::show(ui, "mesh_file", "models/thing.glb", &committed) {
                Some(PathEdit::Set(path)) => {
                    instance.mesh = MeshPath::File(path);
                    claim = Some("Set mesh");
                }
                Some(PathEdit::Cleared) if matches!(instance.mesh, MeshPath::File(_)) => {
                    instance.mesh = MeshPath::default();
                    claim = Some("Set mesh");
                }
                _ => {}
            }
            ui.end_row();

            ui.label("colour");
            // A popup, as every colour field is: see `sprite::show`.
            let popup = ui.auto_id_with("popup");
            let response = ui.color_edit_button_rgba_unmultiplied(&mut instance.color);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Set colour"));
            }
            ui.end_row();
        });
    claim
}
