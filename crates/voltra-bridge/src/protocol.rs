//! What goes over the socket.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One command, with the id its answer will carry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The id the answer will carry.
    pub id: u64,
    /// What to do.
    pub command: Command,
}

/// What a tool asks the editor to do. Entities are named as the MCP tools
/// name them: an id (a UUID) or a path of names from a root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    /// What the editor has open, its play state, its selection.
    Status,
    /// The open scene's entities, as `scene_read` answers a file.
    Read,
    /// Creates an entity named `name`, under `parent`, with `components`.
    Spawn {
        /// The new entity's name.
        #[serde(default)]
        name: Option<String>,
        /// The parent entity; none makes it a root.
        #[serde(default)]
        parent: Option<String>,
        /// Registered component name → RON.
        #[serde(default)]
        components: BTreeMap<String, String>,
    },
    /// Removes `entity` and everything under it.
    Delete {
        /// The entity to remove.
        entity: String,
    },
    /// Moves `entity` under `parent`.
    Reparent {
        /// The entity to move.
        entity: String,
        /// The new parent; none makes it a root.
        #[serde(default)]
        parent: Option<String>,
    },
    /// Renames `entity` to `name`.
    Rename {
        /// The entity to rename.
        entity: String,
        /// The new name.
        name: String,
    },
    /// Sets `component` on `entity` from RON.
    ComponentSet {
        /// The entity to change.
        entity: String,
        /// The registered component name.
        component: String,
        /// The component's value, as RON.
        ron: String,
    },
    /// Removes `component` from `entity`.
    ComponentRemove {
        /// The entity to change.
        entity: String,
        /// The registered component name.
        component: String,
    },
    /// Selects these entities, the last the primary; none clears it.
    Select {
        /// The entities to select, by id or path.
        entities: Vec<String>,
    },
    /// Undoes the last edit.
    Undo,
    /// Redoes the last edit that was undone.
    Redo,
    /// Writes the open scene to its file, or to `path` (an asset path),
    /// which becomes its file.
    Save {
        /// An asset path to save to, which becomes the scene's file.
        #[serde(default)]
        path: Option<String>,
    },
    /// Opens the scene at `path` in place of the open one. Refused over
    /// unsaved changes unless `discard_changes`.
    Open {
        /// The asset path of the scene to open.
        path: String,
        /// Open even over unsaved changes.
        #[serde(default)]
        discard_changes: bool,
    },
    /// Replaces the open scene with an empty, untitled one, on the same
    /// terms as `Open`.
    New {
        /// Replace even over unsaved changes.
        #[serde(default)]
        discard_changes: bool,
    },
    /// Starts playing the open scene.
    Play,
    /// Pauses play.
    Pause,
    /// Runs this many fixed steps while paused.
    Step {
        /// How many fixed steps to run, clamped to 1 to 600.
        frames: u32,
    },
    /// Stops play and returns to editing.
    Stop,
    /// Holds an action down for this many frames of play.
    Input {
        /// The input action, as the input map names it.
        action: String,
        /// How many frames to hold it, clamped to 1 to 6000.
        frames: u32,
    },
    /// The viewport as it was last drawn, as PNG bytes in base64.
    Screenshot,
    /// The grid and snapping: sets what is given, answers all of it.
    Snap {
        /// Whether snapping is on.
        #[serde(default)]
        enabled: Option<bool>,
        /// The grid's spacing, in world units; ignored unless positive.
        #[serde(default)]
        step: Option<f32>,
        /// The angle a turn snaps to, in degrees; ignored unless positive.
        #[serde(default)]
        angle: Option<f32>,
        /// The increment a scale snaps to; ignored unless positive.
        #[serde(default)]
        scale: Option<f32>,
    },
    /// What a gizmo drag of `entity` with `tool` — `move`, `rotate` or
    /// `scale` — from `from` to `to`, world units, does: snapped as the
    /// viewport's would be, one undo entry.
    Drag {
        /// The entity to drag.
        entity: String,
        /// The gizmo tool: `move`, `rotate` or `scale`.
        tool: String,
        /// Where the drag starts, in world units.
        from: [f32; 2],
        /// Where the drag ends, in world units.
        to: [f32; 2],
    },
    /// The Console's lines at `level` (`error` … `trace`, default `info`)
    /// or louder holding `filter`, the last `last` of them.
    Console {
        /// The quietest level to show, from `error` to `trace`; `info` when none.
        #[serde(default)]
        level: Option<String>,
        /// Text a line must hold to be shown.
        #[serde(default)]
        filter: Option<String>,
        /// How many of the newest matching lines to answer with.
        #[serde(default)]
        last: Option<usize>,
    },
    /// A project settings file — `input`, `layers` or `build` — read, or,
    /// with `text`, checked and saved as the Project Settings window saves.
    Settings {
        /// Which file: `input`, `layers` or `build`.
        file: String,
        /// New contents to check and save; none reads the file instead.
        #[serde(default)]
        text: Option<String>,
    },
    /// Packs every PNG under the folder `folder` (an asset path) into
    /// `<folder>.png` and `<folder>.atlas.ron` beside it, as the asset
    /// browser's "Pack into atlas" does.
    PackAtlas {
        /// The folder of loose pictures, as an asset path.
        folder: String,
        /// Cut transparent borders off first.
        #[serde(default)]
        trim: bool,
    },
    /// The last `frames` recorded frames, summarised; `record` first starts
    /// or stops recording.
    Profile {
        /// How many recorded frames to summarise.
        frames: u32,
        /// Starts (`true`) or stops (`false`) recording first; none leaves it as it is.
        #[serde(default)]
        record: Option<bool>,
    },
}

impl Command {
    /// What an undo entry made by this command is called.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Spawn { .. } => "AI: spawn",
            Self::Delete { .. } => "AI: delete",
            Self::Reparent { .. } => "AI: reparent",
            Self::Rename { .. } => "AI: rename",
            Self::ComponentSet { .. } => "AI: set component",
            Self::ComponentRemove { .. } => "AI: remove component",
            Self::Drag { .. } => "AI: drag",
            _ => "AI",
        }
    }
}

/// The answer to the request of the same id: a JSON value, or why not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The id of the request it answers.
    pub id: u64,
    /// The result, when the command succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<Value>,
    /// Why the command failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    /// The response to request `id` for the outcome `result`.
    pub fn answer(id: u64, result: Result<Value, String>) -> Self {
        match result {
            Ok(value) => Self {
                id,
                ok: Some(value),
                error: None,
            },
            Err(error) => Self {
                id,
                ok: None,
                error: Some(error),
            },
        }
    }

    /// The result: the error if there is one, else the value, or null if there is neither.
    pub fn into_result(self) -> Result<Value, String> {
        match (self.ok, self.error) {
            (_, Some(error)) => Err(error),
            (Some(value), None) => Ok(value),
            (None, None) => Ok(Value::Null),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_reads_back_from_its_line() {
        let request = Request {
            id: 7,
            command: Command::ComponentSet {
                entity: "Player".into(),
                component: "Sprite".into(),
                ron: "(color: (1.0, 0.0, 0.0, 1.0))".into(),
            },
        };
        let line = serde_json::to_string(&request).expect("writes");
        assert!(line.contains("\"op\":\"component_set\""), "{line}");
        assert_eq!(
            serde_json::from_str::<Request>(&line).expect("reads"),
            request
        );
        let response = Response::answer(7, Err("no such entity".into()));
        let back: Response =
            serde_json::from_str(&serde_json::to_string(&response).expect("writes"))
                .expect("reads");
        assert_eq!(back.into_result(), Err("no such entity".to_owned()));
    }
}
