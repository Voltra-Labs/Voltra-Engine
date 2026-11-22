//! The tools that read and write asset files.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_router, ErrorData};
use serde::Deserialize;

use super::discovery::SceneArgs;
use super::{answer, VoltraMcp};
use crate::tools::assets;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WriteArgs {
    /// The asset, as an asset path; its name says what it is
    /// ("x.anim.ron", "x.animator.ron", "x.input.ron", "x.atlas.ron",
    /// "x.layers.ron", or a scene "x.ron").
    pub path: String,
    /// The whole file, as RON.
    pub text: String,
    /// Replace a file that is there. Off by default.
    #[serde(default)]
    pub overwrite: bool,
}

#[tool_router(router = asset_router, vis = "pub(super)")]
impl VoltraMcp {
    #[tool(
        description = "Read an asset file: the text of a RON (or other text) asset, or the kind and size of a binary one."
    )]
    fn asset_read(
        &self,
        Parameters(args): Parameters<SceneArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(assets::asset_read(&self.project, &args.path))
    }

    #[tool(
        description = "Write a RON asset file — a scene, an animation clip, a state machine, an input map, an atlas or sorting layers, by its name — once the text reads as that kind of asset; atomically. Refuses to replace an existing file unless overwrite is set. Destructive with overwrite."
    )]
    fn asset_write(
        &self,
        Parameters(args): Parameters<WriteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(assets::asset_write(
            &self.project,
            &args.path,
            &args.text,
            args.overwrite,
        ))
    }
}
