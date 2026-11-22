//! Running scenes and writing assets through the tools.

mod common;

use common::{call, client_for, scratch_project, text_of};
use serde_json::{json, Value};

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[tokio::test]
async fn a_scene_runs_and_its_bodies_fall_with_a_screenshot_of_the_end() {
    let (project, _root) = scratch_project();
    let client = client_for(project).await;
    let before = parsed(&text_of(
        &call(
            &client,
            "run_scene",
            json!({ "scene": "scenes/sandbox.ron", "frames": 0 }),
        )
        .await,
    ));
    let after_result = call(
        &client,
        "run_scene",
        json!({ "scene": "scenes/sandbox.ron", "frames": 60, "screenshot": { "width": 96, "height": 54 } }),
    )
    .await;
    assert_ne!(
        after_result.is_error,
        Some(true),
        "{}",
        text_of(&after_result)
    );
    let after = parsed(&text_of(&after_result));
    assert_eq!(after["frames"], 60);
    let height = |report: &Value, name: &str| {
        report["entities"]
            .as_array()
            .expect("a summary")
            .iter()
            .find(|e| e["name"] == name)
            .and_then(|e| e["position"][1].as_f64())
    };
    // Some named body in the sandbox ends lower than it started.
    let fell = before["entities"]
        .as_array()
        .expect("a summary")
        .iter()
        .filter_map(|e| e["name"].as_str())
        .any(|name| matches!((height(&before, name), height(&after, name)), (Some(b), Some(a)) if a < b - 0.01));
    assert!(fell, "before {before} after {after}");
    match &after["screenshot"] {
        Value::Object(size) => {
            assert_eq!(size["width"], 96);
            assert!(
                after_result.content.iter().any(|c| c.as_image().is_some()),
                "an image block"
            );
        }
        other => assert!(
            other.as_str().is_some_and(|s| s.contains("adapter")),
            "{other}"
        ),
    }
}

#[tokio::test]
async fn a_run_reports_named_entities_in_full_and_says_which_actions_are_unbound() {
    let (project, _root) = scratch_project();
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
    let run = parsed(&text_of(
        &call(
            &client,
            "run_scene",
            json!({ "scene": "scenes/sandbox.ron", "frames": 5, "report": [id],
                    "inputs": [{ "action": "no such action", "press_at": 1 }] }),
        )
        .await,
    ));
    assert!(run["entities"][0]["ron"]
        .as_str()
        .expect("ron")
        .contains("Transform"));
    assert_eq!(run["unbound_actions"], json!(["no such action"]));
    let too_long = call(
        &client,
        "run_scene",
        json!({ "scene": "scenes/sandbox.ron", "frames": 100000 }),
    )
    .await;
    assert_eq!(too_long.is_error, Some(true));
}

#[tokio::test]
async fn an_asset_is_written_only_when_it_reads_as_what_it_is() {
    let (project, root) = scratch_project();
    let client = client_for(project).await;
    let clip = "(looping: true, tracks: [(property: Rotation, keys: [(time: 0.0, value: Float(0.0)), (time: 1.0, value: Float(6.28))])])";
    let written = call(
        &client,
        "asset_write",
        json!({ "path": "animations/spin.anim.ron", "text": clip }),
    )
    .await;
    assert_ne!(written.is_error, Some(true), "{}", text_of(&written));
    assert!(root.join("assets/animations/spin.anim.ron").is_file());
    let read = parsed(&text_of(
        &call(
            &client,
            "asset_read",
            json!({ "path": "animations/spin.anim.ron" }),
        )
        .await,
    ));
    assert_eq!(read["kind"], "Animation");
    assert_eq!(read["text"], clip);

    let again = call(
        &client,
        "asset_write",
        json!({ "path": "animations/spin.anim.ron", "text": clip }),
    )
    .await;
    assert_eq!(
        again.is_error,
        Some(true),
        "not over a file without overwrite"
    );
    let bad = call(&client, "asset_write", json!({ "path": "animations/bad.anim.ron", "text": "(tracks: [(property: Nope)])", "overwrite": true })).await;
    assert_eq!(bad.is_error, Some(true));
    assert!(
        !root.join("assets/animations/bad.anim.ron").exists(),
        "nothing written"
    );
    let png = call(
        &client,
        "asset_write",
        json!({ "path": "sprites/x.png", "text": "hi" }),
    )
    .await;
    assert_eq!(png.is_error, Some(true), "binary assets are not written");
    let texture = parsed(&text_of(
        &call(
            &client,
            "asset_read",
            json!({ "path": "sprites/checker.png" }),
        )
        .await,
    ));
    assert_eq!(texture["binary"], true);
}

#[tokio::test]
async fn a_profiled_run_says_where_its_frames_went() {
    let (project, _root) = scratch_project();
    let client = client_for(project).await;
    let result = call(
        &client,
        "run_scene",
        json!({ "scene": "scenes/sandbox.ron", "frames": 20, "profile": true }),
    )
    .await;
    assert_ne!(result.is_error, Some(true), "{}", text_of(&result));
    let report = parsed(&text_of(&result));
    let profile = &report["profile"];
    assert_eq!(profile["frames"], 20);
    let paths: Vec<&str> = profile["scopes"]
        .as_array()
        .expect("scopes")
        .iter()
        .filter_map(|scope| scope["path"].as_str())
        .collect();
    assert!(paths.contains(&"update/physics/step/solve"), "{paths:?}");

    let plain = parsed(&text_of(
        &call(
            &client,
            "run_scene",
            json!({ "scene": "scenes/sandbox.ron", "frames": 5 }),
        )
        .await,
    ));
    assert!(plain["profile"].is_null(), "only when asked");
}
