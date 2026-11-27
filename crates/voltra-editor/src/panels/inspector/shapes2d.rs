//! A `Line2D`'s, a `Polygon2D`'s and a `Trail2D`'s fields: their points
//! (edited in the viewport, with the shadow caster's handles), widths,
//! joints, caps, colours and textures.

use voltra_assets::{AssetPath, Textures};
use voltra_core::egui::{self, Button, DragValue, RichText, Ui};
use voltra_ecs::Entity;
use voltra_render::wgpu;
use voltra_scene::{Curve, Line2D, LineCap, LineJoint, LineTexture, Polygon2D, Trail2D};

use super::active;
use super::mixed;
use super::path_field::{self, PathEdit};
use crate::shape_edit::{Points, ShapeEdit};
use voltra_scene::format::serial_tree::Step;

/// What the three need besides themselves.
pub(super) struct Context<'a> {
    pub entity: Entity,
    pub shape: &'a mut ShapeEdit,
    pub textures: &'a mut Textures,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
}

pub(super) fn line(ui: &mut Ui, line: &mut Line2D, cx: Context<'_>) -> Option<&'static str> {
    ui.label(RichText::new("Line").strong());
    let mut claim = points(ui, cx.shape, cx.entity, Points::Line, line.points.len());
    if ui.checkbox(&mut line.closed, "closed").changed() {
        claim = claim.or(Some("Edit line"));
    }
    egui::Grid::new("line2d").num_columns(2).show(ui, |ui| {
        ui.label("width");
        claim = claim.or(drag(ui, &mut line.width, "Edit line"));
        ui.end_row();
        ui.label("joints");
        claim = claim.or(pick(
            ui,
            "joint",
            &mut line.joint,
            &[
                (LineJoint::Miter, "miter"),
                (LineJoint::Bevel, "bevel"),
                (LineJoint::Round, "round"),
            ],
        ));
        ui.end_row();
        if line.joint == LineJoint::Miter {
            ui.label("miter limit");
            claim = claim.or(drag(ui, &mut line.miter_limit, "Edit line"));
            ui.end_row();
        }
        let caps = [
            (LineCap::Butt, "butt"),
            (LineCap::Square, "square"),
            (LineCap::Round, "round"),
        ];
        ui.label("begin cap");
        claim = claim.or(pick(ui, "begin_cap", &mut line.begin_cap, &caps));
        ui.end_row();
        ui.label("end cap");
        claim = claim.or(pick(ui, "end_cap", &mut line.end_cap, &caps));
        ui.end_row();
        ui.label("texture");
        claim = claim.or(pick(
            ui,
            "texture_mode",
            &mut line.texture_mode,
            &[
                (LineTexture::Stretch, "stretched"),
                (LineTexture::Tile, "tiled"),
            ],
        ));
        ui.end_row();
    });
    claim = claim.or(colour(ui, &mut line.color, "Edit line"));
    let mut tapered = line.width_curve.is_some();
    if ui
        .checkbox(&mut tapered, "width along the line")
        .on_hover_text("multiplies the width, first point to last")
        .changed()
    {
        line.width_curve = tapered.then(|| Curve::linear(1.0, 1.0));
        claim = claim.or(Some("Edit curve"));
    }
    if let Some(curve) = &mut line.width_curve {
        claim = claim.or(super::particles::curve(ui, "line_width_curve", curve, 1.0));
    }
    let path = texture(ui, "line_texture", line.texture.as_ref());
    if let Some(path) = path {
        line.set_texture(path, cx.textures, cx.device, cx.queue);
        claim = claim.or(Some("Set texture"));
    }
    claim
}

pub(super) fn polygon(
    ui: &mut Ui,
    polygon: &mut Polygon2D,
    cx: Context<'_>,
) -> Option<&'static str> {
    ui.label(RichText::new("Polygon").strong());
    let mut claim = points(
        ui,
        cx.shape,
        cx.entity,
        Points::Polygon,
        polygon.points.len(),
    );
    claim = claim.or(colour(ui, &mut polygon.color, "Edit polygon"));
    if let Some(path) = texture(ui, "polygon_texture", polygon.texture.as_ref()) {
        polygon.set_texture(path, cx.textures, cx.device, cx.queue);
        claim = claim.or(Some("Set texture"));
    }
    claim
}

pub(super) fn trail(ui: &mut Ui, trail: &mut Trail2D, cx: Context<'_>) -> Option<&'static str> {
    ui.label(RichText::new("Trail").strong());
    let mut claim = None;
    egui::Grid::new("trail2d").num_columns(2).show(ui, |ui| {
        ui.label("lifetime s");
        claim = claim.or(drag(ui, &mut trail.lifetime, "Edit trail"));
        ui.end_row();
        ui.label("width");
        claim = claim.or(drag(ui, &mut trail.width, "Edit trail"));
        ui.end_row();
        ui.label("min distance");
        claim = claim.or(drag(ui, &mut trail.min_distance, "Edit trail"));
        ui.end_row();
        ui.label("max points");
        claim = claim.or(active(
            &ui.add(DragValue::new(&mut trail.max_points).range(1..=4096)),
            "Edit trail",
        ));
        ui.end_row();
    });
    claim = claim.or(colour(ui, &mut trail.color, "Edit trail"));
    ui.label(RichText::new("fades to clear and tapers as it ages").weak());
    if let Some(path) = texture(ui, "trail_texture", trail.texture.as_ref()) {
        trail.set_texture(path, cx.textures, cx.device, cx.queue);
        claim = claim.or(Some("Set texture"));
    }
    claim
}

/// The "Edit points" toggle for `entity`'s list `which`.
fn points(
    ui: &mut Ui,
    shape: &mut ShapeEdit,
    entity: Entity,
    which: Points,
    count: usize,
) -> Option<&'static str> {
    let editing = shape.is_editing_points(entity, which);
    ui.horizontal(|ui| {
        if ui
            .add(Button::selectable(editing, "Edit points"))
            .on_hover_text(
                "drag a point to move it, an edge's middle to add one; right-click removes one",
            )
            .clicked()
        {
            shape.set_editing_points((!editing).then_some((entity, which)));
        }
        ui.label(RichText::new(format!("{count} points")).weak());
    });
    None
}

fn drag(ui: &mut Ui, value: &mut f32, label: &'static str) -> Option<&'static str> {
    active(
        &ui.add(DragValue::new(value).speed(0.01).range(0.0..=1000.0)),
        label,
    )
}

fn colour(ui: &mut Ui, color: &mut [f32; 4], label: &'static str) -> Option<&'static str> {
    let mut claim = None;
    ui.horizontal(|ui| {
        ui.label("colour");
        let popup = ui.auto_id_with("popup");
        let response = ui.color_edit_button_rgba_unmultiplied(color);
        if egui::Popup::is_id_open(ui.ctx(), popup) || response.changed() {
            claim = Some(label);
        }
    });
    claim
}

/// A dropdown for the line's field named `field`.
fn pick<T: PartialEq + Copy>(
    ui: &mut Ui,
    field: &'static str,
    value: &mut T,
    options: &[(T, &str)],
) -> Option<&'static str> {
    let current = options
        .iter()
        .find(|(option, _)| option == value)
        .map_or("?", |(_, name)| *name);
    let mut claim = None;
    egui::ComboBox::from_id_salt(("line", field))
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (option, name) in options {
                let response = ui.selectable_label(option == value, *name);
                let path = vec![Step::Field(field)];
                if mixed::chosen(ui, response, "Line2D", path).clicked() && option != value {
                    *value = *option;
                    claim = Some("Edit line");
                }
            }
        });
    claim
}

/// A texture path field; `Some` with the new path (or `None` to clear)
/// when it changed.
fn texture(
    ui: &mut Ui,
    salt: &'static str,
    committed: Option<&AssetPath>,
) -> Option<Option<AssetPath>> {
    ui.label("texture");
    let committed = committed.map(AssetPath::as_str).unwrap_or("").to_owned();
    Some(
        match path_field::show(ui, salt, "path/to/texture.png", &committed)? {
            PathEdit::Cleared => None,
            PathEdit::Set(path) => Some(path),
        },
    )
}
