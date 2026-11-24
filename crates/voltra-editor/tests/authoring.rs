//! The editor's authoring windows driven through its bridge: the texture
//! packer, and the Atlas Editor cutting a sheet by grid, renaming and
//! pivoting a region, undoing, and saving the file a diff can check.
//!
//! Launches the editor, so it needs a display and a GPU adapter; ignored by
//! default. Under Xvfb with a software Vulkan driver:
//!
//!   xvfb-run -a cargo test -p voltra-editor --test authoring -- --ignored

mod common;

use std::path::Path;

use common::{call, launch, project};
use serde_json::{json, Value};
use voltra_bridge::Command;

/// Copies one of the engine's sample sprites into the project.
fn sprite(assets: &Path, name: &str, to: &str) {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/sprites")
        .join(name);
    let to = assets.join(to);
    std::fs::create_dir_all(to.parent().expect("a folder")).expect("folders");
    std::fs::copy(from, to).expect("copied");
}

fn names(answer: &Value) -> Vec<String> {
    answer["regions"]
        .as_array()
        .expect("regions")
        .iter()
        .map(|region| region["name"].as_str().expect("a name").to_owned())
        .collect()
}

/// What the Atlas Editor saves for the coin sheet sliced by 16, its third
/// cell renamed and its first standing on its feet.
const EXPECTED: &str = r#"(
    version: 2,
    sheet: Some((64, 16)),
    texture: Some("sprites/coin.png"),
    frames: [
        (x: 0, y: 0, width: 16, height: 16, name: Some("coin_0"), pivot: Some((0.5, 1.0))),
        (x: 16, y: 0, width: 16, height: 16, name: Some("coin_1")),
        (x: 32, y: 0, width: 16, height: 16, name: Some("edge_on")),
        (x: 48, y: 0, width: 16, height: 16, name: Some("coin_3")),
    ],
)
"#;

#[test]
#[ignore = "launches the editor: needs a display and a GPU adapter"]
fn the_packer_and_the_atlas_editor_through_the_bridge() {
    let assets = project("authoring");
    sprite(&assets, "coin.png", "sprites/coin.png");
    sprite(&assets, "coin.png", "sprites/loose/coin.png");
    sprite(&assets, "ground.png", "sprites/loose/tiles/ground.png");
    let (editor, mut client) = launch(&assets);
    let _ = &editor;

    // The packer, as the asset browser runs it.
    let packed = call(
        &mut client,
        Command::PackAtlas {
            folder: "sprites/loose".into(),
            trim: true,
        },
    );
    assert_eq!(packed["atlas"], "sprites/loose.atlas.ron", "{packed}");
    assert_eq!(packed["regions"], 2);
    assert!(assets.join("sprites/loose.png").is_file());
    let text = std::fs::read_to_string(assets.join("sprites/loose.atlas.ron")).expect("written");
    assert!(text.contains(r#"name: Some("tiles/ground")"#), "{text}");

    // A sheet with no atlas yet, cut by a grid.
    let opened = call(
        &mut client,
        Command::AtlasOpen {
            path: "sprites/coin.png".into(),
        },
    );
    assert_eq!(opened["path"], "sprites/coin.atlas.ron", "{opened}");
    assert_eq!(opened["sheet"], json!([64, 16]));
    let sliced = call(
        &mut client,
        Command::AtlasSlice {
            cell: [16, 16],
            offset: [0, 0],
            spacing: [0, 0],
        },
    );
    assert_eq!(names(&sliced), ["coin_0", "coin_1", "coin_2", "coin_3"]);

    // A region renamed, a pivot set, the pivot undone and redone.
    call(
        &mut client,
        Command::AtlasRegion {
            index: Some(2),
            name: Some("edge_on".into()),
            rect: None,
            pivot: None,
            remove: false,
        },
    );
    call(
        &mut client,
        Command::AtlasRegion {
            index: Some(0),
            name: None,
            rect: None,
            pivot: Some([0.5, 1.0]),
            remove: false,
        },
    );
    let undone = call(&mut client, Command::AtlasUndo);
    assert_eq!(undone["regions"][0]["pivot"], Value::Null, "{undone}");
    assert_eq!(
        undone["regions"][2]["name"], "edge_on",
        "only the last edit"
    );
    let redone = call(&mut client, Command::AtlasRedo);
    assert_eq!(redone["regions"][0]["pivot"], json!([0.5, 1.0]));

    // A name another region has is refused, and changes nothing.
    let refused = client.call(
        Command::AtlasRegion {
            index: Some(1),
            name: Some("coin_3".into()),
            rect: None,
            pivot: None,
            remove: false,
        },
        common::WAIT,
    );
    assert!(refused.is_err(), "{refused:?}");

    let saved = call(&mut client, Command::AtlasSave { path: None });
    assert_eq!(saved["unsaved_changes"], false);
    let written =
        std::fs::read_to_string(assets.join("sprites/coin.atlas.ron")).expect("the atlas saved");
    assert_eq!(written, EXPECTED, "the file, diffed:\n{written}");

    // And it opens again as it was saved.
    let reopened = call(
        &mut client,
        Command::AtlasOpen {
            path: "sprites/coin.atlas.ron".into(),
        },
    );
    assert_eq!(names(&reopened), ["coin_0", "coin_1", "edge_on", "coin_3"]);
}
