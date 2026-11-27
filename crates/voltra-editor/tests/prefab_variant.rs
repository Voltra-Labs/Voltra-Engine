//! Variants and nested prefabs, driven through the bridge: a variant of a
//! coin made and edited in prefab mode, its edit kept as the variant's
//! override; then the coin edited, and every instance of it — plain, of the
//! variant, nested in a row prefab nobody saves again — following it.
//!
//! Launches the editor, so it needs a display and a GPU adapter; ignored by
//! default. Under Xvfb with a software Vulkan driver:
//!
//!   xvfb-run -a cargo test -p voltra-editor --test prefab_variant -- --ignored

mod common;

use std::path::Path;

use common::{call, launch, project, wait_for_scene};
use serde_json::Value;
use voltra_bridge::Command;

const TRANSFORM: &str = r#""Transform": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0))"#;
const RED: &str = r#""Sprite": (color:(1.0,0.0,0.0,1.0),sort_order:0)"#;
const BLUE: &str = r#""Sprite": (color:(0.0,0.0,1.0,1.0),sort_order:0)"#;
const WHITE: &str = r#""Sprite": (color:(1.0,1.0,1.0,1.0),sort_order:1)"#;
const COIN: &str = "019fe500-0000-7000-8000-0000000000c1";
const SHINE: &str = "019fe500-0000-7000-8000-0000000000c2";

fn file(assets: &Path, path: &str, text: &str) {
    let to = assets.join(path);
    std::fs::create_dir_all(to.parent().expect("a folder")).expect("folders");
    std::fs::write(to, text).expect("written");
}

/// The coin: a red root and a white shine under it.
fn coin(assets: &Path) {
    file(
        assets,
        "prefabs/coin.prefab.ron",
        &format!(
            r#"(version: 1, entities: [
                (id: "{COIN}", components: {{"Name": "Coin", {TRANSFORM}, {RED}}}),
                (id: "{SHINE}", components: {{"Name": "Shine", "Parent": "{COIN}", {TRANSFORM}, {WHITE}}}),
            ])"#
        ),
    );
}

/// A row prefab holding a coin, stored as an instance of it; and a level
/// holding a plain coin, a coin of the variant (saved before it had parts,
/// so adopted by name) and a row.
fn level(assets: &Path) {
    let id = |n: u32| format!("019fe500-0000-7000-8000-00000000{n:04}");
    file(
        assets,
        "prefabs/row.prefab.ron",
        &format!(
            r#"(version: 1, entities: [
                (id: "{r0}", components: {{"Name": "Row", {TRANSFORM}}}),
                (id: "{r1}", components: {{"Name": "Coin", "Parent": "{r0}", {TRANSFORM}, {RED},
                  "PrefabInstance": (source: Path("prefabs/coin.prefab.ron")),
                  "PrefabPart": (record: "{COIN}")}}),
                (id: "{r2}", components: {{"Name": "Shine", "Parent": "{r1}", {TRANSFORM}, {WHITE},
                  "PrefabPart": (record: "{SHINE}")}}),
            ])"#,
            r0 = id(1),
            r1 = id(2),
            r2 = id(3),
        ),
    );
    file(
        assets,
        "scenes/level.ron",
        &format!(
            r#"(version: 1, entities: [
                (id: "{a}", components: {{"Name": "Plain", {TRANSFORM}, {RED},
                  "PrefabInstance": (source: Path("prefabs/coin.prefab.ron")),
                  "PrefabPart": (record: "{COIN}", overridden: ["Name"])}}),
                (id: "{b}", components: {{"Name": "Shine", "Parent": "{a}", {TRANSFORM}, {WHITE},
                  "PrefabPart": (record: "{SHINE}")}}),
                (id: "{c}", components: {{"Name": "Varied", {TRANSFORM}, {BLUE},
                  "PrefabInstance": (source: Path("prefabs/coin Variant.prefab.ron"))}}),
                (id: "{d}", components: {{"Name": "Shine", "Parent": "{c}", {TRANSFORM}, {WHITE}}}),
                (id: "{e}", components: {{"Name": "Row", {TRANSFORM},
                  "PrefabInstance": (source: Path("prefabs/row.prefab.ron")),
                  "PrefabPart": (record: "{r0}")}}),
                (id: "{f}", components: {{"Name": "Coin", "Parent": "{e}", {TRANSFORM}, {RED},
                  "PrefabInstance": (source: Path("prefabs/coin.prefab.ron")),
                  "PrefabPart": (record: "{r1}")}}),
                (id: "{g}", components: {{"Name": "Shine", "Parent": "{f}", {TRANSFORM}, {WHITE},
                  "PrefabPart": (record: "{r2}")}}),
            ])"#,
            a = id(11),
            b = id(12),
            c = id(13),
            d = id(14),
            e = id(15),
            f = id(16),
            g = id(17),
            r0 = id(1),
            r1 = id(2),
            r2 = id(3),
        ),
    );
}

/// The sprites of every entity named `name`, as `read` answers them.
fn sprites(read: &Value, name: &str) -> Vec<String> {
    read["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .filter(|e| e["name"] == name)
        .map(|e| {
            e["components"]["Sprite2D"]
                .as_str()
                .expect("a sprite")
                .to_owned()
        })
        .collect()
}

#[test]
#[ignore = "launches the editor: needs a display and a GPU adapter"]
fn a_variant_and_a_nested_prefab_follow_their_base_through_the_bridge() {
    let (_project, assets) = project("prefab-variant");
    coin(&assets);
    let (editor, mut client) = launch(&assets);
    let _ = &editor;
    wait_for_scene(&mut client);

    // A variant of the coin, beside it, linked to it.
    let made = call(
        &mut client,
        Command::PrefabVariant {
            path: "prefabs/coin.prefab.ron".into(),
        },
    );
    assert_eq!(made["variant"], "prefabs/coin Variant.prefab.ron", "{made}");
    let text =
        std::fs::read_to_string(assets.join("prefabs/coin Variant.prefab.ron")).expect("written");
    assert!(text.contains("prefabs/coin.prefab.ron"), "{text}");
    let again = call(
        &mut client,
        Command::PrefabVariant {
            path: "prefabs/coin.prefab.ron".into(),
        },
    );
    assert_eq!(
        again["variant"], "prefabs/coin Variant 1.prefab.ron",
        "never an overwrite"
    );

    // Edited in its mode: blue, recorded as the variant's override.
    let open = call(
        &mut client,
        Command::PrefabOpen {
            path: Some("prefabs/coin Variant.prefab.ron".into()),
            entity: None,
        },
    );
    assert_eq!(open["entities"], 2, "{open}");
    call(
        &mut client,
        Command::ComponentSet {
            entity: "Coin".into(),
            component: "Sprite2D".into(),
            ron: "(color:(0.0,0.0,1.0,1.0),sort_order:0)".into(),
        },
    );
    call(&mut client, Command::Save { path: None });
    let text =
        std::fs::read_to_string(assets.join("prefabs/coin Variant.prefab.ron")).expect("saved");
    assert!(text.contains("overridden"), "{text}");
    call(
        &mut client,
        Command::PrefabClose {
            discard_changes: false,
        },
    );

    level(&assets);
    let row_before = std::fs::read(assets.join("prefabs/row.prefab.ron")).expect("written");
    let opened = call(
        &mut client,
        Command::Open {
            path: "scenes/level.ron".into(),
            discard_changes: true,
        },
    );
    assert_eq!(opened["entities"], 7, "{opened}");

    // The coin edited: its shine goes green.
    call(
        &mut client,
        Command::PrefabOpen {
            path: None,
            entity: Some("Plain".into()),
        },
    );
    call(
        &mut client,
        Command::ComponentSet {
            entity: "Coin/Shine".into(),
            component: "Sprite2D".into(),
            ron: "(color:(0.0,1.0,0.0,1.0),sort_order:1)".into(),
        },
    );
    call(&mut client, Command::Save { path: None });
    call(
        &mut client,
        Command::PrefabClose {
            discard_changes: false,
        },
    );

    // Every shine followed — the plain coin's, the variant's, the row's —
    // the variant's blue stayed, and the row was never saved again.
    let read = call(&mut client, Command::Read);
    let shines = sprites(&read, "Shine");
    assert_eq!(shines.len(), 3, "{read}");
    for shine in &shines {
        assert!(shine.contains("color:(0.0,1.0,0.0,1.0)"), "{read}");
    }
    assert!(sprites(&read, "Varied")[0].contains("color:(0.0,0.0,1.0,1.0)"));
    assert!(sprites(&read, "Plain")[0].contains("color:(1.0,0.0,0.0,1.0)"));
    assert_eq!(
        std::fs::read(assets.join("prefabs/row.prefab.ron")).expect("there"),
        row_before
    );
}
