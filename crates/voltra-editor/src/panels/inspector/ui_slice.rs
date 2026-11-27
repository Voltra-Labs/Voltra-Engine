//! A UI image's 9-slice borders, or the button that adds them.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::ui::Edges;
use voltra_scene::{Sprite2D, UiRect, UiSlice};

use super::active;

/// The 9-slice section of a UI element with a sprite: its borders, or a
/// button to slice it. Nothing for anything else.
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let image = world.get::<UiRect>(entity).is_some() && world.get::<Sprite2D>(entity).is_some();
    let Some(slice) = world.get_mut::<UiSlice>(entity) else {
        if image && ui.button("Add 9-slice").clicked() {
            world.insert(entity, UiSlice::default());
            return Some("Add 9-slice");
        }
        return None;
    };
    ui.separator();
    ui.label(RichText::new("9-slice").strong())
        .on_hover_text("corners keep their size, edges and middle stretch");
    let mut claim = None;
    egui::Grid::new("ui_slice").num_columns(2).show(ui, |ui| {
        claim = edges(ui, "borders", &mut slice.borders, "texels of the frame");
        ui.label("scale")
            .on_hover_text("points per texel of border");
        claim = claim.or(active(
            &ui.add(
                DragValue::new(&mut slice.scale)
                    .speed(0.05)
                    .range(0.0..=64.0),
            ),
            "Edit 9-slice",
        ));
        ui.end_row();
        ui.label("middle");
        if ui.checkbox(&mut slice.center, "").changed() {
            claim = claim.or(Some("Edit 9-slice"));
        }
        ui.end_row();
    });
    if ui.small_button("Remove 9-slice").clicked() {
        world.remove::<UiSlice>(entity);
        claim = Some("Remove 9-slice");
    }
    claim
}

/// One row per side of `edges`, none below zero.
pub(super) fn edges(
    ui: &mut Ui,
    label: &str,
    edges: &mut Edges,
    hint: &str,
) -> Option<&'static str> {
    let mut claim = None;
    for (side, value) in [
        ("left", &mut edges.left),
        ("top", &mut edges.top),
        ("right", &mut edges.right),
        ("bottom", &mut edges.bottom),
    ] {
        ui.label(format!("{label} {side}")).on_hover_text(hint);
        let drag = DragValue::new(value).speed(0.5).range(0.0..=f32::MAX);
        claim = claim.or(active(&ui.add(drag), "Edit edges"));
        ui.end_row();
    }
    claim
}
