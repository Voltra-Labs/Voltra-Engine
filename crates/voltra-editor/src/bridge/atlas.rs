//! The Atlas Editor through the bridge: what its buttons and fields do,
//! applied to the atlas it has open, each change one entry of its history.

use serde_json::{json, Value};
use voltra_assets::{AssetPath, Frame};
use voltra_bridge::Command;
use voltra_core::UiFrame;
use voltra_render::glam::{UVec2, Vec2};

use crate::atlas_doc::AtlasDoc;
use crate::editor::Editor;

pub(super) fn apply(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    command: &Command,
) -> Result<Value, String> {
    let window = &mut editor.atlas_editor;
    if let Command::AtlasOpen { path } = command {
        let path = AssetPath::new(path).map_err(|e| e.to_string())?;
        window.open_path(frame, &path)?;
        return Ok(read(window.doc.as_ref().expect("just opened")));
    }
    let doc = window
        .doc
        .as_mut()
        .ok_or("the Atlas Editor has nothing open: atlas_open first")?;
    match command {
        Command::AtlasRead => {}
        Command::AtlasSlice {
            cell,
            offset,
            spacing,
        } => {
            doc.slice_grid(
                UVec2::from(*cell),
                UVec2::from(*offset),
                UVec2::from(*spacing),
            )?;
        }
        Command::AtlasRegion {
            index,
            name,
            rect,
            pivot,
            remove,
        } => {
            let frame_of = |rect: &[u32; 4]| Frame::new(rect[0], rect[1], rect[2], rect[3]);
            match index {
                None => {
                    let rect = rect.as_ref().ok_or("a new region needs its rect")?;
                    let index = doc.add(frame_of(rect), name.as_deref())?;
                    if let Some(pivot) = pivot {
                        doc.set_pivot(index, Some(Vec2::from(*pivot)))?;
                    }
                }
                Some(index) if *remove => doc.remove(*index)?,
                Some(index) => {
                    if let Some(name) = name {
                        doc.rename(*index, name)?;
                    }
                    if let Some(rect) = rect {
                        doc.set_frame(*index, frame_of(rect))?;
                    }
                    if let Some(pivot) = pivot {
                        doc.set_pivot(*index, Some(Vec2::from(*pivot)))?;
                    }
                }
            }
        }
        Command::AtlasUndo => {
            if !doc.undo() {
                return Err("nothing to undo in the Atlas Editor".into());
            }
        }
        Command::AtlasRedo => {
            if !doc.redo() {
                return Err("nothing to redo in the Atlas Editor".into());
            }
        }
        Command::AtlasSave { path } => {
            let to = path
                .as_deref()
                .map(AssetPath::new)
                .transpose()
                .map_err(|e| e.to_string())?;
            let root = frame.textures.root().to_path_buf();
            doc.save(&root, to)?;
            frame.atlases.reload(doc.path());
        }
        _ => return Err("not an Atlas Editor command".into()),
    }
    Ok(read(doc))
}

/// What the bridge answers every atlas command with: the atlas as it now is.
fn read(doc: &AtlasDoc) -> Value {
    let regions: Vec<Value> = doc
        .regions()
        .iter()
        .map(|region| {
            json!({
                "name": region.name,
                "rect": [region.frame.x, region.frame.y, region.frame.width, region.frame.height],
                "pivot": region.pivot.map(|pivot| [pivot.x, pivot.y]),
            })
        })
        .collect();
    json!({
        "path": doc.path().as_str(),
        "texture": doc.texture().as_str(),
        "sheet": [doc.sheet().x, doc.sheet().y],
        "unsaved_changes": doc.is_dirty(),
        "can_undo": doc.can_undo(),
        "regions": regions,
    })
}
