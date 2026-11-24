//! A layer's terrains: which frames join up, and which one the brush paints.

use voltra_core::egui::{self, DragValue, RichText, TextEdit, Ui};
use voltra_scene::tilemap::terrain::{
    blob_masks, BLOB_FRAMES, EAST, NORTH, NORTH_EAST, NORTH_WEST, SOUTH, SOUTH_EAST, SOUTH_WEST,
    WEST,
};
use voltra_scene::tilemap::Terrain;
use voltra_scene::Tilemap;

use super::super::active;
use crate::paint::Paint;

/// Every terrain of `map`: its name, its sixteen frames by neighbourhood,
/// and a toggle to paint with it.
pub(super) fn show(ui: &mut Ui, map: &mut Tilemap, paint: &mut Paint) -> Option<&'static str> {
    ui.label(RichText::new("Terrains").strong())
        .on_hover_text("paint with a terrain and each cell picks its frame from its neighbours");
    let mut claim = None;
    let mut remove = None;
    for (index, terrain) in map.terrains.iter_mut().enumerate() {
        ui.push_id(index, |ui| {
            ui.horizontal(|ui| {
                let painting = paint.terrain == Some(index);
                if ui.selectable_label(painting, "paint").clicked() {
                    // Editor state, not a scene edit: no history entry.
                    paint.terrain = if painting { None } else { Some(index) };
                }
                claim = claim.or(active(
                    &ui.add(TextEdit::singleline(&mut terrain.name).desired_width(100.0)),
                    "Rename terrain",
                ));
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
            });
            let mut blob = terrain.is_blob();
            if ui
                .checkbox(&mut blob, "47-tile (corners too)")
                .on_hover_text("the blob set: inner corners join as well as sides")
                .changed()
            {
                terrain.blob = if blob {
                    vec![terrain.frames[0]; BLOB_FRAMES]
                } else {
                    Vec::new()
                };
                claim = Some("Edit terrain");
            }
            if terrain.is_blob() {
                egui::Grid::new("terrain_blob")
                    .num_columns(8)
                    .show(ui, |ui| {
                        for (i, (frame, mask)) in
                            terrain.blob.iter_mut().zip(blob_masks()).enumerate()
                        {
                            claim = claim.or(active(
                                &ui.add(DragValue::new(frame)).on_hover_text(sides(*mask)),
                                "Edit terrain",
                            ));
                            if i % 8 == 7 {
                                ui.end_row();
                            }
                        }
                    });
                return;
            }
            // The sixteen frames, by which sides touch the same terrain.
            egui::Grid::new("terrain_frames")
                .num_columns(4)
                .show(ui, |ui| {
                    for (mask, frame) in terrain.frames.iter_mut().enumerate() {
                        claim = claim.or(active(
                            &ui.add(DragValue::new(frame)).on_hover_text(sides(mask)),
                            "Edit terrain",
                        ));
                        if mask % 4 == 3 {
                            ui.end_row();
                        }
                    }
                });
        });
    }
    if let Some(index) = remove {
        map.terrains.remove(index);
        // The choice pointed at an index that moved or went away.
        paint.terrain = None;
        claim = Some("Remove terrain");
    }
    if ui.button("Add terrain").clicked() {
        let n = map.terrains.len() + 1;
        map.terrains.push(Terrain::uniform(
            format!("Terrain {n}"),
            paint.stamp.first().frame,
        ));
        claim = Some("Add terrain");
    }
    claim
}

/// The sides a mask has neighbours on, for a frame's tooltip.
fn sides(mask: usize) -> String {
    let named: Vec<&str> = [
        (NORTH, "N"),
        (EAST, "E"),
        (SOUTH, "S"),
        (WEST, "W"),
        (NORTH_EAST, "NE"),
        (SOUTH_EAST, "SE"),
        (SOUTH_WEST, "SW"),
        (NORTH_WEST, "NW"),
    ]
    .into_iter()
    .filter(|(bit, _)| mask & bit != 0)
    .map(|(_, name)| name)
    .collect();
    if named.is_empty() {
        "alone".to_owned()
    } else {
        format!("touching {}", named.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::sides;

    #[test]
    fn a_mask_names_its_sides() {
        assert_eq!(sides(0), "alone");
        assert_eq!(sides(1 | 4), "touching N S");
        assert_eq!(sides(15), "touching N E S W");
        assert_eq!(sides(255), "touching N E S W NE SE SW NW");
    }
}
