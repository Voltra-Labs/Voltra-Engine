//! The MCP server: the tools, as the protocol presents them — one router
//! per group of tools, added together.

mod assets;
mod discovery;
mod editor;
mod project;
mod run;
mod scenes;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::{tool_handler, ErrorData, ServerHandler};

use crate::tools::ToolResult;
use crate::Project;

/// The server for one project.
#[derive(Clone)]
pub struct VoltraMcp {
    project: Project,
    /// The port a running editor's bridge listens on.
    editor_port: u16,
    /// Every group's tools, added together; what `#[tool_handler]` routes
    /// calls through.
    tool_router: ToolRouter<Self>,
}

impl VoltraMcp {
    pub fn new(project: Project) -> Self {
        Self::with_editor_port(project, voltra_bridge::DEFAULT_PORT)
    }

    /// A server whose `editor_*` tools reach the editor bridge on `port`.
    pub fn with_editor_port(project: Project, editor_port: u16) -> Self {
        Self {
            project,
            editor_port,
            tool_router: Self::discovery_router()
                + Self::scene_router()
                + Self::asset_router()
                + Self::run_router()
                + Self::project_router()
                + Self::editor_router(),
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }
}

/// A tool's answer as the client reads it: the JSON as text, or the error
/// as a tool error — not a protocol error, so the client can read why and
/// try again.
fn answer(result: ToolResult) -> Result<CallToolResult, ErrorData> {
    Ok(match result {
        Ok(value) => CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&value)
                .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}")),
        )]),
        Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_string())]),
    })
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for VoltraMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Voltra Engine project tools. Paths are asset paths under the project's asset root \
             (\"scenes/level1.ron\"). Entities are named by their id (a UUID) or by a path of \
             names from a root (\"Player/Arm\"). Components are RON as scene files hold them; \
             components_list gives each one's template. Edits are saved to the scene file at \
             once, or not at all when they fail. The editor_* tools drive a running editor \
             started with `voltra-editor --bridge`, its edits undoable by the person there.",
        )
    }
}
