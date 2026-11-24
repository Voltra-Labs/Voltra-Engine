//! What goes over the socket.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One command, with the id its answer will carry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
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
    Spawn {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        parent: Option<String>,
        /// Registered component name → RON.
        #[serde(default)]
        components: BTreeMap<String, String>,
    },
    Delete {
        entity: String,
    },
    Reparent {
        entity: String,
        #[serde(default)]
        parent: Option<String>,
    },
    Rename {
        entity: String,
        name: String,
    },
    ComponentSet {
        entity: String,
        component: String,
        ron: String,
    },
    ComponentRemove {
        entity: String,
        component: String,
    },
    /// Selects these entities, the last the primary; none clears it.
    Select {
        entities: Vec<String>,
    },
    Undo,
    Redo,
    /// Writes the open scene to its file, or to `path` (an asset path),
    /// which becomes its file.
    Save {
        #[serde(default)]
        path: Option<String>,
    },
    /// Opens the scene at `path` in place of the open one. Refused over
    /// unsaved changes unless `discard_changes`.
    Open {
        path: String,
        #[serde(default)]
        discard_changes: bool,
    },
    /// Replaces the open scene with an empty, untitled one, on the same
    /// terms as `Open`.
    New {
        #[serde(default)]
        discard_changes: bool,
    },
    Play,
    Pause,
    /// Runs this many fixed steps while paused.
    Step {
        frames: u32,
    },
    Stop,
    /// Holds an action down for this many frames of play.
    Input {
        action: String,
        frames: u32,
    },
    /// The viewport as it was last drawn, as PNG bytes in base64.
    Screenshot,
    /// The grid and snapping: sets what is given, answers all of it.
    Snap {
        #[serde(default)]
        enabled: Option<bool>,
        #[serde(default)]
        step: Option<f32>,
        #[serde(default)]
        angle: Option<f32>,
        #[serde(default)]
        scale: Option<f32>,
    },
    /// What a gizmo drag of `entity` with `tool` — `move`, `rotate` or
    /// `scale` — from `from` to `to`, world units, does: snapped as the
    /// viewport's would be, one undo entry.
    Drag {
        entity: String,
        tool: String,
        from: [f32; 2],
        to: [f32; 2],
    },
    /// The Console's lines at `level` (`error` … `trace`, default `info`)
    /// or louder holding `filter`, the last `last` of them.
    Console {
        #[serde(default)]
        level: Option<String>,
        #[serde(default)]
        filter: Option<String>,
        #[serde(default)]
        last: Option<usize>,
    },
    /// A project settings file — `input`, `layers` or `build` — read, or,
    /// with `text`, checked and saved as the Project Settings window saves.
    Settings {
        file: String,
        #[serde(default)]
        text: Option<String>,
    },
    /// The last `frames` recorded frames, summarised; `record` first starts
    /// or stops recording.
    Profile {
        frames: u32,
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
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
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
