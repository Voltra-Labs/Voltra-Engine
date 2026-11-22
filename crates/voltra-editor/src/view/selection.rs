//! The outline of every selected entity, drawn in the scene view.
//!
//! The gizmo sits on the primary only; with several entities selected, the
//! rest would be selected invisibly. Each gets its drawn shape outlined — a
//! sprite's quad, a text's box, a layer's bounds — the primary brighter, as
//! Unity and Godot tint the active object.

use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::LineBatch;
use voltra_scene::pick;
use voltra_scene::sprite::sheets::Sheets;

use crate::selection::Selection;

/// The primary's outline: the selection's own orange.
const PRIMARY: [f32; 4] = [1.0, 0.6, 0.15, 1.0];

/// The others': the same orange, dimmer.
const OTHER: [f32; 4] = [1.0, 0.6, 0.15, 0.55];

/// Outline thickness, in pixels. Matches the other overlays.
const WIDTH: f32 = 1.5;

/// One outline to draw: its corners in the world and its colour.
pub type Outline = ([Vec2; 4], [f32; 4]);

/// The outline of each selected entity that draws something. A lone
/// selection is left to the gizmo, as it always was. Gathered apart from
/// the drawing, because the sheets it reads and the lines it writes are
/// borrowed from the same frame.
pub fn outlines(world: &World, selection: &Selection, sheets: Sheets<'_>) -> Vec<Outline> {
    if selection.len() < 2 {
        return Vec::new();
    }
    let primary = selection.primary();
    selection
        .iter()
        .filter_map(|entity| {
            let corners = pick::outline(world, entity, sheets)?;
            let color = if Some(entity) == primary {
                PRIMARY
            } else {
                OTHER
            };
            Some((corners, color))
        })
        .collect()
}

/// Pushes `outlines` into `lines`.
pub fn draw(outlines: &[Outline], lines: &mut LineBatch) {
    for (corners, color) in outlines {
        for i in 0..4 {
            lines.push(corners[i], corners[(i + 1) % 4], WIDTH, *color);
        }
    }
}
