//! The 3D camera: how it projects, its place in the frame's order, and
//! how its light becomes its picture.

use voltra_core::egui::{self, Color32, DragValue, RichText, Ui};
use voltra_render::render3d::Tonemap;
use voltra_scene::{Camera3D, CameraClear, Projection3D};

use super::{active, post_fields};

/// The kinds of projection, as the dropdown names them.
const KINDS: [&str; 2] = ["Perspective", "Orthographic"];

/// The camera's projection, priority, active flag, clear colour, tone
/// curve and grade.
///
/// Switching the kind keeps the near plane and starts the other kind's
/// own fields at their defaults. A projection that cannot be drawn as
/// written says why under the fields, in the words the log uses.
pub(super) fn show(ui: &mut Ui, camera: &mut Camera3D) -> Option<&'static str> {
    ui.label(RichText::new(Camera3D::NAME).strong())
        .on_hover_text("looks down the entity's −Z; every active camera draws, by priority");

    let mut claim = None;
    let kind = match camera.projection {
        Projection3D::Perspective { .. } => KINDS[0],
        Projection3D::Orthographic { .. } => KINDS[1],
    };
    let mut picked = kind;
    egui::ComboBox::from_id_salt("camera3d_projection")
        .selected_text(kind)
        .show_ui(ui, |ui| {
            for option in KINDS {
                ui.selectable_value(&mut picked, option, option);
            }
        });
    if picked != kind {
        camera.projection = switched(camera.projection);
        claim = Some("Change camera projection");
    }

    egui::Grid::new("camera3d").num_columns(2).show(ui, |ui| {
        match &mut camera.projection {
            Projection3D::Perspective {
                fov_y_degrees,
                near,
                far,
            } => {
                ui.label("field of view")
                    .on_hover_text("the view's height as an angle, in degrees");
                let drag = DragValue::new(fov_y_degrees)
                    .speed(0.2)
                    .range(1.0..=179.0)
                    .suffix("°");
                claim = claim.or(active(&ui.add(drag), "Set field of view"));
                ui.end_row();
                claim = claim.or(near_row(ui, near, 0.001));
                ui.label("far")
                    .on_hover_text("off: no far plane, everything ahead is drawn");
                ui.horizontal(|ui| {
                    let mut finite = far.is_some();
                    if ui.checkbox(&mut finite, "").changed() {
                        *far = finite.then_some(Projection3D::DEFAULT_ORTHOGRAPHIC_FAR);
                        claim = claim.or(Some("Set far plane"));
                    }
                    if let Some(far) = far {
                        let drag = DragValue::new(far).speed(0.5).suffix(" m");
                        claim = claim.or(active(&ui.add(drag), "Set far plane"));
                    }
                });
                ui.end_row();
            }
            Projection3D::Orthographic { size, near, far } => {
                ui.label("size")
                    .on_hover_text("half the height it shows, in metres");
                let drag = DragValue::new(size).speed(0.05).suffix(" m");
                claim = claim.or(active(&ui.add(drag), "Set camera size"));
                ui.end_row();
                claim = claim.or(near_row(ui, near, 0.0));
                ui.label("far");
                let drag = DragValue::new(far).speed(0.5).suffix(" m");
                claim = claim.or(active(&ui.add(drag), "Set far plane"));
                ui.end_row();
            }
        }

        ui.label("priority").on_hover_text(
            "cameras draw lowest first, a later one over an earlier; at a tie 3D draws before 2D",
        );
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut camera.priority)),
            "Set camera priority",
        ));
        ui.end_row();

        ui.label("active");
        if ui.checkbox(&mut camera.active, "").changed() {
            claim = claim.or(Some("Toggle camera"));
        }
        ui.end_row();

        ui.label("clear").on_hover_text(
            "what the picture starts as: the background — transparent over a 2D camera that \
             drew first — or a colour of its own, opaque",
        );
        ui.horizontal(|ui| {
            let mut own = matches!(camera.clear, CameraClear::Color(_));
            if ui.checkbox(&mut own, "").changed() {
                camera.clear = if own {
                    CameraClear::Color([0.1, 0.1, 0.12, 1.0])
                } else {
                    CameraClear::Background
                };
                claim = claim.or(Some("Set camera clear"));
            }
            if let CameraClear::Color(colour) = &mut camera.clear {
                // A popup, as every colour field is: see `sprite::show`.
                let popup = ui.auto_id_with("popup");
                let response = ui.color_edit_button_rgba_unmultiplied(colour);
                if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
                    claim = claim.or(Some("Set camera clear"));
                }
            }
        });
        ui.end_row();

        ui.label("tone curve")
            .on_hover_text("how light past 1 is brought into the screen's range; AgX by default");
        egui::ComboBox::from_id_salt("camera3d_tonemap")
            .selected_text(format!("{:?}", camera.tonemap))
            .show_ui(ui, |ui| {
                for curve in Tonemap::ALL {
                    if ui
                        .selectable_value(&mut camera.tonemap, curve, format!("{curve:?}"))
                        .changed()
                    {
                        claim = claim.or(Some("Set tone curve"));
                    }
                }
            });
        ui.end_row();
    });
    claim = claim.or(post_fields::show(ui, "camera3d_post", &mut camera.post));
    if let Some(problem) = camera.projection.problem() {
        ui.label(RichText::new(problem).color(Color32::from_rgb(230, 120, 80)));
    }
    claim
}

fn near_row(ui: &mut Ui, near: &mut f32, least: f32) -> Option<&'static str> {
    ui.label("near")
        .on_hover_text("the nearest distance drawn, in metres; raise it for far scenes on WebGL2");
    let drag = DragValue::new(near)
        .speed(0.01)
        .range(least..=f32::MAX)
        .suffix(" m");
    let claim = active(&ui.add(drag), "Set near plane");
    ui.end_row();
    claim
}

/// `projection` as the other kind, keeping its near plane.
fn switched(projection: Projection3D) -> Projection3D {
    match projection {
        Projection3D::Perspective { near, .. } => Projection3D::Orthographic {
            size: Projection3D::DEFAULT_ORTHOGRAPHIC_SIZE,
            near,
            far: Projection3D::DEFAULT_ORTHOGRAPHIC_FAR,
        },
        Projection3D::Orthographic { near, .. } => Projection3D::Perspective {
            fov_y_degrees: Projection3D::DEFAULT_FOV_Y_DEGREES,
            near: near.max(voltra_render::render3d::View3D::DEFAULT_NEAR),
            far: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switching_the_projection_keeps_the_near_plane_and_round_trips() {
        let perspective = Projection3D::Perspective {
            fov_y_degrees: 45.0,
            near: 0.5,
            far: Some(80.0),
        };
        let ortho = switched(perspective);
        assert!(matches!(ortho, Projection3D::Orthographic { near, .. } if near == 0.5));
        assert!(ortho.problem().is_none());
        let back = switched(ortho);
        assert!(matches!(back, Projection3D::Perspective { near, far: None, .. } if near == 0.5));
        // An orthographic near of 0 is no perspective near plane.
        let touching = Projection3D::Orthographic {
            size: 3.0,
            near: 0.0,
            far: 10.0,
        };
        assert!(touching.problem().is_none());
        assert!(switched(touching).problem().is_none());
    }
}
