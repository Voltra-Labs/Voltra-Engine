//! The tools that build, test and package, and the one that places a
//! prefab.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_router, ErrorData};
use serde::Deserialize;
use voltra_render::glam::Vec2;

use super::{answer, VoltraMcp};
use crate::tools::project::{run_cargo, CargoJob};
use crate::tools::{prefab, ToolResult};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BuildArgs {
    /// One package ("voltra-core", a game's crate); absent builds the
    /// workspace.
    #[serde(default)]
    pub package: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TestArgs {
    /// One package; absent tests the workspace.
    #[serde(default)]
    pub package: Option<String>,
    /// Only tests whose names contain this.
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DistArgs {
    /// A build manifest in the project, relative to its root; absent is
    /// `build.ron`.
    #[serde(default)]
    pub manifest: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InstantiateArgs {
    /// The scene to place it in, as an asset path.
    pub scene: String,
    /// The prefab — any scene file — as an asset path.
    pub prefab: String,
    /// Where its root goes, in world units.
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// An entity to put it under: an id or a path of names.
    #[serde(default)]
    pub parent: Option<String>,
}

impl VoltraMcp {
    /// `job`, run off the async runtime.
    async fn cargo(&self, job: CargoJob) -> ToolResult {
        let project = self.project.clone();
        tokio::task::spawn_blocking(move || run_cargo(&project, &job))
            .await
            .unwrap_or_else(|e| Err(crate::tools::ToolError(format!("cargo stopped: {e}"))))
    }
}

#[tool_router(router = project_router, vis = "pub(super)")]
impl VoltraMcp {
    #[tool(
        description = "Build the workspace, or one package, with cargo. Answers success, time taken and the end of the output."
    )]
    async fn project_build(
        &self,
        Parameters(args): Parameters<BuildArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.cargo(CargoJob::Build {
                package: args.package,
            })
            .await,
        )
    }

    #[tool(
        description = "Run the tests of the workspace, or one package, optionally only those whose names contain a filter. Answers success, time taken and the end of the output."
    )]
    async fn project_test(
        &self,
        Parameters(args): Parameters<TestArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.cargo(CargoJob::Test {
                package: args.package,
                filter: args.filter,
            })
            .await,
        )
    }

    #[tool(
        description = "Package the game with `cargo xtask dist`, from build.ron or another manifest. Writes into dist/; destructive there."
    )]
    async fn project_dist(
        &self,
        Parameters(args): Parameters<DistArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(
            self.cargo(CargoJob::Dist {
                manifest: args.manifest,
            })
            .await,
        )
    }

    #[tool(
        description = "Place an instance of a prefab in a scene, linked to the prefab file as the editor's drop does, its root at x, y and optionally under a parent. Saves the scene."
    )]
    fn prefab_instantiate(
        &self,
        Parameters(args): Parameters<InstantiateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(prefab::prefab_instantiate(
            &self.project,
            &args.scene,
            &args.prefab,
            Vec2::new(args.x, args.y),
            args.parent.as_deref(),
        ))
    }
}
