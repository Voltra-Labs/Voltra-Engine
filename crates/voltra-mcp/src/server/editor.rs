//! The tools that drive a running editor — its open scene, play mode,
//! undo history and viewport — through the bridge.

use std::collections::BTreeMap;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{tool, tool_router, ErrorData};
use serde::Deserialize;
use serde_json::Value;
use voltra_bridge::Command;

use super::{answer, VoltraMcp};
use crate::tools::{editor, ToolResult};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorSpawnArgs {
    #[serde(default)]
    pub name: Option<String>,
    /// An id or a path of names.
    #[serde(default)]
    pub parent: Option<String>,
    /// Registered component name → RON.
    #[serde(default)]
    pub components: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorEntityArgs {
    /// An id, or a path of names from a root ("Player/Arm").
    pub entity: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorReparentArgs {
    pub entity: String,
    /// Absent makes it a root.
    #[serde(default)]
    pub parent: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorRenameArgs {
    pub entity: String,
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorComponentSetArgs {
    pub entity: String,
    pub component: String,
    pub ron: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorComponentArgs {
    pub entity: String,
    pub component: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorSelectArgs {
    /// Ids or name paths, the last the primary; empty clears the selection.
    pub entities: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HistoryAction {
    Undo,
    Redo,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorHistoryArgs {
    pub action: HistoryAction,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlayAction {
    Play,
    Pause,
    Step,
    Stop,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorPlayArgs {
    pub action: PlayAction,
    /// For `step`: how many fixed steps; 1 by default.
    #[serde(default)]
    pub frames: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorSaveArgs {
    /// Where to save instead of the scene's own file; it becomes its file.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorOpenArgs {
    /// The scene's asset path ("scenes/level.ron").
    pub path: String,
    /// Throw away the open scene's unsaved changes.
    #[serde(default)]
    pub discard_changes: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorNewArgs {
    /// Throw away the open scene's unsaved changes.
    #[serde(default)]
    pub discard_changes: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorProfileArgs {
    /// How many recent frames to summarise; 120 (two seconds) by default,
    /// at most 600.
    #[serde(default = "two_seconds")]
    pub frames: u32,
}

fn two_seconds() -> u32 {
    120
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EditorInputArgs {
    /// The input map's action.
    pub action: String,
    /// How many frames to hold it.
    pub frames: u32,
}

impl VoltraMcp {
    /// `command` to the editor, off the async runtime.
    async fn bridge(&self, command: Command) -> ToolResult {
        let port = self.editor_port;
        tokio::task::spawn_blocking(move || editor::call(port, command))
            .await
            .unwrap_or_else(|e| {
                Err(crate::tools::ToolError(format!(
                    "the bridge call stopped: {e}"
                )))
            })
    }
}

#[tool_router(router = editor_router, vis = "pub(super)")]
impl VoltraMcp {
    #[tool(
        description = "Ask the running editor (started with `voltra-editor --bridge`) what it has open: the scene path, play state, selection, entity count, and what undo and redo would do."
    )]
    async fn editor_status(&self) -> Result<CallToolResult, ErrorData> {
        answer(self.bridge(Command::Status).await)
    }

    #[tool(
        description = "Read the running editor's open scene, live: every entity's id, name, parent and components as RON."
    )]
    async fn editor_read(&self) -> Result<CallToolResult, ErrorData> {
        answer(self.bridge(Command::Read).await)
    }

    #[tool(
        description = "Add an entity to the running editor's scene, as one undo entry the person can take back with Ctrl+Z. Only while editing."
    )]
    async fn editor_spawn(
        &self,
        Parameters(args): Parameters<EditorSpawnArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::Spawn {
                name: args.name,
                parent: args.parent,
                components: args.components,
            })
            .await,
        )
    }

    #[tool(
        description = "Delete an entity and everything under it in the running editor, as one undo entry. Only while editing."
    )]
    async fn editor_delete(
        &self,
        Parameters(args): Parameters<EditorEntityArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::Delete {
                entity: args.entity,
            })
            .await,
        )
    }

    #[tool(
        description = "Put an entity under another — or make it a root — in the running editor, as one undo entry."
    )]
    async fn editor_reparent(
        &self,
        Parameters(args): Parameters<EditorReparentArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::Reparent {
                entity: args.entity,
                parent: args.parent,
            })
            .await,
        )
    }

    #[tool(description = "Rename an entity in the running editor, as one undo entry.")]
    async fn editor_rename(
        &self,
        Parameters(args): Parameters<EditorRenameArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::Rename {
                entity: args.entity,
                name: args.name,
            })
            .await,
        )
    }

    #[tool(
        description = "Set one component from RON on an entity in the running editor, as one undo entry."
    )]
    async fn editor_component_set(
        &self,
        Parameters(args): Parameters<EditorComponentSetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::ComponentSet {
                entity: args.entity,
                component: args.component,
                ron: args.ron,
            })
            .await,
        )
    }

    #[tool(
        description = "Remove one component from an entity in the running editor, as one undo entry."
    )]
    async fn editor_component_remove(
        &self,
        Parameters(args): Parameters<EditorComponentArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::ComponentRemove {
                entity: args.entity,
                component: args.component,
            })
            .await,
        )
    }

    #[tool(
        description = "Select entities in the running editor, the last the primary; an empty list clears the selection."
    )]
    async fn editor_select(
        &self,
        Parameters(args): Parameters<EditorSelectArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::Select {
                entities: args.entities,
            })
            .await,
        )
    }

    #[tool(description = "Undo or redo the running editor's last edit — the person's or an AI's.")]
    async fn editor_history(
        &self,
        Parameters(args): Parameters<EditorHistoryArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let command = match args.action {
            HistoryAction::Undo => Command::Undo,
            HistoryAction::Redo => Command::Redo,
        };
        answer(self.bridge(command).await)
    }

    #[tool(
        description = "Save the running editor's scene to its file, or to `path` (an asset path ending in .ron), which becomes its file — overwriting what is there. Only while editing."
    )]
    async fn editor_save(
        &self,
        Parameters(args): Parameters<EditorSaveArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(self.bridge(Command::Save { path: args.path }).await)
    }

    #[tool(
        description = "Open the scene at `path` in the running editor, in place of its open scene; its undo history starts over. Refused over unsaved changes unless discard_changes — which throws them away."
    )]
    async fn editor_open(
        &self,
        Parameters(args): Parameters<EditorOpenArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::Open {
                path: args.path,
                discard_changes: args.discard_changes,
            })
            .await,
        )
    }

    #[tool(
        description = "Replace the running editor's scene with an empty, untitled one. Refused over unsaved changes unless discard_changes — which throws them away."
    )]
    async fn editor_new(
        &self,
        Parameters(args): Parameters<EditorNewArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::New {
                discard_changes: args.discard_changes,
            })
            .await,
        )
    }

    #[tool(
        description = "Drive the running editor's play mode: play, pause, step (some fixed steps while paused) or stop (which puts the authored scene back)."
    )]
    async fn editor_play(
        &self,
        Parameters(args): Parameters<EditorPlayArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let command = match args.action {
            PlayAction::Play => Command::Play,
            PlayAction::Pause => Command::Pause,
            PlayAction::Step => Command::Step {
                frames: args.frames.unwrap_or(1),
            },
            PlayAction::Stop => Command::Stop,
        };
        answer(self.bridge(command).await)
    }

    #[tool(
        description = "Hold an input action down for some frames in the running editor's game, as a player would. Only while playing."
    )]
    async fn editor_input(
        &self,
        Parameters(args): Parameters<EditorInputArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.bridge(Command::Input {
                action: args.action,
                frames: args.frames,
            })
            .await,
        )
    }

    #[tool(
        description = "Where the running editor's frames go: frame time and fps, each scope (update, tick, physics/step/solve, draw, ui, present…) with calls, average, worst and own milliseconds and its share of the frame, and counters such as entities and contacts. Records for as long as it needs when recording is off. Play first to profile the game rather than the editor at rest."
    )]
    async fn editor_profile(
        &self,
        Parameters(args): Parameters<EditorProfileArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let port = self.editor_port;
        let frames = args.frames;
        answer(
            tokio::task::spawn_blocking(move || editor::profile(port, frames))
                .await
                .unwrap_or_else(|e| {
                    Err(crate::tools::ToolError(format!("the profile stopped: {e}")))
                }),
        )
    }

    #[tool(
        description = "A screenshot of the running editor's viewport as it was last drawn — the scene, lit and graded, without the editor's overlay."
    )]
    async fn editor_screenshot(&self) -> Result<CallToolResult, ErrorData> {
        Ok(match self.bridge(Command::Screenshot).await {
            Ok(Value::Object(mut shot)) => {
                let png = shot
                    .remove("png_base64")
                    .and_then(|v| v.as_str().map(str::to_owned));
                let mut content = vec![ContentBlock::text(Value::Object(shot).to_string())];
                if let Some(png) = png {
                    content.push(ContentBlock::image(png, "image/png"));
                }
                CallToolResult::success(content)
            }
            Ok(other) => CallToolResult::success(vec![ContentBlock::text(other.to_string())]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_string())]),
        })
    }
}
