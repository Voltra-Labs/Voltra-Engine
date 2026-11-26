//! The picked tile's frame as the tileset sees it: its collision shape and
//! its custom values, edited for every tile of that frame on the layer.

use voltra_core::egui::{self, DragValue, RichText, TextEdit, Ui};
use voltra_render::glam::Vec2;
use voltra_scene::tilemap::{Side, TileData, TileShape, TileValue};
use voltra_scene::Tilemap;

use super::super::active;
use crate::paint::Paint;

/// The shapes a tile can be given, as the combo box lists them.
const SHAPES: [&str; 5] = ["Full", "None", "Half", "Slope", "Polygon"];

fn shape_name(shape: &TileShape) -> &'static str {
    match shape {
        TileShape::Full => SHAPES[0],
        TileShape::Empty => SHAPES[1],
        TileShape::Half(_) => SHAPES[2],
        TileShape::Slope { .. } => SHAPES[3],
        TileShape::Polygon(_) => SHAPES[4],
    }
}

/// A fresh shape of the kind named, sensible to start editing from.
fn shape_named(name: &str) -> TileShape {
    match name {
        "None" => TileShape::Empty,
        "Half" => TileShape::Half(Side::Bottom),
        "Slope" => TileShape::Slope {
            left: 0.0,
            right: 1.0,
        },
        "Polygon" => TileShape::Polygon(vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.5, 1.0),
        ]),
        _ => TileShape::Full,
    }
}

/// The frame of the stamp's first tile: its shape and its custom values.
pub(super) fn show(ui: &mut Ui, map: &mut Tilemap, paint: &Paint) -> Option<&'static str> {
    let frame = paint.stamp.first().frame;
    ui.label(RichText::new(format!("Tile {frame}")).strong())
        .on_hover_text("the picked tile's frame: its collision shape and values, for every tile of it on this layer");
    let mut claim = None;
    let data = map.tile_data.entry(frame).or_default();

    let mut picked = None;
    egui::ComboBox::from_id_salt("tile_shape")
        .selected_text(shape_name(&data.shape))
        .show_ui(ui, |ui| {
            for name in SHAPES {
                if ui
                    .selectable_label(shape_name(&data.shape) == name, name)
                    .clicked()
                {
                    picked = Some(name);
                }
            }
        });
    if let Some(name) = picked.filter(|&name| name != shape_name(&data.shape)) {
        data.shape = shape_named(name);
        claim = Some("Set tile shape");
    }
    claim = claim.or(shape_fields(ui, &mut data.shape));

    claim = claim.or(custom_values(ui, data));
    // A frame left plain is not written: the layer saves as it did.
    if map.tile_data.get(&frame) == Some(&TileData::default()) {
        map.tile_data.remove(&frame);
    }
    claim
}

fn shape_fields(ui: &mut Ui, shape: &mut TileShape) -> Option<&'static str> {
    let mut claim = None;
    match shape {
        TileShape::Half(side) => {
            ui.horizontal(|ui| {
                for (value, label) in [
                    (Side::Bottom, "bottom"),
                    (Side::Top, "top"),
                    (Side::Left, "left"),
                    (Side::Right, "right"),
                ] {
                    if ui.selectable_label(*side == value, label).clicked() && *side != value {
                        *side = value;
                        claim = Some("Set tile shape");
                    }
                }
            });
        }
        TileShape::Slope { left, right } => {
            ui.horizontal(|ui| {
                ui.label("height at left");
                let l = ui.add(DragValue::new(left).range(0.0..=1.0).speed(0.05));
                ui.label("right");
                let r = ui.add(DragValue::new(right).range(0.0..=1.0).speed(0.05));
                claim = active(&l, "Set tile shape").or(active(&r, "Set tile shape"));
            });
        }
        TileShape::Polygon(points) => {
            let mut remove = None;
            for (i, point) in points.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    let x = ui.add(DragValue::new(&mut point.x).range(0.0..=1.0).speed(0.05));
                    let y = ui.add(DragValue::new(&mut point.y).range(0.0..=1.0).speed(0.05));
                    claim = claim
                        .or(active(&x, "Set tile shape"))
                        .or(active(&y, "Set tile shape"));
                    if ui.small_button(crate::icons::REMOVE).clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove.filter(|_| points.len() > 3) {
                points.remove(i);
                claim = Some("Set tile shape");
            }
            if points.len() < 8 && ui.small_button("Add point").clicked() {
                points.push(Vec2::splat(0.5));
                claim = Some("Set tile shape");
            }
            ui.label(
                RichText::new("x right, y up, 0..1 of the cell; collides as its convex hull")
                    .weak(),
            );
        }
        TileShape::Full | TileShape::Empty => {}
    }
    claim
}

fn custom_values(ui: &mut Ui, data: &mut TileData) -> Option<&'static str> {
    ui.label("Custom values")
        .on_hover_text("read by a game with Tilemap::custom_at or tilemap::query::custom_under");
    let mut claim = None;
    let mut remove = None;
    for (key, value) in data.custom.iter_mut() {
        ui.push_id(key, |ui| {
            ui.horizontal(|ui| {
                ui.label(key);
                claim = claim.or(value_field(ui, value));
                if ui.small_button(crate::icons::REMOVE).clicked() {
                    remove = Some(key.clone());
                }
            });
        });
    }
    if let Some(key) = remove {
        data.custom.remove(&key);
        claim = Some("Edit tile values");
    }
    let id = ui.id().with("new_tile_value");
    let mut name: String = ui.data_mut(|d| d.get_temp(id).unwrap_or_default());
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(&mut name)
                .hint_text("name")
                .desired_width(90.0),
        );
        let free = !name.trim().is_empty() && !data.custom.contains_key(name.trim());
        if ui.add_enabled(free, egui::Button::new("Add")).clicked() {
            data.custom
                .insert(name.trim().to_owned(), TileValue::Bool(true));
            name.clear();
            claim = Some("Edit tile values");
        }
    });
    ui.data_mut(|d| d.insert_temp(id, name));
    claim
}

fn value_field(ui: &mut Ui, value: &mut TileValue) -> Option<&'static str> {
    let label = "Edit tile values";
    let kind = match value {
        TileValue::Bool(_) => "bool",
        TileValue::Int(_) => "int",
        TileValue::Float(_) => "float",
        TileValue::Text(_) => "text",
    };
    let mut claim = None;
    egui::ComboBox::from_id_salt("kind")
        .width(56.0)
        .selected_text(kind)
        .show_ui(ui, |ui| {
            for (name, fresh) in [
                ("bool", TileValue::Bool(true)),
                ("int", TileValue::Int(0)),
                ("float", TileValue::Float(0.0)),
                ("text", TileValue::Text(String::new())),
            ] {
                if ui.selectable_label(kind == name, name).clicked() && kind != name {
                    *value = fresh;
                    claim = Some(label);
                }
            }
        });
    let response = match value {
        TileValue::Bool(v) => ui.checkbox(v, ""),
        TileValue::Int(v) => ui.add(DragValue::new(v)),
        TileValue::Float(v) => ui.add(DragValue::new(v).speed(0.05)),
        TileValue::Text(v) => ui.add(TextEdit::singleline(v).desired_width(90.0)),
    };
    if response.changed() && matches!(value, TileValue::Bool(_)) {
        claim = Some(label);
    }
    claim.or(active(&response, label))
}
