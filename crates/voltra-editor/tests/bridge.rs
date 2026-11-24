//! The editor's comforts driven through its bridge, as a tool drives it:
//! grid snapping on a drag, the Console's level filter, and a project
//! setting saved as its file and picked up live; and the particle editor's
//! live preview — a curve edited through the bridge changes the next
//! screenshot, as one undo entry.
//!
//! Launches the editor, so it needs a display and a GPU adapter; ignored by
//! default. Under Xvfb with a software Vulkan driver:
//!
//!   xvfb-run -a cargo test -p voltra-editor --test bridge -- --ignored

use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use voltra_bridge::{BridgeClient, Command};

const WAIT: Duration = Duration::from_secs(30);

/// A scene holding one box.
const BOX: &str = r#"(version: 1, entities: [(id: "019fe500-0000-7000-8000-000000000001", components: {"Name": "Box", "Transform": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0)), "Sprite": (color:(1.0,0.5,0.2,1.0),sort_order:0)})])"#;

/// A project of its own, named `name`: the engine's input map, layers and
/// font, `scene` as `scenes/box.ron`, and a `build.ron` beside the assets.
fn project(name: &str, scene: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("voltra-bridge-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let assets = dir.join("assets");
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    for file in [
        "default.input.ron",
        "default.layers.ron",
        "fonts/Ubuntu-Light.ttf",
    ] {
        let to = assets.join(file);
        std::fs::create_dir_all(to.parent().expect("a folder")).expect("folders");
        std::fs::copy(engine.join(file), &to).expect("copied");
    }
    std::fs::create_dir_all(assets.join("scenes")).expect("scenes");
    std::fs::write(assets.join("scenes/box.ron"), scene).expect("the scene");
    std::fs::write(
        dir.join("build.ron"),
        r#"(version: 1, name: "Bridge Test", scene: "scenes/box.ron")"#,
    )
    .expect("the manifest");
    assets
}

/// The editor process, killed when the test ends however it ends, and the
/// file its log goes to.
struct Editor {
    child: Child,
    log: PathBuf,
}

impl Editor {
    /// What the editor has logged so far, for a failure to show.
    fn log(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_else(|e| format!("(no log: {e})"))
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .expect("a free port")
}

/// Starts the editor on the project and waits for its bridge — failing at
/// once, with the editor's log, if it exits first, and with the log too if
/// it never answers.
fn launch(assets: &Path) -> (Editor, BridgeClient) {
    let port = free_port();
    let log = assets.with_file_name("editor.log");
    let stderr = std::fs::File::create(&log).expect("the log file");
    let child = Process::new(env!("CARGO_BIN_EXE_voltra-editor"))
        .args(["scenes/box.ron", "--bridge", &port.to_string()])
        .env("VOLTRA_ASSET_ROOT", assets)
        .env(
            "RUST_LOG",
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned()),
        )
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .expect("the editor starts");
    let mut editor = Editor { child, log };
    let start = Instant::now();
    loop {
        if let Ok(client) = BridgeClient::connect(port, Duration::from_secs(2)) {
            return (editor, client);
        }
        if let Ok(Some(status)) = editor.child.try_wait() {
            panic!(
                "the editor exited ({status}) before its bridge answered:\n{}",
                editor.log()
            );
        }
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "the bridge never answered; the editor logged:\n{}",
            editor.log()
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Waits until the scene is open: `entity` is there to be found.
fn wait_for(client: &mut BridgeClient, entity: &str) {
    let start = Instant::now();
    while client
        .call(
            Command::Rename {
                entity: entity.to_owned(),
                name: entity.to_owned(),
            },
            WAIT,
        )
        .is_err()
    {
        assert!(start.elapsed() < WAIT, "the scene never opened");
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn call(client: &mut BridgeClient, command: Command) -> Value {
    client
        .call(command, WAIT)
        .unwrap_or_else(|e| panic!("the bridge refused: {e}"))
}

fn transform_of(answer: &Value) -> String {
    answer["components"]["Transform"]
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
    let assets = project("comforts", BOX);
    let (_editor, mut client) = launch(&assets);
    wait_for(&mut client, "Box");

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
                "Sprite".to_owned(),
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

/// An emitter of still particles over a box round the origin, its size
/// curve a constant `size` — the field the test edits.
fn emitter(size: f32) -> String {
    format!(
        "(emitting:true,rate:400.0,max_particles:400,lifetime:(30.0,30.0),speed:(0.0,0.0),\
direction:0.0,spread:0.0,acceleration:(0.0,0.0),size:(0.1,0.1),\
size_curve:Some([(t:0.0,value:{size:?})]),shape:Box(half_extents:(3.0,2.0)),\
color_start:(1.0,1.0,1.0,1.0),color_end:(1.0,1.0,1.0,1.0),texture:None,sort_order:5,\
world_space:true,one_shot:false)"
    )
}

/// The viewport, decoded.
fn screenshot(client: &mut BridgeClient) -> image::RgbaImage {
    use base64::Engine;
    let answer = call(client, Command::Screenshot);
    let png = base64::engine::general_purpose::STANDARD
        .decode(answer["png_base64"].as_str().expect("a png"))
        .expect("base64");
    image::load_from_memory(&png).expect("a png").to_rgba8()
}

/// Pixels nearly white: where particles are.
fn white(shot: &image::RgbaImage) -> usize {
    shot.pixels()
        .filter(|p| p.0[..3].iter().all(|&c| c > 230))
        .count()
}

/// The preview fills in over a second: the shot after it has.
fn settled(client: &mut BridgeClient) -> usize {
    std::thread::sleep(Duration::from_millis(1500));
    white(&screenshot(client))
}

#[test]
#[ignore = "launches the editor: needs a display and a GPU adapter"]
fn a_curve_edited_through_the_bridge_changes_the_preview_as_one_undo_entry() {
    let scene = format!(
        r#"(version: 1, entities: [(id: "019fe500-0000-7000-8000-000000000002", components: {{"Name": "Fx", "Transform": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0)), "ParticleEmitter": {}}})])"#,
        emitter(0.5)
    );
    let assets = project("particles", &scene);
    let (_editor, mut client) = launch(&assets);
    wait_for(&mut client, "Fx");
    // Selected and not playing: the preview runs.
    call(
        &mut client,
        Command::Select {
            entities: vec!["Fx".to_owned()],
        },
    );
    let status = call(&mut client, Command::Status);
    assert_eq!(status["play"], "Editing");
    assert!(status["undo"].is_null(), "nothing done yet: {status}");
    let small = settled(&mut client);
    assert!(
        small > 50,
        "the preview shows particles: {small} white pixels"
    );

    // The size curve three times as big, through the bridge.
    call(
        &mut client,
        Command::ComponentSet {
            entity: "Fx".to_owned(),
            component: "ParticleEmitter".to_owned(),
            ron: emitter(1.5),
        },
    );
    let big = settled(&mut client);
    assert!(
        big > small * 3,
        "a bigger size curve covers more: {small} then {big}"
    );

    // One entry: undone once, nothing is left to undo.
    let status = call(&mut client, Command::Status);
    assert!(status["undo"].is_string(), "the edit is an entry: {status}");
    call(&mut client, Command::Undo);
    let status = call(&mut client, Command::Status);
    assert!(status["undo"].is_null(), "one entry, not more: {status}");
    let read = call(&mut client, Command::Read);
    assert!(
        read.to_string().contains("value:0.5"),
        "the curve is back: {read}"
    );
    let back = settled(&mut client);
    assert!(
        back < big / 2,
        "the undone curve draws small again: {back} after {big}"
    );
}
