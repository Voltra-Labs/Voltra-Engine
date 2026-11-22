//! The tools that list and read.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_router, ErrorData};
use serde::Deserialize;
use voltra_scene::ComponentRegistry;

use super::{answer, VoltraMcp};
use crate::tools;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AssetsListArgs {
    /// A directory under the asset root, as an asset path ("scenes",
    /// "sprites/tiles"); empty or absent lists the root.
    #[serde(default)]
    pub dir: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SceneArgs {
    /// The scene, as an asset path under the asset root ("scenes/level1.ron").
    pub path: String,
}

#[tool_router(router = discovery_router, vis = "pub(super)")]
impl VoltraMcp {
    #[tool(
        description = "List every component a scene entity can carry: its registered name, whether it references other entities by id, and its template — its default value as RON, to edit and hand back to component tools."
    )]
    fn components_list(&self) -> Result<CallToolResult, ErrorData> {
        answer(tools::discovery::components_list(
            &ComponentRegistry::with_defaults(),
        ))
    }

    #[tool(
        description = "List a directory of the project's asset root: each entry's name, asset path and kind (Directory, Texture, Atlas, Clip, Font, Scene, Animation, Animator, InputMap, SortingLayers, Other)."
    )]
    fn assets_list(
        &self,
        Parameters(args): Parameters<AssetsListArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(tools::discovery::assets_list(
            &self.project,
            args.dir.as_deref(),
        ))
    }

    #[tool(
        description = "List every scene file (which is also every prefab) under the asset root, as asset paths."
    )]
    fn scenes_list(&self) -> Result<CallToolResult, ErrorData> {
        answer(tools::discovery::scenes_list(&self.project))
    }

    #[tool(
        description = "Read a scene file: every entity's id, name, parent id, and each component as the RON the file holds."
    )]
    fn scene_read(
        &self,
        Parameters(args): Parameters<SceneArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(tools::scene_read::scene_read(&self.project, &args.path))
    }
}
