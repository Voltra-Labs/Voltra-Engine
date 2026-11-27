//! A real MCP client talking to the server over an in-process pipe.

#![allow(dead_code)]

use std::path::Path;

use rmcp::model::{CallToolRequestParams, CallToolResult, ClientConfig};
use rmcp::service::{RoleClient, RunningService};
use rmcp::{ClientHandler, ServiceExt};
use serde_json::Value;
use voltra_mcp::{Project, VoltraMcp};

#[derive(Debug, Clone, Default)]
pub struct Client;

impl ClientHandler for Client {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::default()
    }
}

/// The repository itself, read-only: its shipped assets.
pub fn repo_project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Project::new(root, None)
}

/// A scratch project with a copy of the repository's scenes, to write to.
pub fn scratch_project() -> (Project, voltra_testkit::Scratch) {
    let root = voltra_testkit::scratch_root();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    copy_dir(&source, &root.join("assets"));
    (Project::new(&root, None), root)
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a dir");
    for entry in std::fs::read_dir(from).expect("readable").flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("copied");
        }
    }
}

/// A client connected to a server for `project`.
pub async fn client_for(project: Project) -> RunningService<RoleClient, Client> {
    client_of(VoltraMcp::new(project)).await
}

/// A client connected to `server`.
pub async fn client_of(server: VoltraMcp) -> RunningService<RoleClient, Client> {
    // What `main` does: the engine's log, which a run hands back.
    voltra_core::logging::init(false);
    let (server_side, client_side) = tokio::io::duplex(1 << 20);
    tokio::spawn(async move {
        if let Ok(service) = server.serve(server_side).await {
            let _ = service.waiting().await;
        }
    });
    Client
        .serve(client_side)
        .await
        .expect("the client connects")
}

/// Calls `tool` with `args`.
pub async fn call(
    client: &RunningService<RoleClient, Client>,
    tool: &str,
    args: Value,
) -> CallToolResult {
    let arguments = args.as_object().cloned().unwrap_or_default();
    client
        .call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(arguments))
        .await
        .expect("the call is answered")
}

/// The first text block of a result.
pub fn text_of(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .find_map(|c| c.as_text().map(|t| t.text.clone()))
        .unwrap_or_default()
}
