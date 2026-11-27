//! The editor's comforts driven through its bridge, as a tool drives it:
//! grid snapping on a drag, the Console's level filter, and a project
//! setting saved as its file and picked up live.
//!
//! Launches the editor, so it needs a display and a GPU adapter; ignored by
//! default. Under Xvfb with a software Vulkan driver:
//!
//!   xvfb-run -a cargo test -p voltra-editor --test bridge -- --ignored

mod common;

use std::time::{Duration, Instant};

use common::{call, launch, project, WAIT};
use serde_json::Value;
use voltra_bridge::{BridgeClient, Command};

fn transform_of(answer: &Value) -> String {
    answer["components"]["Transform2D"]
        .as_str()
        .expect("a transform")
        .to_owned()
}

fn console(client: &mut BridgeClient, level: &str, filter: &str) -> Vec<Value> {
    let answer = call(
        client,
        Command::Console {
            level: Some(level.to_owned()),
            filter: Some(filter.to_owned()),
            last: None,
        },
    );
    answer["lines"].as_array().cloned().unwrap_or_default()
}

/// Waits until `ready` holds of the console's lines, or `WAIT` passes.
fn console_until(
    client: &mut BridgeClient,
    level: &str,
    filter: &str,
    ready: impl Fn(&[Value]) -> bool,
) -> Vec<Value> {
    let start = Instant::now();
    loop {
        let lines = console(client, level, filter);
        if ready(&lines) || start.elapsed() > WAIT {
            return lines;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
#[ignore = "launches the editor: needs a display and a GPU adapter"]
fn grid_snapping_the_console_and_project_settings_through_the_bridge() {
    let (_project, assets) = project("bridge");
    let (_editor, mut client) = launch(&assets);
    // The scene is open once the box is there to be found.
    let start = Instant::now();
    while client
        .call(
            Command::Rename {
                entity: "Box".to_owned(),
                name: "Box".to_owned(),
            },
            WAIT,
        )
        .is_err()
    {
        assert!(start.elapsed() < WAIT, "the scene never opened");
        std::thread::sleep(Duration::from_millis(200));
    }

    // Grid snapping: a move lands on the half-unit grid, a turn on 15°.
    let snap = call(
        &mut client,
        Command::Snap {
            enabled: Some(true),
            step: Some(0.5),
            angle: Some(15.0),
            scale: None,
        },
    );
    assert_eq!(snap["enabled"], true);
    let moved = call(
        &mut client,
        Command::Drag {
            entity: "Box".to_owned(),
            tool: "move".to_owned(),
            from: [0.0, 0.0],
            to: [0.7, 1.3],
        },
    );
    assert!(
        transform_of(&moved).contains("translation:(0.5,1.5)"),
        "{}",
        transform_of(&moved)
    );
    let turned = call(
        &mut client,
        Command::Drag {
            entity: "Box".to_owned(),
            tool: "rotate".to_owned(),
            from: [1.5, 1.5],
            // 0.3 rad round the box at (0.5, 1.5).
            to: [0.5 + 0.955_336_5, 1.5 + 0.295_520_2],
        },
    );
    let rotation = transform_of(&turned);
    assert!(
        rotation.contains("rotation:0.2617"),
        "15° in radians: {rotation}"
    );
    // Off, the same move lands where it is let go.
    call(
        &mut client,
        Command::Snap {
            enabled: Some(false),
            step: None,
            angle: None,
            scale: None,
        },
    );
    let loose = call(
        &mut client,
        Command::Drag {
            entity: "Box".to_owned(),
            tool: "move".to_owned(),
            from: [0.0, 0.0],
            to: [0.1, 0.0],
        },
    );
    assert!(
        transform_of(&loose).contains("translation:(0.6,1.5)"),
        "{}",
        transform_of(&loose)
    );

    // The Console: a texture that is not there warns; the warning is shown at
    // warn and not at error.
    call(
        &mut client,
        Command::Spawn {
            name: Some("Missing".to_owned()),
            parent: None,
            components: [(
                "Sprite2D".to_owned(),
                r#"(color:(1.0,1.0,1.0,1.0),sort_order:0,texture:Some(Path("nope.png")))"#
                    .to_owned(),
            )]
            .into_iter()
            .collect(),
        },
    );
    let warned = console_until(&mut client, "warn", "nope.png", |lines| !lines.is_empty());
    assert!(!warned.is_empty(), "the missing texture is warned about");
    assert_eq!(warned[0]["level"], "WARN");
    assert!(
        console(&mut client, "error", "nope.png").is_empty(),
        "not an error"
    );

    // Project settings: a layers file that would not read is refused and
    // nothing is written; a good one is saved and read back by the editor.
    let path = assets.join("default.layers.ron");
    let before = std::fs::read_to_string(&path).expect("the layers");
    let refused = client.call(
        Command::Settings {
            file: "layers".to_owned(),
            text: Some(r#"(version: 1, layers: [""])"#.to_owned()),
        },
        WAIT,
    );
    assert!(refused.is_err(), "an empty layer name is refused");
    assert_eq!(std::fs::read_to_string(&path).expect("the layers"), before);
    let text = r#"(version: 1, layers: ["Sky", "Background", "Default", "Foreground"])"#;
    call(
        &mut client,
        Command::Settings {
            file: "layers".to_owned(),
            text: Some(text.to_owned()),
        },
    );
    assert_eq!(std::fs::read_to_string(&path).expect("the layers"), text);
    let read = call(
        &mut client,
        Command::Settings {
            file: "layers".to_owned(),
            text: None,
        },
    );
    assert_eq!(read["text"], text);
    let reloaded = console_until(
        &mut client,
        "info",
        "reloaded default.layers.ron",
        |lines| !lines.is_empty(),
    );
    assert!(
        !reloaded.is_empty(),
        "the editor read the saved layers back"
    );
}
