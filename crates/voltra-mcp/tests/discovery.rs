//! The discovery and reading tools, called through a real MCP client over
//! an in-process pipe.

mod common;

use common::{call, client_for, text_of};
use serde_json::{json, Value};

#[tokio::test]
async fn the_server_lists_its_tools() {
    let client = client_for(common::repo_project()).await;
    let tools = client.list_all_tools().await.expect("listed");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    for wanted in [
        "components_list",
        "assets_list",
        "scenes_list",
        "scene_read",
    ] {
        assert!(names.contains(&wanted), "{wanted} in {names:?}");
    }
}

#[tokio::test]
async fn components_come_with_templates_that_name_their_fields() {
    let client = client_for(common::repo_project()).await;
    let answer: Value =
        serde_json::from_str(&text_of(&call(&client, "components_list", json!({})).await))
            .expect("json");
    let find = |name: &str| {
        answer["components"]
            .as_array()
            .expect("a list")
            .iter()
            .find(|c| c["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("{name} is there"))
    };
    assert!(find("Sprite")["template"]
        .as_str()
        .expect("a template")
        .contains("color"));
    assert!(
        find("Parent")["template"].is_null(),
        "structure has no template"
    );
    assert_eq!(find("Joint2D")["references_entities"], true);
}

#[tokio::test]
async fn assets_and_scenes_are_listed_and_a_scene_reads_as_entities() {
    let client = client_for(common::repo_project()).await;
    let root: Value =
        serde_json::from_str(&text_of(&call(&client, "assets_list", json!({})).await))
            .expect("json");
    assert!(root["entries"]
        .as_array()
        .expect("a list")
        .iter()
        .any(|e| e["name"] == "scenes" && e["kind"] == "Directory"));

    let scenes: Value =
        serde_json::from_str(&text_of(&call(&client, "scenes_list", json!({})).await))
            .expect("json");
    assert!(scenes["scenes"]
        .as_array()
        .expect("a list")
        .iter()
        .any(|s| s == "scenes/sandbox.ron"));

    let scene: Value = serde_json::from_str(&text_of(
        &call(
            &client,
            "scene_read",
            json!({ "path": "scenes/sandbox.ron" }),
        )
        .await,
    ))
    .expect("json");
    let entities = scene["entities"].as_array().expect("entities");
    assert!(!entities.is_empty());
    assert!(entities
        .iter()
        .any(|e| e["components"]["Transform"].is_string()));
}

#[tokio::test]
async fn a_path_outside_the_project_is_a_tool_error_not_a_crash() {
    let client = client_for(common::repo_project()).await;
    let result = call(&client, "scene_read", json!({ "path": "../Cargo.toml" })).await;
    assert_eq!(result.is_error, Some(true));
    assert!(
        text_of(&result).contains("not an asset path"),
        "{}",
        text_of(&result)
    );
    let missing = call(
        &client,
        "scene_read",
        json!({ "path": "scenes/missing.ron" }),
    )
    .await;
    assert_eq!(missing.is_error, Some(true));
}
