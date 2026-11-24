//! The editor's end of the bridge: requests from `voltra-mcp`, applied in
//! the frame.
//!
//! Off unless the editor was started with `--bridge`. Each frame, after the
//! history has watched and before any panel draws, every request that
//! arrived is applied and answered: an edit only while editing, as one undo
//! entry under an "AI: …" label, so a person's Ctrl+Z takes it back like
//! their own; play, step and stop through the toolbar's own paths; a
//! screenshot from the viewport as this frame drew it.

mod edits;
mod read;

use base64::Engine;
use serde_json::{json, Value};
use voltra_bridge::{Command, Pending};
use voltra_core::UiFrame;

use crate::editor::Editor;
use crate::play::PlayState;
use crate::scene_file;
use crate::selection::Selection;

/// Applies and answers whatever arrived since last frame.
pub(crate) fn serve(editor: &mut Editor, frame: &mut UiFrame<'_>) {
    let Some(server) = editor.bridge.as_ref() else {
        return;
    };
    let pending: Vec<Pending> = server.poll();
    for request in pending {
        let result = apply(editor, frame, &request.command);
        request.answer(result);
    }
}

fn apply(editor: &mut Editor, frame: &mut UiFrame<'_>, command: &Command) -> Result<Value, String> {
    let editing = editor.play.state() == PlayState::Editing;
    match command {
        Command::Status => Ok(json!({
            "scene": editor.document.path().map(|path| path.as_str()),
            "unsaved_changes": editor.document.is_dirty(&editor.history),
            "play": format!("{:?}", editor.play.state()),
            "selection": editor.selection.ids(frame.world).iter().map(|id| id.0.to_string()).collect::<Vec<_>>(),
            "entities": frame.world.query::<voltra_scene::SceneId>().count(),
            "undo": editor.history.undo_label(),
            "redo": editor.history.redo_label(),
        })),
        Command::Read => read::entities(frame.world, &editor.registry),
        Command::Snap {
            enabled,
            step,
            angle,
            scale,
        } => {
            let snap = &mut editor.snap;
            if let Some(v) = enabled {
                snap.enabled = *v;
            }
            for (field, value) in [
                (&mut snap.step, step),
                (&mut snap.angle, angle),
                (&mut snap.scale, scale),
            ] {
                if let Some(v) = value.filter(|v| v.is_finite() && *v > 0.0) {
                    *field = v;
                }
            }
            Ok(json!({
                "enabled": snap.enabled,
                "step": snap.step,
                "angle": snap.angle,
                "scale": snap.scale,
            }))
        }
        Command::Console {
            level,
            filter,
            last,
        } => {
            editor.console.read();
            let level = match level.as_deref() {
                Some(name) => crate::console::level_named(name)
                    .ok_or_else(|| format!("{name:?} is no log level"))?,
                None => log::Level::Info,
            };
            let lines: Vec<Value> = editor
                .console
                .shown(level, filter.as_deref().unwrap_or(""))
                .map(|line| {
                    json!({
                        "level": line.level.to_string(),
                        "target": line.target,
                        "message": line.message,
                    })
                })
                .collect();
            let skip = lines.len().saturating_sub(last.unwrap_or(usize::MAX));
            Ok(json!({ "lines": lines[skip..] }))
        }
        Command::Settings { file, text } => {
            let which = crate::project_settings::SettingsFile::named(file)
                .ok_or_else(|| format!("{file:?} is not input, layers, buses or build"))?;
            let assets = frame.textures.root().to_path_buf();
            match text {
                Some(text) => {
                    let path = crate::project_settings::write(which, &assets, text)?;
                    Ok(json!({ "saved": path.display().to_string() }))
                }
                None => {
                    let text = crate::project_settings::read(which, &assets)?;
                    Ok(json!({ "path": which.path(&assets).display().to_string(), "text": text }))
                }
            }
        }
        Command::Spawn { .. }
        | Command::Drag { .. }
        | Command::Delete { .. }
        | Command::Reparent { .. }
        | Command::Rename { .. }
        | Command::ComponentSet { .. }
        | Command::ComponentRemove { .. } => {
            if !editing {
                return Err(
                    "the editor is playing: stop first, edits land in the authored scene".into(),
                );
            }
            let answer = edits::apply(editor, frame.world, command)?;
            // What the edit wrote names its texture, atlas, font or clip by
            // path; the handles that draw them are this session's, so they
            // are looked up now — or a textured sprite spawned here draws
            // the placeholder and a text nothing, with no word why.
            frame.resolve_scene_assets();
            Ok(answer)
        }
        Command::Select { entities } => {
            let mut ids = Vec::new();
            for reference in entities {
                let entity =
                    voltra_scene::edit::find(frame.world, reference).map_err(|e| e.to_string())?;
                if let Some(&id) = frame.world.get::<voltra_scene::SceneId>(entity) {
                    ids.push(id);
                }
            }
            editor.selection = Selection::from_ids(frame.world, &ids);
            Ok(json!({ "selected": ids.len() }))
        }
        Command::Undo => {
            let label = editor.history.undo_label();
            if label.is_none() {
                return Err("nothing to undo".into());
            }
            editor.undo(frame);
            Ok(json!({ "undone": label }))
        }
        Command::Redo => {
            let label = editor.history.redo_label();
            if label.is_none() {
                return Err("nothing to redo".into());
            }
            editor.redo(frame);
            Ok(json!({ "redone": label }))
        }
        Command::Save { path } => {
            if !editing {
                return Err(
                    "the editor is playing: stop first, so the authored scene is saved".into(),
                );
            }
            let path = match path {
                Some(text) => {
                    let path = scene_file::scene_path(text)?;
                    scene_file::save_as(editor, frame, path.clone())?;
                    path
                }
                None => scene_file::save(editor, frame)
                    .map_err(|e| format!("{e} (pass a path to save it as)"))?,
            };
            Ok(json!({ "saved": path.as_str() }))
        }
        Command::Open {
            path,
            discard_changes,
        } => {
            let path = scene_file::scene_path(path)?;
            keep_unsaved(editor, *discard_changes)?;
            scene_file::open(editor, frame, &path)?;
            Ok(json!({
                "opened": path.as_str(),
                "entities": frame.world.query::<voltra_scene::SceneId>().count(),
            }))
        }
        Command::New { discard_changes } => {
            keep_unsaved(editor, *discard_changes)?;
            scene_file::new_scene(editor, frame);
            Ok(json!({ "new": true }))
        }
        Command::Play => {
            editor.start_play(frame);
            Ok(json!({ "play": format!("{:?}", editor.play.state()) }))
        }
        Command::Pause => {
            if editing {
                return Err("the editor is not playing".into());
            }
            editor.pause_play(frame);
            Ok(json!({ "play": format!("{:?}", editor.play.state()) }))
        }
        Command::Step { frames } => {
            if editing {
                return Err("play first: a step runs the paused game".into());
            }
            if editor.play.state() == PlayState::Playing {
                editor.pause_play(frame);
            }
            for _ in 0..(*frames).clamp(1, 600) {
                editor.step_play(frame);
            }
            Ok(json!({ "stepped": (*frames).clamp(1, 600) }))
        }
        Command::Stop => {
            let stopped = editor.stop_play(frame);
            Ok(json!({ "stopped": stopped }))
        }
        Command::Input { action, frames } => {
            if editing {
                return Err("input reaches a running game: play first".into());
            }
            if !frame.hold_action(action, (*frames).clamp(1, 6000)) {
                return Err(format!("the input map has no binding for {action:?}"));
            }
            Ok(json!({ "holding": action, "frames": frames }))
        }
        Command::Profile { frames, record } => {
            if let Some(record) = record {
                voltra_profile::set_enabled(*record);
            }
            let mut answer = voltra_bridge::profile::to_json(&voltra_profile::summary(
                (*frames).clamp(1, voltra_profile::HISTORY as u32) as usize,
            ));
            answer["recording"] = json!(voltra_profile::is_enabled());
            answer["frames_recorded"] = json!(voltra_profile::frames_recorded());
            Ok(answer)
        }
        Command::Screenshot => {
            let shot = frame.capture_viewport();
            let png = shot.png_bytes().map_err(|e| e.to_string())?;
            Ok(json!({
                "width": shot.width,
                "height": shot.height,
                "png_base64": base64::engine::general_purpose::STANDARD.encode(png),
            }))
        }
    }
}

/// Refuses to let unsaved changes go unless the caller said to: the person
/// at the editor may be in the middle of them, and an AI must not throw
/// their work away by accident.
fn keep_unsaved(editor: &Editor, discard_changes: bool) -> Result<(), String> {
    if editor.document.is_dirty(&editor.history) && !discard_changes {
        return Err(format!(
            "{} has unsaved changes: save it first, or pass discard_changes to drop them",
            editor.document.name()
        ));
    }
    Ok(())
}
