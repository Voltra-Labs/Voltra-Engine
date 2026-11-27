//! Which text's laid-out box is under a point.
//!
//! The box rather than the glyphs' ink: a click between two letters, or in
//! the counter of an "o", is still a click on the label — Unity's and
//! Godot's rule for a text's rect.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::MIN_DETERMINANT;
use crate::hierarchy::WorldTransforms2D;
use crate::sprite::draw_key;
use crate::sprite::sheets::Sheets;
use crate::text::{shape, Text};
use crate::transform::Transform2D;

/// The topmost text whose box contains `point`, in world space, and the key
/// it draws at. Nothing without fonts in `sheets`.
pub fn text_at(
    world: &World,
    point: Vec2,
    sheets: Sheets<'_>,
) -> Option<(Entity, (i32, i32, u32))> {
    let fonts = sheets.fonts?;
    let transforms = WorldTransforms2D::seen_from(world, sheets.camera);
    world
        .query2::<Transform2D, Text>()
        .filter(|(entity, _, text)| {
            if crate::ui::is_ui(world, *entity) {
                return false;
            }
            let matrix = transforms.matrix(*entity);
            // See `MIN_DETERMINANT`: a collapsed text is nowhere.
            if matrix.determinant().abs() < MIN_DETERMINANT {
                return false;
            }
            let local = matrix.inverse().transform_point2(point);
            shape::bounds(text, fonts).is_some_and(|bounds| bounds.contains(local))
        })
        .map(|(entity, _, text)| {
            let rank = sheets.rank(&text.sorting_layer);
            (entity, draw_key(entity, rank, text.sort_order))
        })
        .max_by_key(|(_, key)| *key)
}
