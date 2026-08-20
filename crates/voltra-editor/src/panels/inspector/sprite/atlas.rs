//! The sprite's sheet: which slicing, which frame, mirrored, and how big.

use voltra_assets::{AssetPath, Atlases};
use voltra_core::egui::{self, Color32, DragValue, Rect, RichText, Ui};
use voltra_core::ui::TextureId;
use voltra_scene::Sprite;

use super::super::active;
use super::super::path_field::{self, PathEdit};

/// Side of the frame preview, in points.
const PREVIEW: f32 = 64.0;

/// What a sprite that has never been given a rate starts at when one is turned
/// on.
///
/// Sixteen: the pixel-art default in Unity's importer and in every tilemap
/// tutorial written since, and the cell size the engine's own sheets use.
const DEFAULT_PIXELS_PER_UNIT: f32 = 16.0;

pub(super) fn show(
    ui: &mut Ui,
    sprite: &mut Sprite,
    atlases: &mut Atlases,
    preview: Option<TextureId>,
) -> Option<&'static str> {
    ui.label(RichText::new("Atlas").strong());

    let committed = sprite
        .atlas
        .as_ref()
        .map(AssetPath::as_str)
        .unwrap_or("")
        .to_owned();

    let mut claim = match path_field::show(ui, "atlas_path", "path/to/sheet.atlas.ron", &committed)
    {
        Some(PathEdit::Cleared) => {
            sprite.set_atlas(None, atlases);
            Some("Set atlas")
        }
        Some(PathEdit::Set(path)) => {
            sprite.set_atlas(Some(path), atlases);
            Some("Set atlas")
        }
        None => None,
    };

    // Read after the assignment above, so a sheet dropped this frame shows its
    // own frame count rather than the previous one's.
    let atlas = sprite
        .atlas_handle
        .and_then(|handle| atlases.try_get(handle));
    let frames = atlas.map(|atlas| atlas.len()).unwrap_or(0);

    egui::Grid::new("atlas").num_columns(2).show(ui, |ui| {
        ui.label("frame");
        ui.horizontal(|ui| {
            // Bounded by the sheet: a `DragValue` that runs past the last frame
            // draws the placeholder, and an author dragging a slider off the
            // end of their own sheet is not asking for that.
            let last = frames.saturating_sub(1) as u32;
            let response = ui.add_enabled(
                frames > 0,
                DragValue::new(&mut sprite.frame).range(0..=last),
            );
            claim = claim.or(active(&response, "Set frame"));
            ui.label(RichText::new(format!("of {frames}")).weak());
        });
        ui.end_row();

        ui.label("flip");
        ui.horizontal(|ui| {
            if ui.checkbox(&mut sprite.flip_x, "x").changed() {
                claim = claim.or(Some("Flip sprite"));
            }
            if ui.checkbox(&mut sprite.flip_y, "y").changed() {
                claim = claim.or(Some("Flip sprite"));
            }
        });
        ui.end_row();

        ui.label("pixels per unit");
        ui.horizontal(|ui| {
            // A checkbox rather than a sentinel value: `None` and `Some(0.0)`
            // draw the same unit quad, and a field that means "off" at zero
            // makes an author type a magic number to get the default back.
            let mut sized = sprite.pixels_per_unit.is_some();
            if ui.checkbox(&mut sized, "").changed() {
                sprite.pixels_per_unit = sized.then_some(DEFAULT_PIXELS_PER_UNIT);
                claim = claim.or(Some("Set pixels per unit"));
            }
            if let Some(rate) = sprite.pixels_per_unit.as_mut() {
                let response = ui.add(DragValue::new(rate).range(1.0..=f32::MAX).speed(1.0));
                claim = claim.or(active(&response, "Set pixels per unit"));
            } else {
                ui.label(RichText::new("from the transform").weak());
            }
        });
        ui.end_row();
    });

    // The frame as it will actually be drawn, mirrors included. A number and a
    // count say which cell is selected; only the picture says whether it is
    // the right one.
    if let (Some(atlas), Some(id)) = (atlas, preview) {
        if let Some(frame) = atlas.frame(sprite.frame) {
            if let Some((min, max)) = frame.uv(atlas.sheet()) {
                let (rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(PREVIEW), egui::Sense::hover());
                let (u0, u1) = mirrored(sprite.flip_x, min[0], max[0]);
                let (v0, v1) = mirrored(sprite.flip_y, min[1], max[1]);
                ui.painter().image(
                    id,
                    rect,
                    Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1)),
                    Color32::WHITE,
                );
            }
        }
    }

    claim
}

/// The pair, swapped when the sprite is mirrored on that axis — the same rule
/// `voltra_scene::sprite::quad` draws by, so the preview is the sprite.
fn mirrored(flip: bool, min: f32, max: f32) -> (f32, f32) {
    if flip {
        (max, min)
    } else {
        (min, max)
    }
}
