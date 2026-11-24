//! Prefab mode and the two search boxes, driven through the bridge: a
//! prefab opened from an instance, edited with an undo history of its own,
//! saved, and the instances in the scene it came from following it (but
//! for what each overrides); the hierarchy searched by name and by
//! component, the assets by name and by kind.
//!
//! Launches the editor, so it needs a display and a GPU adapter; ignored by
//! default. Under Xvfb with a software Vulkan driver:
//!
//!   xvfb-run -a cargo test -p voltra-editor --test prefab_mode -- --ignored

mod common;

use std::path::Path;

use common::{call, launch, project, wait_for_scene, WAIT};
use serde_json::Value;
use voltra_bridge::Command;

/// A coin prefab — a red root and a shine under it — and a level holding
/// two instances of it, each with a name of its own and the second
/// overriding its colour to blue, and a crate beside them.
fn level(assets: &Path) {
    let file = |path: &str, text: &str| {
        let to = assets.join(path);
        std::fs::create_dir_all(to.parent().expect("a folder")).expect("folders");
        std::fs::write(to, text).expect("written");
    };
    let transform = r#""Transform": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0))"#;
    let red = r#""Sprite": (color:(1.0,0.0,0.0,1.0),sort_order:0)"#;
    let white = r#""Sprite": (color:(1.0,1.0,1.0,1.0),sort_order:1)"#;
    let blue = r#""Sprite": (color:(0.0,0.0,1.0,1.0),sort_order:0)"#;
    let coin = "019fe500-0000-7000-8000-0000000000c1";
    let shine = "019fe500-0000-7000-8000-0000000000c2";
    file(
        "prefabs/coin.prefab.ron",
        &format!(
            r#"(version: 1, entities: [
                (id: "{coin}", components: {{"Name": "Coin", {transform}, {red}}}),
                (id: "{shine}", components: {{"Name": "Shine", "Parent": "{coin}", {transform}, {white}}}),
            ])"#
        ),
    );
    let instance = |n: u32, name: &str, sprite: &str, overridden: &str| {
        let root = format!("019fe500-0000-7000-8000-00000000a{n}01");
        let child = format!("019fe500-0000-7000-8000-00000000a{n}02");
        format!(
            r#"(id: "{root}", components: {{"Name": "{name}", {transform}, {sprite},
                  "PrefabInstance": (source: Path("prefabs/coin.prefab.ron")),
                  "PrefabPart": (record: "{coin}", overridden: [{overridden}])}}),
               (id: "{child}", components: {{"Name": "Shine", "Parent": "{root}", {transform}, {white},
                  "PrefabPart": (record: "{shine}")}}),"#
        )
    };
    file(
        "scenes/level.ron",
        &format!(
            r#"(version: 1, entities: [
                {}
                {}
                (id: "019fe500-0000-7000-8000-0000000000b1", components: {{"Name": "Crate", {transform}, {red},
                  "Collider": Box(half_extents:(0.5,0.5))}}),
            ])"#,
            instance(1, "Coin A", red, r#""Name""#),
            instance(2, "Coin B", blue, r#""Name", "Sprite""#),
        ),
    );
    let sprite = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    for (from, to) in [
        ("sprites/coin.png", "sprites/coin.png"),
        ("audio/coin.wav", "audio/coin.wav"),
    ] {
        let to = assets.join(to);
        std::fs::create_dir_all(to.parent().expect("a folder")).expect("folders");
        std::fs::copy(sprite.join(from), to).expect("copied");
    }
}

/// The names in a search's answer under `key`.
fn names(answer: &Value, key: &str) -> Vec<String> {
    answer[key]
        .as_array()
        .unwrap_or_else(|| panic!("no {key} in {answer}"))
        .iter()
        .map(|row| {
            row["name"]
                .as_str()
                .or_else(|| row["path"].as_str())
                .expect("a name or a path")
                .to_owned()
        })
        .collect()
}

/// Every entity named `name` in the open document, as `read` answers them.
fn named(read: &Value, name: &str) -> Vec<Value> {
    read["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .filter(|e| e["name"] == name)
        .cloned()
        .collect()
}

fn sprite_of(read: &Value, name: &str) -> String {
    let found = named(read, name);
    assert_eq!(found.len(), 1, "one {name} in {read}");
    found[0]["components"]["Sprite"]
        .as_str()
        .expect("a sprite")
        .to_owned()
}

#[test]
#[ignore = "launches the editor: needs a display and a GPU adapter"]
fn a_prefab_edited_in_its_mode_and_the_searches_through_the_bridge() {
    let assets = project("prefab-mode");
    level(&assets);
    let (editor, mut client) = launch(&assets);
    let _ = &editor;
    wait_for_scene(&mut client);
    let opened = call(
        &mut client,
        Command::Open {
            path: "scenes/level.ron".into(),
            discard_changes: false,
        },
    );
    assert_eq!(opened["entities"], 5, "{opened}");
    // An edit of the scene's own, for its history to hold on the way back.
    call(
        &mut client,
        Command::Rename {
            entity: "Crate".into(),
            name: "Box".into(),
        },
    );

    // The hierarchy: by name, with the rows above a match; by component;
    // both; and a component nothing is called.
    let search = |client: &mut _, query: &str| {
        call(
            client,
            Command::HierarchySearch {
                query: query.into(),
            },
        )
    };
    let shine = search(&mut client, "SHINE");
    assert_eq!(names(&shine, "matches"), ["Shine", "Shine"], "{shine}");
    assert_eq!(names(&shine, "parents"), ["Coin A", "Coin B"], "{shine}");
    let solid = search(&mut client, "t:collider");
    assert_eq!(names(&solid, "matches"), ["Box"], "{solid}");
    assert!(names(&solid, "parents").is_empty());
    let coins = search(&mut client, "t:Sprite t:PrefabInstance coin");
    assert_eq!(names(&coins, "matches"), ["Coin A", "Coin B"], "{coins}");
    assert!(client
        .call(
            Command::HierarchySearch {
                query: "t:Spirte".into()
            },
            WAIT
        )
        .is_err());
    assert_eq!(names(&search(&mut client, ""), "matches").len(), 5);

    // The assets: by name in every folder, and by kind.
    let assets_search = |client: &mut _, query: &str| {
        names(
            &call(
                client,
                Command::AssetSearch {
                    query: query.into(),
                },
            ),
            "matches",
        )
    };
    assert_eq!(
        assets_search(&mut client, "coin"),
        [
            "audio/coin.wav",
            "prefabs/coin.prefab.ron",
            "sprites/coin.png"
        ]
    );
    assert_eq!(
        assets_search(&mut client, "t:prefab"),
        ["prefabs/coin.prefab.ron"]
    );
    assert_eq!(
        assets_search(&mut client, "t:scene"),
        [
            "prefabs/coin.prefab.ron",
            "scenes/box.ron",
            "scenes/level.ron"
        ]
    );
    assert_eq!(
        assets_search(&mut client, "coin t:sound t:texture"),
        ["audio/coin.wav", "sprites/coin.png"]
    );
    assert!(client
        .call(
            Command::AssetSearch {
                query: "t:texure".into()
            },
            WAIT
        )
        .is_err());

    // Into the prefab, from an instance of it: the prefab alone, with an
    // empty history of its own.
    let open = call(
        &mut client,
        Command::PrefabOpen {
            path: None,
            entity: Some("Coin A".into()),
        },
    );
    assert_eq!(open["editing"], "prefabs/coin.prefab.ron", "{open}");
    assert_eq!(open["from"], "level.ron");
    assert_eq!(open["entities"], 2);
    let status = call(&mut client, Command::Status);
    assert_eq!(status["scene"], "prefabs/coin.prefab.ron", "{status}");
    assert_eq!(status["prefab_trail"], serde_json::json!(["level.ron"]));
    assert_eq!(
        status["undo"],
        Value::Null,
        "the scene's history stays there"
    );
    assert!(
        client
            .call(
                Command::Open {
                    path: "scenes/box.ron".into(),
                    discard_changes: true
                },
                WAIT
            )
            .is_err(),
        "nothing replaces the scene set aside"
    );

    // Edited, undone, redone — in the prefab's own history.
    call(
        &mut client,
        Command::ComponentSet {
            entity: "Coin".into(),
            component: "Sprite".into(),
            ron: "(color:(0.0,1.0,0.0,1.0),sort_order:0)".into(),
        },
    );
    call(&mut client, Command::Undo);
    let read = call(&mut client, Command::Read);
    assert!(sprite_of(&read, "Coin").contains("color:(1.0,0.0,0.0,1.0)"));
    call(&mut client, Command::Redo);
    call(
        &mut client,
        Command::Spawn {
            name: Some("Glint".into()),
            parent: Some("Coin".into()),
            components: [(
                "Transform".to_owned(),
                "(translation:(0.2,0.2),rotation:0.0,scale:(1.0,1.0))".to_owned(),
            )]
            .into_iter()
            .collect(),
        },
    );
    assert!(
        client
            .call(
                Command::PrefabClose {
                    discard_changes: false
                },
                WAIT
            )
            .is_err(),
        "unsaved changes keep it open"
    );
    assert!(
        client
            .call(
                Command::Save {
                    path: Some("prefabs/copy.prefab.ron".into())
                },
                WAIT
            )
            .is_err(),
        "a prefab is saved to its own file"
    );
    let saved = call(&mut client, Command::Save { path: None });
    assert_eq!(saved["saved"], "prefabs/coin.prefab.ron");
    let text = std::fs::read_to_string(assets.join("prefabs/coin.prefab.ron")).expect("saved");
    assert!(text.contains("Glint"), "{text}");

    // Back: the scene and its history as they were, its instances following
    // the prefab but for the blue one's own colour.
    let back = call(
        &mut client,
        Command::PrefabClose {
            discard_changes: false,
        },
    );
    assert_eq!(back["scene"], "scenes/level.ron", "{back}");
    assert_eq!(back["prefab_mode"], false);
    let status = call(&mut client, Command::Status);
    assert_eq!(status["undo"], "AI: rename", "{status}");
    assert_eq!(status["unsaved_changes"], true);
    let read = call(&mut client, Command::Read);
    assert!(
        sprite_of(&read, "Coin A").contains("color:(0.0,1.0,0.0,1.0)"),
        "{read}"
    );
    assert!(sprite_of(&read, "Coin B").contains("color:(0.0,0.0,1.0,1.0)"));
    assert_eq!(named(&read, "Glint").len(), 2, "both gained the new child");
    call(&mut client, Command::Undo);
    let read = call(&mut client, Command::Read);
    assert_eq!(named(&read, "Crate").len(), 1, "the scene's own undo");

    // Changes dropped on the way back reach nothing.
    call(
        &mut client,
        Command::PrefabOpen {
            path: Some("prefabs/coin.prefab.ron".into()),
            entity: None,
        },
    );
    call(
        &mut client,
        Command::ComponentSet {
            entity: "Coin".into(),
            component: "Sprite".into(),
            ron: "(color:(1.0,1.0,0.0,1.0),sort_order:0)".into(),
        },
    );
    call(
        &mut client,
        Command::PrefabClose {
            discard_changes: true,
        },
    );
    let read = call(&mut client, Command::Read);
    assert!(sprite_of(&read, "Coin A").contains("color:(0.0,1.0,0.0,1.0)"));
}
