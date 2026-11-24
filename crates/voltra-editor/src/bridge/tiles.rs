//! The tilemap tools through the bridge: a block picked off the palette,
//! the stamp turned, a stroke of any tool through a list of cells — one undo
//! entry, as a stroke by hand is — and a layer's cells read back.

use serde_json::{json, Value};
use voltra_bridge::Command;
use voltra_core::UiFrame;
use voltra_ecs::Entity;
use voltra_render::glam::{IVec2, UVec2};
use voltra_scene::{SceneId, Tile, Tilemap};

use crate::editor::Editor;
use crate::paint::palette::PaletteGrid;
use crate::paint::{PaintTool, Stamp};
use crate::play::PlayState;
use crate::undo::SceneView;

/// The most cells a `TileRead` answers with.
const MAX_READ: i64 = 1 << 16;

pub(super) fn apply(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    command: &Command,
) -> Result<Value, String> {
    match command {
        Command::TilePick { layer, from, to } => {
            let layer = find_layer(frame, layer)?;
            let map = frame.world.get::<Tilemap>(layer).expect("found as a layer");
            let atlas = map
                .atlas_handle
                .and_then(|handle| frame.atlases.try_get(handle))
                .ok_or("the layer has no tileset loaded")?;
            let grid = PaletteGrid::of(atlas);
            editor.tile_palette.pick(
                &mut editor.paint,
                &grid,
                UVec2::from(*from),
                UVec2::from(*to),
            )?;
            Ok(stamp(&editor.paint.stamp))
        }
        Command::TileStamp {
            turns,
            flip_x,
            flip_y,
        } => {
            let paint = &mut editor.paint;
            for _ in 0..turns.rem_euclid(4) {
                paint.stamp = paint.stamp.rotated_cw();
            }
            if *flip_x {
                paint.stamp = paint.stamp.mirrored_x();
            }
            if *flip_y {
                paint.stamp = paint.stamp.mirrored_y();
            }
            Ok(stamp(&paint.stamp))
        }
        Command::TilePaint {
            layer,
            tool,
            cells,
            pick,
            terrain,
        } => {
            if editor.play.state() != PlayState::Editing {
                return Err(
                    "the editor is playing: stop first, strokes land in the authored scene".into(),
                );
            }
            let tool = PaintTool::named(tool)
                .ok_or_else(|| format!("{tool:?} is not brush, rectangle, eraser or fill"))?;
            let layer = find_layer(frame, layer)?;
            let cells: Vec<IVec2> = cells.iter().map(|&c| IVec2::from(c)).collect();
            let id = frame.world.get::<SceneId>(layer).copied();
            let view = SceneView {
                world: frame.world,
                registry: &editor.registry,
                selected: editor.selection.ids(frame.world),
            };
            editor.history.begin(command.label(), view, id);

            // The terrain this stroke paints with is the request's, not
            // whatever the inspector had chosen.
            let chosen = std::mem::replace(&mut editor.paint.terrain, *terrain);
            let label = editor
                .paint
                .stroke_through(frame.world, layer, tool, *pick, &cells);
            editor.paint.terrain = chosen;

            let view = SceneView {
                world: frame.world,
                registry: &editor.registry,
                selected: editor.selection.ids(frame.world),
            };
            // An eyedropper stroke changed nothing; the history drops it.
            editor.history.commit(view);
            Ok(json!({
                "stroke": label,
                "tiles": frame.world.get::<Tilemap>(layer).map_or(0, Tilemap::len),
                "stamp": stamp(&editor.paint.stamp),
            }))
        }
        Command::TileRead { layer, from, to } => {
            let layer = find_layer(frame, layer)?;
            let map = frame.world.get::<Tilemap>(layer).expect("found as a layer");
            let (a, b) = (IVec2::from(*from), IVec2::from(*to));
            let (min, max) = (a.min(b), a.max(b));
            let size = max.as_i64vec2() - min.as_i64vec2() + 1;
            if size.x.saturating_mul(size.y) > MAX_READ {
                return Err(format!("at most {MAX_READ} cells a read"));
            }
            // Rows from the top, as the scene is seen.
            let rows: Vec<Vec<Value>> = (min.y..=max.y)
                .rev()
                .map(|y| {
                    (min.x..=max.x)
                        .map(|x| map.get(IVec2::new(x, y)).map_or(Value::Null, tile))
                        .collect()
                })
                .collect();
            Ok(json!({ "rows": rows, "tiles": map.len() }))
        }
        _ => Err("not a tile command".into()),
    }
}

fn find_layer(frame: &UiFrame<'_>, reference: &str) -> Result<Entity, String> {
    let entity = voltra_scene::edit::find(frame.world, reference).map_err(|e| e.to_string())?;
    if frame.world.get::<Tilemap>(entity).is_none() {
        return Err(format!("{reference:?} is not a tilemap layer"));
    }
    Ok(entity)
}

fn tile(tile: Tile) -> Value {
    json!({
        "frame": tile.frame,
        "flip_x": tile.flip_x,
        "flip_y": tile.flip_y,
        "transpose": tile.transpose,
    })
}

/// The stamp, its rows from the top, holes as null.
fn stamp(stamp: &Stamp) -> Value {
    let size = stamp.size();
    let rows: Vec<Vec<Value>> = (0..size.y)
        .rev()
        .map(|y| {
            (0..size.x)
                .map(|x| stamp.get(UVec2::new(x, y)).map_or(Value::Null, tile))
                .collect()
        })
        .collect();
    json!({ "size": [size.x, size.y], "rows": rows })
}
