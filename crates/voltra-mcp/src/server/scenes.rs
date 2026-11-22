//! The tools that edit scene files.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_router, ErrorData};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::discovery::SceneArgs;
use super::{answer, VoltraMcp};
use crate::tools::scene_edit;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SpawnArgs {
    /// The scene, as an asset path.
    pub scene: String,
    /// A name for the entity.
    #[serde(default)]
    pub name: Option<String>,
    /// The entity to put it under: an id or a path of names.
    #[serde(default)]
    pub parent: Option<String>,
    /// Its components: registered name → RON, as `components_list`'s
    /// templates are written ({"Transform": "(translation: (0.0, 1.0), rotation: 0.0, scale: (1.0, 1.0))"}).
    #[serde(default)]
    pub components: Map<String, Value>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EntityArgs {
    /// The scene, as an asset path.
    pub scene: String,
    /// The entity: its id, or a path of names from a root ("Player/Arm").
    pub entity: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReparentArgs {
    pub scene: String,
    pub entity: String,
    /// The new parent: an id or a path of names; absent makes it a root.
    #[serde(default)]
    pub parent: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RenameArgs {
    pub scene: String,
    pub entity: String,
    /// The new name; empty takes the name off.
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ComponentSetArgs {
    pub scene: String,
    pub entity: String,
    /// The component's registered name ("Sprite").
    pub component: String,
    /// Its value as RON, the whole component — start from its template.
    pub ron: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ComponentArgs {
    pub scene: String,
    pub entity: String,
    pub component: String,
}

#[tool_router(router = scene_router, vis = "pub(super)")]
impl VoltraMcp {
    #[tool(description = "Create a new, empty scene file. Refused if the file exists.")]
    fn scene_create(
        &self,
        Parameters(args): Parameters<SceneArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(scene_edit::scene_create(&self.project, &args.path))
    }

    #[tool(
        description = "Add an entity to a scene, with components given as RON by registered name, optionally named and under a parent. Saves the scene; answers the new entity's id and components."
    )]
    fn entity_spawn(
        &self,
        Parameters(args): Parameters<SpawnArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(scene_edit::entity_spawn(
            &self.project,
            &args.scene,
            args.name.as_deref(),
            args.parent.as_deref(),
            &args.components,
        ))
    }

    #[tool(
        description = "Delete an entity and everything under it from a scene. Destructive: saves the scene."
    )]
    fn entity_delete(
        &self,
        Parameters(args): Parameters<EntityArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(scene_edit::entity_delete(
            &self.project,
            &args.scene,
            &args.entity,
        ))
    }

    #[tool(
        description = "Put an entity under another, or make it a root when no parent is given. Saves the scene."
    )]
    fn entity_reparent(
        &self,
        Parameters(args): Parameters<ReparentArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(scene_edit::entity_reparent(
            &self.project,
            &args.scene,
            &args.entity,
            args.parent.as_deref(),
        ))
    }

    #[tool(description = "Rename an entity; an empty name takes it off. Saves the scene.")]
    fn entity_rename(
        &self,
        Parameters(args): Parameters<RenameArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(scene_edit::entity_rename(
            &self.project,
            &args.scene,
            &args.entity,
            &args.name,
        ))
    }

    #[tool(
        description = "Set one component on an entity from RON — the whole value, as components_list's template writes it. Adds the component if it is missing. Saves the scene; answers the entity's components."
    )]
    fn component_set(
        &self,
        Parameters(args): Parameters<ComponentSetArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(scene_edit::component_set(
            &self.project,
            &args.scene,
            &args.entity,
            &args.component,
            &args.ron,
        ))
    }

    #[tool(description = "Remove one component from an entity. Saves the scene.")]
    fn component_remove(
        &self,
        Parameters(args): Parameters<ComponentArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        answer(scene_edit::component_remove(
            &self.project,
            &args.scene,
            &args.entity,
            &args.component,
        ))
    }
}
