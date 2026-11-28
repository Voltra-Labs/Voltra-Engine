//! A 3D mesh: which one, its colour and its surface material.

use std::path::Path;

use voltra_assets::{AssetPath, BuiltinMesh, MeshPath};
use voltra_core::egui::{self, RichText, Ui};
use voltra_scene::MeshInstance3D;

use super::path_field::{self, PathEdit};

/// The mesh — a built-in one from the dropdown, or a file by path — its
/// colour, and its material: a `.material.ron` typed or dropped, or a new
/// one written under `root` and named here.
pub(super) fn show(
    ui: &mut Ui,
    instance: &mut MeshInstance3D,
    root: &Path,
) -> Option<&'static str> {
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
            let response = crate::panels::colour_edit::rgba(ui, &mut instance.color);
            if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                claim = claim.or(Some("Set colour"));
            }
            ui.end_row();

            ui.label("material")
                .on_hover_text("a surface .material.ron; none draws the default white standard");
            let committed = instance
                .material
                .as_ref()
                .map(|p| p.as_str().to_owned())
                .unwrap_or_default();
            match path_field::show(ui, "mesh_material", "materials/x.material.ron", &committed) {
                Some(PathEdit::Set(path)) => {
                    instance.material = Some(path);
                    claim = Some("Set material");
                }
                Some(PathEdit::Cleared) if instance.material.is_some() => {
                    instance.material = None;
                    claim = Some("Clear material");
                }
                _ => {}
            }
            ui.end_row();
        });
    if instance.material.is_none() && ui.small_button("New surface material").clicked() {
        match create(root) {
            Ok(path) => {
                log::info!("wrote {}", path.as_str());
                instance.material = Some(path);
                claim = Some("Set material");
            }
            Err(e) => log::error!("a new surface material: {e}"),
        }
    }
    claim
}

/// Writes a default surface material at the first free
/// `materials/surface[-N].material.ron` under `root`.
fn create(root: &Path) -> Result<AssetPath, String> {
    for n in 1..1000 {
        let name = if n == 1 {
            "materials/surface.material.ron".to_owned()
        } else {
            format!("materials/surface-{n}.material.ron")
        };
        let path = AssetPath::new(&name).map_err(|e| e.to_string())?;
        if !root.join(path.as_str()).exists() {
            crate::surface_doc::SurfaceDoc::create(root, &path)?;
            return Ok(path);
        }
    }
    Err("a thousand surface materials already; name one by hand".to_owned())
}
