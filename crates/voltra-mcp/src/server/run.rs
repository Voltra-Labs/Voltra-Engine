//! The tool that runs a scene.

use base64::Engine;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{tool, tool_router, ErrorData};
use serde::Deserialize;

use super::VoltraMcp;
use crate::tools::run::{self, HeldAction, RunRequest};

fn sixtieth() -> f32 {
    1.0 / 60.0
}

fn sixty() -> u32 {
    60
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ScreenshotSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunArgs {
    /// The scene, as an asset path.
    pub scene: String,
    /// Frames to run; 60 by default, at most 6000.
    #[serde(default = "sixty")]
    pub frames: u32,
    /// Seconds a frame lasts; 1/60 by default.
    #[serde(default = "sixtieth")]
    pub delta: f32,
    /// Actions held over frames of the run, as a player would press them.
    #[serde(default)]
    pub inputs: Vec<HeldAction>,
    /// Entities to report in full (ids or name paths); empty reports where
    /// every named entity ended up.
    #[serde(default)]
    pub report: Vec<String>,
    /// A screenshot at the end, through the scene's camera; absent takes
    /// none.
    #[serde(default)]
    pub screenshot: Option<ScreenshotSize>,
}

#[tool_router(router = run_router, vis = "pub(super)")]
impl VoltraMcp {
    #[tool(
        description = "Run a scene headless, as the player would — the game loop, physics, animation, cameras — for some frames, holding input actions on the frames given. Answers the reported entities (every component as RON) or where each named entity ended up, the engine's log, and optionally a screenshot through the scene's camera."
    )]
    async fn run_scene(
        &self,
        Parameters(args): Parameters<RunArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let project = self.project.clone();
        let request = RunRequest {
            scene: args.scene,
            frames: args.frames,
            delta: args.delta,
            inputs: args.inputs,
            report: args.report,
            screenshot: args.screenshot.map(|s| (s.width, s.height)),
        };
        let outcome = tokio::task::spawn_blocking(move || run::run_scene(&project, &request))
            .await
            .map_err(|e| ErrorData::internal_error(format!("the run stopped: {e}"), None))?;
        Ok(match outcome {
            Ok(outcome) => {
                let mut content = vec![ContentBlock::text(
                    serde_json::to_string_pretty(&outcome.report).unwrap_or_default(),
                )];
                if let Some(png) = outcome.png {
                    let data = base64::engine::general_purpose::STANDARD.encode(png);
                    content.push(ContentBlock::image(data, "image/png"));
                }
                CallToolResult::success(content)
            }
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_string())]),
        })
    }
}
