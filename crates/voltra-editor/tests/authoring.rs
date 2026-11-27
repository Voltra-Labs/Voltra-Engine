//! The editor's authoring windows driven through its bridge: the texture
//! packer; the Atlas Editor cutting a sheet by grid, renaming and pivoting a
//! region, undoing, and saving the file a diff can check; and the tilemap
//! tools — a 3×2 stamp off the palette, a flood fill of a closed area, the
//! eyedropper — each stroke one undo entry.
//!
//! Launches the editor, so it needs a display and a GPU adapter; ignored by
//! default. Under Xvfb with a software Vulkan driver:
//!
//!   xvfb-run -a cargo test -p voltra-editor --test authoring -- --ignored

mod common;

use std::path::Path;

use common::{call, launch, project, wait_for_scene};
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
    let (_project, assets) = project("authoring");
    sprite(&assets, "coin.png", "sprites/coin.png");
    sprite(&assets, "coin.png", "sprites/loose/coin.png");
    sprite(&assets, "ground.png", "sprites/loose/tiles/ground.png");
    let (editor, mut client) = launch(&assets);
    let _ = &editor;
    wait_for_scene(&mut client);

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

/// The frames of a `TileRead`'s rows, `None` for an empty cell.
fn frames(answer: &Value) -> Vec<Vec<Option<u64>>> {
    answer["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| {
            row.as_array()
                .expect("a row")
                .iter()
                .map(|cell| cell["frame"].as_u64())
                .collect()
        })
        .collect()
}

fn read(
    client: &mut voltra_bridge::BridgeClient,
    from: [i32; 2],
    to: [i32; 2],
) -> Vec<Vec<Option<u64>>> {
    frames(&call(
        client,
        Command::TileRead {
            layer: "Ground".into(),
            from,
            to,
        },
    ))
}

fn paint(
    client: &mut voltra_bridge::BridgeClient,
    tool: &str,
    cells: &[[i32; 2]],
    pick: bool,
) -> Value {
    call(
        client,
        Command::TilePaint {
            layer: "Ground".into(),
            tool: tool.into(),
            cells: cells.to_vec(),
            pick,
            terrain: None,
        },
    )
}

fn pick(client: &mut voltra_bridge::BridgeClient, from: [u32; 2], to: [u32; 2]) -> Value {
    call(
        client,
        Command::TilePick {
            layer: "Ground".into(),
            from,
            to,
        },
    )
}

#[test]
#[ignore = "launches the editor: needs a display and a GPU adapter"]
fn the_tilemap_tools_through_the_bridge() {
    let (_project, assets) = project("tiles");
    sprite(&assets, "ground.png", "sprites/ground.png");
    // The ground sheet cut into 8×8 tiles: a palette 8 across and 2 down,
    // frames 0..8 on top and 8..16 below.
    std::fs::write(
        assets.join("sprites/tiles.atlas.ron"),
        r#"(version: 1, texture: Some("sprites/ground.png"), grid: Some((cell: (8, 8), columns: 8, rows: 2)))"#,
    )
    .expect("the tileset");
    let (editor, mut client) = launch(&assets);
    let _ = &editor;
    wait_for_scene(&mut client);
    call(
        &mut client,
        Command::Spawn {
            name: Some("Ground".into()),
            parent: None,
            components: [(
                "Tilemap".to_owned(),
                r#"(tile_size:(0.5,0.5),texture:Some(Path("sprites/ground.png")),atlas:Some(Path("sprites/tiles.atlas.ron")))"#
                    .to_owned(),
            )]
            .into_iter()
            .collect(),
        },
    );

    // A 3×2 block off the palette, stamped with its bottom-left on (0, 0).
    let stamp = pick(&mut client, [1, 0], [3, 1]);
    assert_eq!(stamp["size"], json!([3, 2]), "{stamp}");
    paint(&mut client, "brush", &[[0, 0]], false);
    assert_eq!(
        read(&mut client, [0, 0], [2, 1]),
        [[Some(1), Some(2), Some(3)], [Some(9), Some(10), Some(11)]]
    );
    let undone = call(&mut client, Command::Undo);
    assert!(undone["undone"].is_string(), "{undone}");
    assert_eq!(
        read(&mut client, [0, 0], [2, 1]),
        [[None, None, None], [None, None, None]],
        "the stamp was one entry"
    );
    call(&mut client, Command::Redo);

    // A closed ring of frame 0, drawn as one brush stroke, then filled.
    pick(&mut client, [0, 0], [0, 0]);
    paint(
        &mut client,
        "brush",
        &[[10, 0], [14, 0], [14, 4], [10, 4], [10, 0]],
        false,
    );
    pick(&mut client, [5, 0], [5, 0]);
    paint(&mut client, "fill", &[[12, 2]], false);
    let ring = Some(0);
    let fill = Some(5);
    assert_eq!(
        read(&mut client, [9, 0], [15, 4]),
        [
            [None, ring, ring, ring, ring, ring, None],
            [None, ring, fill, fill, fill, ring, None],
            [None, ring, fill, fill, fill, ring, None],
            [None, ring, fill, fill, fill, ring, None],
            [None, ring, ring, ring, ring, ring, None],
        ],
        "the inside filled, the ring and the outside not"
    );

    // The eyedropper picks a tile of the stamp back off the map, painting
    // nothing and adding no undo entry: the next undo is still the fill.
    let picked = paint(&mut client, "brush", &[[1, 1]], true);
    assert_eq!(picked["stroke"], Value::Null, "{picked}");
    assert_eq!(picked["stamp"]["rows"][0][0]["frame"], 2, "{picked}");
    call(&mut client, Command::Undo);
    assert_eq!(
        read(&mut client, [12, 2], [12, 2]),
        [[None]],
        "the fill undone"
    );

    // A picked tile turned and painted lands turned.
    let turned = call(
        &mut client,
        Command::TileStamp {
            turns: 1,
            flip_x: false,
            flip_y: false,
        },
    );
    assert_eq!(turned["rows"][0][0]["transpose"], true, "{turned}");
    paint(&mut client, "brush", &[[20, 20]], false);
    let cell = call(
        &mut client,
        Command::TileRead {
            layer: "Ground".into(),
            from: [20, 20],
            to: [20, 20],
        },
    );
    assert_eq!(cell["rows"][0][0]["frame"], 2);
    assert_eq!(cell["rows"][0][0]["transpose"], true);
    assert_eq!(
        cell["rows"][0][0]["flip_x"], true,
        "a quarter turn clockwise"
    );
}
