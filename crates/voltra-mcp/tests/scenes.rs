//! Editing scene files through the tools, against a scratch copy of the
//! repository's assets.

mod common;

use common::{call, client_for, scratch_project, text_of};
use serde_json::{json, Value};

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[tokio::test]
async fn a_scene_is_made_filled_edited_and_read_back() {
    let (project, root) = scratch_project();
    let client = client_for(project).await;
    let made = call(
        &client,
        "scene_create",
        json!({ "path": "scenes/made.ron" }),
    )
    .await;
    assert_ne!(made.is_error, Some(true), "{}", text_of(&made));
    let again = call(
        &client,
        "scene_create",
        json!({ "path": "scenes/made.ron" }),
    )
    .await;
    assert_eq!(again.is_error, Some(true), "not over an existing file");

    // As a client would: the Sprite's template, with its colour changed.
    let components = parsed(&text_of(&call(&client, "components_list", json!({})).await));
    let template = components["components"]
        .as_array()
        .expect("a list")
        .iter()
        .find(|c| c["name"] == "Sprite")
        .and_then(|c| c["template"].as_str())
        .expect("a template")
        .to_owned();
    let red = template.replacen("color:(1.0,1.0,1.0,1.0)", "color:(1.0,0.0,0.0,1.0)", 1);
    assert_ne!(red, template, "the template names its colour: {template}");
    let player = parsed(&text_of(
        &call(
            &client,
            "entity_spawn",
            json!({
                "scene": "scenes/made.ron",
                "name": "Player",
                "components": {
                    "Transform": "(translation: (1.0, 2.0), rotation: 0.0, scale: (1.0, 1.0))",
                    "Sprite": red,
                },
            }),
        )
        .await,
    ));
    assert!(player["components"]["Sprite"]
        .as_str()
        .expect("ron")
        .contains("color:(1.0,0.0,0.0,1.0)"));
    let arm = parsed(&text_of(
        &call(
            &client,
            "entity_spawn",
            json!({ "scene": "scenes/made.ron", "name": "Arm", "parent": "Player",
                    "components": { "Transform": "(translation: (0.5, 0.0), rotation: 0.0, scale: (1.0, 1.0))" } }),
        )
        .await,
    ));
    let arm_id = arm["id"].as_str().expect("an id").to_owned();

    let renamed = call(
        &client,
        "entity_rename",
        json!({ "scene": "scenes/made.ron", "entity": "Player/Arm", "name": "Hand" }),
    )
    .await;
    assert_ne!(renamed.is_error, Some(true), "{}", text_of(&renamed));
    let set = call(
        &client,
        "component_set",
        json!({ "scene": "scenes/made.ron", "entity": arm_id, "component": "PointLight2D", "ron": "(radius: 4.0)" }),
    )
    .await;
    assert_ne!(set.is_error, Some(true), "{}", text_of(&set));

    let scene = parsed(&text_of(
        &call(&client, "scene_read", json!({ "path": "scenes/made.ron" })).await,
    ));
    let entities = scene["entities"].as_array().expect("entities");
    assert_eq!(entities.len(), 2);
    let hand = entities
        .iter()
        .find(|e| e["name"] == "Hand")
        .expect("renamed");
    assert!(hand["components"]["PointLight2D"]
        .as_str()
        .expect("ron")
        .contains("4.0"));
    assert_eq!(
        hand["parent"],
        entities
            .iter()
            .find(|e| e["name"] == "Player")
            .expect("player")["id"]
    );
    assert!(root.join("assets/scenes/made.ron").is_file());
}

#[tokio::test]
async fn a_failed_edit_leaves_the_file_as_it_was() {
    let (project, root) = scratch_project();
    let path = root.join("assets/scenes/sandbox.ron");
    let before = std::fs::read_to_string(&path).expect("there");
    let client = client_for(project).await;
    let scene = parsed(&text_of(
        &call(
            &client,
            "scene_read",
            json!({ "path": "scenes/sandbox.ron" }),
        )
        .await,
    ));
    let id = scene["entities"][0]["id"]
        .as_str()
        .expect("an id")
        .to_owned();

    for (tool, args) in [
        (
            "component_set",
            json!({ "scene": "scenes/sandbox.ron", "entity": id, "component": "Transform", "ron": "(translation: oops)" }),
        ),
        (
            "component_set",
            json!({ "scene": "scenes/sandbox.ron", "entity": id, "component": "Physics", "ron": "()" }),
        ),
        (
            "component_set",
            json!({ "scene": "scenes/sandbox.ron", "entity": id, "component": "Parent", "ron": "()" }),
        ),
        (
            "entity_delete",
            json!({ "scene": "scenes/sandbox.ron", "entity": "Nobody" }),
        ),
        (
            "entity_spawn",
            json!({ "scene": "scenes/sandbox.ron", "components": { "Sprite": 3 } }),
        ),
    ] {
        let result = call(&client, tool, args).await;
        assert_eq!(result.is_error, Some(true), "{tool}: {}", text_of(&result));
    }
    assert_eq!(
        std::fs::read_to_string(&path).expect("there"),
        before,
        "untouched"
    );
}

#[tokio::test]
async fn deleting_takes_the_subtree_and_reparenting_to_nothing_makes_a_root() {
    let (project, _root) = scratch_project();
    let client = client_for(project).await;
    call(
        &client,
        "scene_create",
        json!({ "path": "scenes/tree.ron" }),
    )
    .await;
    for (name, parent) in [("A", None), ("B", Some("A")), ("C", Some("A/B"))] {
        let mut args = json!({ "scene": "scenes/tree.ron", "name": name,
                               "components": { "Transform": "(translation: (0.0, 0.0), rotation: 0.0, scale: (1.0, 1.0))" } });
        if let Some(parent) = parent {
            args["parent"] = json!(parent);
        }
        let spawned = call(&client, "entity_spawn", args).await;
        assert_ne!(spawned.is_error, Some(true), "{}", text_of(&spawned));
    }
    let freed = call(
        &client,
        "entity_reparent",
        json!({ "scene": "scenes/tree.ron", "entity": "A/B/C" }),
    )
    .await;
    assert_ne!(freed.is_error, Some(true), "{}", text_of(&freed));
    let deleted = parsed(&text_of(
        &call(
            &client,
            "entity_delete",
            json!({ "scene": "scenes/tree.ron", "entity": "A" }),
        )
        .await,
    ));
    assert_eq!(deleted["entities_removed"], 2, "A and B; C is a root now");
    let scene = parsed(&text_of(
        &call(&client, "scene_read", json!({ "path": "scenes/tree.ron" })).await,
    ));
    let names: Vec<&str> = scene["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .filter_map(|e| e["name"].as_str())
        .collect();
    assert_eq!(names, ["C"]);
}

#[tokio::test]
async fn a_prefab_is_placed_in_a_scene_linked_to_its_file() {
    let (project, _root) = scratch_project();
    let client = client_for(project).await;
    call(
        &client,
        "scene_create",
        json!({ "path": "scenes/level.ron" }),
    )
    .await;
    let placed = call(
        &client,
        "prefab_instantiate",
        json!({ "scene": "scenes/level.ron", "prefab": "prefabs/coin.ron", "x": 3.0, "y": 1.0 }),
    )
    .await;
    assert_ne!(placed.is_error, Some(true), "{}", text_of(&placed));
    let placed = parsed(&text_of(&placed));
    let root = placed["root"].as_str().expect("a root").to_owned();
    let scene = parsed(&text_of(
        &call(&client, "scene_read", json!({ "path": "scenes/level.ron" })).await,
    ));
    let entity = scene["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .find(|e| e["id"] == root)
        .expect("the root is saved");
    assert!(entity["components"]["PrefabInstance"]
        .as_str()
        .expect("linked")
        .contains("coin.ron"));
    assert!(entity["components"]["Transform"]
        .as_str()
        .expect("placed")
        .contains("3.0"));
    let itself = call(
        &client,
        "prefab_instantiate",
        json!({ "scene": "scenes/level.ron", "prefab": "scenes/level.ron" }),
    )
    .await;
    assert_eq!(itself.is_error, Some(true));
}

#[tokio::test]
async fn a_variant_of_a_prefab_is_written_linked_to_its_base() {
    let (project, _root) = scratch_project();
    let client = client_for(project).await;
    let made = call(
        &client,
        "prefab_variant",
        json!({ "base": "prefabs/coin.ron", "path": "prefabs/gold.prefab.ron" }),
    )
    .await;
    assert_ne!(made.is_error, Some(true), "{}", text_of(&made));
    let variant = parsed(&text_of(
        &call(
            &client,
            "scene_read",
            json!({ "path": "prefabs/gold.prefab.ron" }),
        )
        .await,
    ));
    let root = &variant["entities"][0];
    assert!(root["components"]["PrefabInstance"]
        .as_str()
        .expect("linked to its base")
        .contains("coin.ron"));
    let again = call(
        &client,
        "prefab_variant",
        json!({ "base": "prefabs/coin.ron", "path": "prefabs/gold.prefab.ron" }),
    )
    .await;
    assert_eq!(again.is_error, Some(true), "never an overwrite");
    let itself = call(
        &client,
        "prefab_variant",
        json!({ "base": "prefabs/gold.prefab.ron", "path": "prefabs/coin.ron" }),
    )
    .await;
    assert_eq!(itself.is_error, Some(true));
}
